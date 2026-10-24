use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Context as _;
use contract::{Context, Pipeline, Result, ScriptError};
use futures::stream::{self, StreamExt};
use process::{Process, ScriptPackage};
use resolver::{ArtifactSource, DependencyKind, NodeId, ResolutionGraph, ResolvedNode};

use super::virtual_store::VirtualStore;

const SCRIPT_CONCURRENCY: usize = 8;

/// A package whose install scripts still have to run, with those scripts in order.
type PendingBuild = (NodeId, ResolvedNode, Vec<(String, String)>);

/// Which dependencies may run install scripts. Nothing runs unless the project trusts the
/// package by name, since install scripts are how compromised packages attack machines.
#[derive(Debug, Clone, Default)]
pub struct ScriptPolicy {
    pub ignore_scripts: bool,
    pub trusted: BTreeSet<String>,
}

impl ScriptPolicy {
    fn allows(&self, name: &str) -> bool {
        !self.ignore_scripts && self.trusted.contains(name)
    }

    /// Recorded with the installed state, so changing the policy re-runs the install.
    pub fn describe(&self) -> String {
        if self.ignore_scripts {
            return "scripts: ignored".to_string();
        }
        let trusted: Vec<&str> = self.trusted.iter().map(String::as_str).collect();
        format!("scripts: trusted {}", trusted.join(","))
    }
}

#[derive(Debug, Default)]
pub struct LifecycleReport {
    /// Packages with install scripts the policy did not allow, by instance id.
    pub skipped: Vec<String>,
}

/// Runs dependencies' `preinstall` / `install` / `postinstall` scripts once per virtual store
/// entry, dependencies before their dependents.
pub struct LifecyclePipe {
    context: Context,
    graph: ResolutionGraph,
    policy: ScriptPolicy,
    store: VirtualStore,
}

impl LifecyclePipe {
    pub fn new(context: &Context, graph: ResolutionGraph, policy: ScriptPolicy) -> Self {
        Self {
            store: VirtualStore::new(&context.node_modules_dir()),
            context: context.clone(),
            graph,
            policy,
        }
    }

    /// Packages that still have install scripts to run here: installed on this platform, not a
    /// `link:` target, and not built before.
    fn pending(&self) -> Vec<PendingBuild> {
        self.graph
            .nodes
            .iter()
            .filter(|(_, node)| node.artifact.platform.supports(&self.context.platform))
            .filter(|(_, node)| !matches!(node.artifact.source, ArtifactSource::Link { .. }))
            .filter(|(id, node)| !self.store.built_marker(id, &node.artifact).exists())
            .filter_map(|(id, node)| {
                let package_dir = self.store.package_dir(id, &node.artifact);
                let manifest = installed_manifest(&package_dir)?;
                let scripts: Vec<(String, String)> = manifest
                    .install_scripts(&package_dir)
                    .into_iter()
                    .map(|(event, script)| (event.to_string(), script))
                    .collect();
                (!scripts.is_empty()).then(|| (id.clone(), node.clone(), scripts))
            })
            .collect()
    }

    /// Builds one package: re-imports it by copy first, so its scripts can't write through
    /// hardlinks into the shared store, then runs each script and marks the entry built.
    async fn build(
        &self,
        node_id: &NodeId,
        node: &ResolvedNode,
        scripts: &[(String, String)],
    ) -> Result<()> {
        let package_dir = self.store.package_dir(node_id, &node.artifact);
        let is_from_store = !matches!(node.artifact.source, ArtifactSource::Directory { .. });
        if is_from_store {
            fsutil::replace_with_copy(&node.artifact.package_dir(&self.context), &package_dir, &[])
                .await
                .with_context(|| format!("failed to prepare {node_id} for its install scripts"))?;
        }

        let manifest_path = package_dir.join("package.json");
        let package = ScriptPackage {
            name: Some(&node.artifact.name),
            version: Some(&node.artifact.version),
            manifest_path: &manifest_path,
        };
        for (event, script) in scripts {
            debug::info!("Running {event} of {node_id}: {script}");
            let (code, output) = Process::shell(script)
                .lifecycle(event, script)
                .script_env(&package_dir, &self.context.cwd, &package)?
                .run_captured()
                .await?;
            if self.context.verbose {
                eprint!("{output}");
            }
            if code != 0 {
                return Err(ScriptError::InstallFailed {
                    package: node_id.to_string(),
                    event: event.to_string(),
                    code,
                    output,
                }
                .into());
            }
        }

        fsutil::write_if_changed(&self.store.built_marker(node_id, &node.artifact), b"").await?;
        println!("Ran install scripts of {node_id}");
        Ok(())
    }

    /// A package only optional dependencies lead to may fail its scripts without failing the
    /// install; its entry is removed either way, so the next install retries the build.
    async fn handle_failure(
        &self,
        node_id: &NodeId,
        node: &ResolvedNode,
        error: anyhow::Error,
        required: &HashSet<NodeId>,
    ) -> Result<()> {
        fsutil::remove_entry(&self.store.entry_dir(node_id, &node.artifact)).await?;
        if required.contains(node_id) {
            return Err(error);
        }
        eprintln!("warning: skipping optional {node_id}: {error:#}");
        Ok(())
    }
}

impl Pipeline<LifecycleReport> for LifecyclePipe {
    async fn run(&self) -> Result<LifecycleReport> {
        let mut report = LifecycleReport::default();
        if self.policy.ignore_scripts {
            return Ok(report);
        }

        let (allowed, skipped): (Vec<_>, Vec<_>) = self
            .pending()
            .into_iter()
            .partition(|(_, node, _)| self.policy.allows(&node.artifact.name));
        report.skipped = skipped.iter().map(|(id, ..)| id.to_string()).collect();
        if allowed.is_empty() {
            return Ok(report);
        }

        let required = required_nodes(&self.graph);
        let depths = dependency_depths(&self.graph);
        let mut waves: BTreeMap<usize, Vec<_>> = BTreeMap::new();
        for build in allowed {
            let depth = depths.get(&build.0).copied().unwrap_or_default();
            waves.entry(depth).or_default().push(build);
        }

        for wave in waves.into_values() {
            let required = &required;
            let results: Vec<Result<()>> = stream::iter(wave)
                .map(|(id, node, scripts)| async move {
                    match self.build(&id, &node, &scripts).await {
                        Ok(()) => Ok(()),
                        Err(error) => self.handle_failure(&id, &node, error, required).await,
                    }
                })
                .buffer_unordered(SCRIPT_CONCURRENCY)
                .collect()
                .await;
            results.into_iter().collect::<Result<()>>()?;
        }
        Ok(report)
    }
}

/// The package.json shipped inside the installed package: registry metadata may leave
/// `scripts` out, the package itself never does.
fn installed_manifest(package_dir: &std::path::Path) -> Option<package::PackageJson> {
    let content = std::fs::read_to_string(package_dir.join("package.json")).ok()?;
    serde_json::from_str(&content).ok()
}

/// Nodes reachable from the roots without passing through an optional dependency.
fn required_nodes(graph: &ResolutionGraph) -> HashSet<NodeId> {
    let mut required = HashSet::new();
    let mut pending: Vec<&NodeId> = graph.roots.iter().map(|root| &root.target).collect();
    while let Some(id) = pending.pop() {
        if !required.insert(id.clone()) {
            continue;
        }
        let Some(node) = graph.nodes.get(id) else {
            continue;
        };
        pending.extend(
            node.dependencies
                .iter()
                .filter(|edge| edge.kind != DependencyKind::Optional)
                .map(|edge| &edge.target),
        );
    }
    required
}

/// How far each node is from the leaves of the graph, so builds can run deepest-dependency
/// first. Cycles are cut where they close.
fn dependency_depths(graph: &ResolutionGraph) -> HashMap<NodeId, usize> {
    let mut depths = HashMap::new();
    for id in graph.nodes.keys() {
        depth_of(graph, id, &mut depths, &mut HashSet::new());
    }
    depths
}

fn depth_of<'g>(
    graph: &'g ResolutionGraph,
    id: &'g NodeId,
    depths: &mut HashMap<NodeId, usize>,
    visiting: &mut HashSet<&'g NodeId>,
) -> usize {
    if let Some(depth) = depths.get(id) {
        return *depth;
    }
    if !visiting.insert(id) {
        return 0;
    }
    let depth = graph.nodes.get(id).map_or(0, |node| {
        node.dependencies
            .iter()
            .map(|edge| depth_of(graph, &edge.target, depths, visiting) + 1)
            .max()
            .unwrap_or(0)
    });
    visiting.remove(id);
    depths.insert(id.clone(), depth);
    depth
}

/// Runs the project's own install lifecycle (`preinstall`, `install`, `postinstall`,
/// `prepare`) with its output shown, as npm does after installing dependencies.
pub async fn run_project_scripts(context: &Context, manifest: &package::PackageJson) -> Result<()> {
    let Some(scripts) = &manifest.scripts else {
        return Ok(());
    };
    let manifest_path = context.package_json_path();
    let package = ScriptPackage {
        name: manifest.name.as_deref(),
        version: manifest.version.as_deref(),
        manifest_path: &manifest_path,
    };

    for event in ["preinstall", "install", "postinstall", "prepare"] {
        let Some(script) = scripts.get(event) else {
            continue;
        };
        let code = Process::shell(script)
            .lifecycle(event, script)
            .script_env(&context.cwd, &context.cwd, &package)?
            .run()
            .await?;
        if code != 0 {
            return Err(ScriptError::Failed {
                name: event.to_string(),
                code,
            }
            .into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolver::{Edge, ResolvedArtifact};

    fn node(dependencies: &[(&str, DependencyKind)]) -> ResolvedNode {
        ResolvedNode {
            artifact: ResolvedArtifact {
                name: "pkg".to_string(),
                version: "1.0.0".to_string(),
                source: ArtifactSource::Registry {
                    tarball_url: String::new(),
                },
                integrity: None,
                platform: Default::default(),
                package: None,
            },
            dependencies: dependencies
                .iter()
                .map(|(target, kind)| Edge {
                    name: target.to_string(),
                    spec: None,
                    target: NodeId::new(target, "1.0.0"),
                    kind: *kind,
                })
                .collect(),
            peers: Vec::new(),
        }
    }

    fn graph() -> ResolutionGraph {
        let id = |name: &str| NodeId::new(name, "1.0.0");
        ResolutionGraph {
            roots: vec![Edge {
                name: "app".to_string(),
                spec: None,
                target: id("app"),
                kind: DependencyKind::Production,
            }],
            nodes: BTreeMap::from([
                (
                    id("app"),
                    node(&[
                        ("native", DependencyKind::Production),
                        ("fsevents", DependencyKind::Optional),
                    ]),
                ),
                (
                    id("native"),
                    node(&[("bindings", DependencyKind::Production)]),
                ),
                (
                    id("bindings"),
                    node(&[("native", DependencyKind::Production)]),
                ),
                (id("fsevents"), node(&[])),
            ]),
        }
    }

    #[test]
    fn test_required_nodes_exclude_optional_only_packages() {
        let required = required_nodes(&graph());
        assert!(required.contains(&NodeId::new("native", "1.0.0")));
        assert!(!required.contains(&NodeId::new("fsevents", "1.0.0")));
    }

    #[test]
    fn test_dependencies_build_before_dependents_even_in_cycles() {
        let depths = dependency_depths(&graph());
        let depth = |name: &str| depths[&NodeId::new(name, "1.0.0")];
        assert!(depth("app") > depth("native"));
        assert!(depth("app") > depth("fsevents"));
        assert_eq!(depth("fsevents"), 0);
    }

    #[test]
    fn test_policy_requires_trust_and_describes_itself() {
        let policy = ScriptPolicy {
            ignore_scripts: false,
            trusted: BTreeSet::from(["bcrypt".to_string(), "esbuild".to_string()]),
        };
        assert!(policy.allows("bcrypt"));
        assert!(!policy.allows("evil"));
        assert_eq!(policy.describe(), "scripts: trusted bcrypt,esbuild");

        let ignored = ScriptPolicy {
            ignore_scripts: true,
            ..policy
        };
        assert!(!ignored.allows("bcrypt"));
        assert_eq!(ignored.describe(), "scripts: ignored");
    }
}
