use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use lockfile::{Importer, LockedDependency, LockedPackage, LockedSource, Lockfile, ROOT_IMPORTER};
use resolver::{ArtifactSource, DependencyKind, NodeId, ResolutionGraph, ResolvedArtifact};

/// The lockfile recording `graph`. `specifiers` maps each project dependency to the spec its
/// package.json lists, which can differ from what was requested (`craft add ms` saves `^2.1.3`).
/// Local paths are written relative to `project_dir`, so the file is the same on every machine.
pub fn lockfile_from_graph(
    graph: &ResolutionGraph,
    specifiers: &HashMap<String, String>,
    project_dir: &Path,
) -> Lockfile {
    let key = |id: &NodeId| lock_key(id, project_dir);
    let mut importer = Importer::default();
    for root in &graph.roots {
        let specifier = specifiers
            .get(&root.name)
            .cloned()
            .or_else(|| root.spec.clone())
            .unwrap_or_default();
        let section = match root.kind {
            DependencyKind::Development => &mut importer.dev_dependencies,
            _ => &mut importer.dependencies,
        };
        section.insert(
            root.name.clone(),
            LockedDependency {
                specifier,
                version: key(&root.target),
            },
        );
    }

    let packages = graph
        .nodes
        .iter()
        .map(|(id, node)| {
            let mut package = LockedPackage::new(
                &node.artifact.name,
                &node.artifact.version,
                locked_source(&node.artifact, project_dir),
            );
            for edge in &node.dependencies {
                let section = match edge.kind {
                    DependencyKind::Peer => &mut package.peer_dependencies,
                    DependencyKind::Optional => &mut package.optional_dependencies,
                    DependencyKind::Production | DependencyKind::Development => {
                        &mut package.dependencies
                    }
                };
                section.insert(edge.name.clone(), key(&edge.target));
            }
            let platform = &node.artifact.platform;
            package.os = platform.os.clone();
            package.cpu = platform.cpu.clone();
            package.libc = platform.libc.clone();
            (key(id), package)
        })
        .collect();

    Lockfile {
        importers: BTreeMap::from([(ROOT_IMPORTER.to_string(), importer)]),
        packages,
        ..Lockfile::default()
    }
}

fn locked_source(artifact: &ResolvedArtifact, project_dir: &Path) -> LockedSource {
    let integrity = artifact.integrity.as_ref().map(ToString::to_string);
    match &artifact.source {
        ArtifactSource::Registry { tarball_url } => LockedSource::Registry {
            tarball: tarball_url.clone(),
            integrity,
        },
        ArtifactSource::Tarball { url } => LockedSource::Tarball {
            url: url.clone(),
            integrity,
        },
        ArtifactSource::LocalTarball { path } => LockedSource::LocalTarball {
            path: project_relative(path, project_dir),
            integrity,
        },
        ArtifactSource::Directory { path } => LockedSource::Directory {
            path: project_relative(path, project_dir),
        },
        ArtifactSource::Link { path } => LockedSource::Link {
            path: project_relative(path, project_dir),
        },
        ArtifactSource::Git { url, sha } => LockedSource::Git {
            url: url.clone(),
            commit: sha.clone(),
        },
    }
}

/// Node ids of local packages embed absolute paths; the lockfile keys them relative to the
/// project instead.
fn lock_key(id: &NodeId, project_dir: &Path) -> String {
    let project_prefix = format!("{}/", project_dir.display());
    id.to_string()
        .replace(&format!("file:{project_prefix}"), "file:")
        .replace(&format!("link:{project_prefix}"), "link:")
}

fn project_relative(path: &Path, project_dir: &Path) -> String {
    fsutil::relative_target(&project_dir.join("package.json"), path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolver::{Edge, ResolvedNode};

    fn node(
        name: &str,
        version: &str,
        source: ArtifactSource,
        dependencies: Vec<Edge>,
    ) -> ResolvedNode {
        ResolvedNode {
            artifact: ResolvedArtifact {
                name: name.to_string(),
                version: version.to_string(),
                source,
                integrity: None,
                platform: Default::default(),
                package: None,
            },
            dependencies,
            peers: Vec::new(),
        }
    }

    fn edge(name: &str, target: &NodeId, kind: DependencyKind) -> Edge {
        Edge {
            name: name.to_string(),
            spec: None,
            target: target.clone(),
            kind,
        }
    }

    #[test]
    fn test_records_roots_packages_and_peers() {
        let project = Path::new("/work/app");
        let react = NodeId::new("react", "19.2.0");
        let ui = NodeId::new("ui", "1.0.0").with_peers([&react]);
        let local = NodeId::new("local", "file:/work/app/libs/local");
        let registry = |name: &str| ArtifactSource::Registry {
            tarball_url: format!("https://registry.test/{name}.tgz"),
        };

        let graph = ResolutionGraph {
            roots: vec![
                edge("ui", &ui, DependencyKind::Production),
                edge("react", &react, DependencyKind::Production),
                edge("local", &local, DependencyKind::Development),
            ],
            nodes: BTreeMap::from([
                (
                    react.clone(),
                    node("react", "19.2.0", registry("react"), vec![]),
                ),
                (
                    ui.clone(),
                    node(
                        "ui",
                        "1.0.0",
                        registry("ui"),
                        vec![edge("react", &react, DependencyKind::Peer)],
                    ),
                ),
                (
                    local.clone(),
                    node(
                        "local",
                        "0.1.0",
                        ArtifactSource::Directory {
                            path: "/work/app/libs/local".into(),
                        },
                        vec![],
                    ),
                ),
            ]),
        };
        let specifiers = HashMap::from([
            ("ui".to_string(), "^1.0.0".to_string()),
            ("react".to_string(), "^19.0.0".to_string()),
            ("local".to_string(), "file:libs/local".to_string()),
        ]);

        let lockfile = lockfile_from_graph(&graph, &specifiers, project);

        let root = lockfile.root().unwrap();
        assert_eq!(root.dependencies["ui"].version, "ui@1.0.0_react@19.2.0");
        assert_eq!(root.dependencies["react"].specifier, "^19.0.0");
        assert_eq!(
            root.dev_dependencies["local"].version,
            "local@file:libs/local"
        );
        assert_eq!(
            lockfile.packages["ui@1.0.0_react@19.2.0"].peer_dependencies["react"],
            "react@19.2.0"
        );
        assert_eq!(
            lockfile.packages["local@file:libs/local"].resolution,
            LockedSource::Directory {
                path: "libs/local".to_string()
            }
        );
    }

    #[test]
    fn test_paths_outside_the_project_are_relative_too() {
        assert_eq!(
            project_relative(Path::new("/work/shared/lib"), Path::new("/work/app")),
            "../shared/lib"
        );
    }
}
