use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use contract::{Context, Pipeline, Result};
use futures::stream::{self, StreamExt};
use node_semver::Version;
use resolver::{ArtifactSource, Edge, NodeId, ResolutionGraph, ResolvedNode};
use tokio::fs;

use super::bins::package_bins;
use super::prune::prune_node_modules;
use super::virtual_store::VirtualStore;

const LINK_CONCURRENCY: usize = 16;
const COPY_EXCLUDED: [&str; 2] = ["node_modules", ".git"];

/// Lays the graph out under the project's `node_modules`: every package in the virtual store
/// (`.craft/<entry>/node_modules/<name>`) with its dependencies linked beside it, every package
/// name hoisted once into `.craft/node_modules`, and the project's own dependencies linked at
/// the top. Whatever the graph no longer contains is pruned. Nothing outside the project is
/// written.
pub struct LinkerPipe {
    context: Context,
    graph: ResolutionGraph,
    store: VirtualStore,
}

impl LinkerPipe {
    pub fn new(context: &Context, graph: ResolutionGraph) -> Self {
        Self {
            store: VirtualStore::new(&context.node_modules_dir()),
            context: context.clone(),
            graph,
        }
    }

    /// Imports a package into its virtual store entry: store entries once (they never change),
    /// `file:` directories afresh from their source every install, `link:` targets not at all.
    async fn import_package(&self, node_id: &NodeId, node: &ResolvedNode) -> Result<()> {
        let dest = self.store.package_dir(node_id, &node.artifact);
        let imported = match &node.artifact.source {
            ArtifactSource::Link { .. } => return Ok(()),
            ArtifactSource::Directory { path } => {
                fsutil::replace_with_copy(path, &dest, &COPY_EXCLUDED).await
            }
            _ => fsutil::import_dir(&node.artifact.package_dir(&self.context), &dest).await,
        };
        imported.with_context(|| format!("failed to import {node_id} into node_modules"))
    }

    /// Links a package's dependencies beside it in its virtual store entry, plus their bins.
    async fn link_dependencies(&self, node_id: &NodeId, node: &ResolvedNode) -> Result<()> {
        let is_link = matches!(node.artifact.source, ArtifactSource::Link { .. });
        if is_link || node.dependencies.is_empty() {
            return Ok(());
        }

        let modules_dir = self.store.modules_dir(node_id, &node.artifact);
        for edge in &node.dependencies {
            let is_self_reference = edge.name == node.artifact.name;
            if is_self_reference {
                continue;
            }
            self.link_edge(edge, &modules_dir).await?;
        }
        self.link_bins(&node.dependencies, &modules_dir).await
    }

    /// Links `<modules_dir>/<edge.name>` to the package the edge resolved to.
    async fn link_edge(&self, edge: &Edge, modules_dir: &Path) -> Result<()> {
        let package_dir = self.target_dir(edge)?;
        self.link_dir(&package_dir, &modules_dir.join(&edge.name))
            .await
    }

    /// Hoists one package per name into `.craft/node_modules`: the project's own dependency
    /// when it has one, otherwise the highest version in the graph.
    async fn hoist(&self) -> Result<()> {
        let mut hoisted: BTreeMap<&str, &Edge> = BTreeMap::new();
        for root in &self.graph.roots {
            hoisted.entry(&root.name).or_insert(root);
        }
        let root_names: Vec<&str> = hoisted.keys().copied().collect();

        for node in self.graph.nodes.values() {
            for edge in &node.dependencies {
                let is_root_name = root_names.contains(&edge.name.as_str());
                if is_root_name {
                    continue;
                }
                let is_newer = match hoisted.get(edge.name.as_str()) {
                    Some(current) => self.version_of(edge) > self.version_of(current),
                    None => true,
                };
                if is_newer {
                    hoisted.insert(&edge.name, edge);
                }
            }
        }

        let hoist_dir = self.store.hoist_dir();
        for (name, edge) in hoisted {
            let package_dir = self.target_dir(edge)?;
            self.link_dir(&package_dir, &hoist_dir.join(name)).await?;
        }
        Ok(())
    }

    /// Links the bins of `edges` into `node_modules/.bin`, one package at a time in name order,
    /// so when two packages declare the same bin the first keeps it on every install.
    async fn link_bins(&self, edges: &[Edge], node_modules: &Path) -> Result<()> {
        let mut edges: Vec<&Edge> = edges.iter().collect();
        edges.sort_by(|a, b| a.name.cmp(&b.name));
        let bin_dir = node_modules.join(".bin");
        let mut owners: HashMap<String, &str> = HashMap::new();

        for edge in edges {
            let artifact = &self.graph.target(edge)?.artifact;
            let Some(manifest) = &artifact.package else {
                continue;
            };
            let package_dir = node_modules.join(&edge.name);

            for (bin_name, bin_path) in package_bins(&artifact.name, manifest, &package_dir) {
                if let Some(owner) = owners.get(&bin_name) {
                    eprintln!(
                        "warning: bin {bin_name} of {} conflicts with {owner}; keeping {owner}'s",
                        edge.name
                    );
                    continue;
                }

                let link = bin_dir.join(&bin_name);
                let source = package_dir.join(&bin_path);
                fsutil::force_symlink_file(&fsutil::relative_target(&link, &source), &link).await?;
                make_executable(&source).await;
                debug::info!("Linked bin {} -> {:?}", bin_name, source);
                owners.insert(bin_name, &edge.name);
            }
        }
        Ok(())
    }

    fn target_dir(&self, edge: &Edge) -> Result<PathBuf> {
        let target = self.graph.target(edge)?;
        Ok(self.store.package_dir(&edge.target, &target.artifact))
    }

    fn version_of(&self, edge: &Edge) -> Option<Version> {
        let artifact = &self.graph.nodes.get(&edge.target)?.artifact;
        Version::parse(&artifact.version).ok()
    }

    /// Links within the project are relative, so the project can be moved; links to `link:`
    /// targets elsewhere stay absolute.
    async fn link_dir(&self, target: &Path, link: &Path) -> Result<()> {
        let is_inside_node_modules = target.starts_with(self.context.node_modules_dir());
        let link_target = if is_inside_node_modules {
            fsutil::relative_target(link, target)
        } else {
            target.to_path_buf()
        };
        fsutil::force_symlink_dir(&link_target, link).await?;
        debug::info!("Linked {:?} -> {:?}", link, link_target);
        Ok(())
    }
}

impl Pipeline<()> for LinkerPipe {
    async fn run(&self) -> Result<()> {
        let node_modules = self.context.node_modules_dir();
        fsutil::ensure_dir(&node_modules).await?;
        let nodes: Vec<(NodeId, ResolvedNode)> = self
            .graph
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.clone()))
            .collect();

        run_all(
            nodes
                .clone()
                .into_iter()
                .map(|(id, node)| async move { self.import_package(&id, &node).await }),
        )
        .await?;
        run_all(
            nodes
                .into_iter()
                .map(|(id, node)| async move { self.link_dependencies(&id, &node).await }),
        )
        .await?;
        self.hoist().await?;

        let node_modules = &node_modules;
        run_all(
            self.graph
                .roots
                .clone()
                .into_iter()
                .map(|root| async move { self.link_edge(&root, node_modules).await }),
        )
        .await?;
        self.link_bins(&self.graph.roots, node_modules).await?;

        let root_names: HashSet<String> = self
            .graph
            .roots
            .iter()
            .map(|root| root.name.clone())
            .collect();
        prune_node_modules(node_modules, Some(&root_names))
            .await
            .context("failed to prune node_modules")
    }
}

async fn run_all(tasks: impl Iterator<Item = impl Future<Output = Result<()>>>) -> Result<()> {
    let results: Vec<Result<()>> = stream::iter(tasks)
        .buffer_unordered(LINK_CONCURRENCY)
        .collect()
        .await;
    results.into_iter().collect()
}

/// Adds execute permission for everyone, keeping the file's other mode bits.
#[cfg(unix)]
async fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = fs::metadata(path).await {
        let mode = metadata.permissions().mode() | 0o111;
        let _ = fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await;
    }
}

#[cfg(not(unix))]
async fn make_executable(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use resolver::{DependencyKind, ResolvedArtifact};
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_link_binaries() -> Result<()> {
        let temp_home = tempdir()?;
        let temp_cwd = tempdir()?;
        let context = Context::new(
            temp_cwd.path().to_path_buf(),
            temp_home.path().to_path_buf(),
            false,
        );

        let pkg_name = "test-pkg";
        let pkg_version = "1.0.0";

        let pkg_json = package::PackageJson {
            name: Some(pkg_name.to_string()),
            version: Some(pkg_version.to_string()),
            scripts: None,
            dependencies: None,
            dev_dependencies: None,
            dist: None,
            bin: Some(package::PackageBin::String("cli.js".to_string())),
            other: HashMap::new(),
        };

        let artifact = ResolvedArtifact {
            name: pkg_name.to_string(),
            version: pkg_version.to_string(),
            source: resolver::ArtifactSource::Registry {
                tarball_url: "http://example.com".to_string(),
            },
            integrity: None,
            package: Some(pkg_json),
        };

        let pkg_dir = artifact.package_dir(&context);
        fs::create_dir_all(&pkg_dir).await?;
        {
            let mut file = File::create(pkg_dir.join("cli.js"))?;
            writeln!(file, "#!/usr/bin/env node")?;
            writeln!(file, "console.log('hello');")?;
        }

        let node_id = NodeId::new(pkg_name, pkg_version);
        let graph = ResolutionGraph {
            roots: vec![Edge {
                name: pkg_name.to_string(),
                spec: Some(pkg_version.to_string()),
                target: node_id.clone(),
                kind: DependencyKind::Production,
            }],
            nodes: [(
                node_id,
                ResolvedNode {
                    artifact,
                    dependencies: vec![],
                },
            )]
            .into(),
        };
        let pipe = LinkerPipe::new(&context, graph);
        pipe.run().await?;

        // Verify
        let bin_link = temp_cwd
            .path()
            .join("node_modules")
            .join(".bin")
            .join(pkg_name);
        assert!(bin_link.exists());
        assert!(bin_link.is_symlink());

        let target = tokio::fs::read_link(&bin_link).await?;
        assert_eq!(target, PathBuf::from("../test-pkg/cli.js"));

        Ok(())
    }
}
