use std::collections::HashMap;
use std::path::{Path, PathBuf};

use contract::{Context, Pipeline, Result};
use futures::stream::{self, StreamExt};
use package::PackageBin;
use resolver::{Edge, ResolutionGraph, ResolvedArtifact, ResolvedNode};
use tokio::fs;

const LINK_CONCURRENCY: usize = 10;

pub struct LinkerPipe {
    context: Context,
    graph: ResolutionGraph,
}

impl LinkerPipe {
    pub fn new(context: &Context, graph: ResolutionGraph) -> Self {
        Self {
            context: context.clone(),
            graph,
        }
    }

    fn package_dir(&self, artifact: &ResolvedArtifact) -> PathBuf {
        self.context.package_dir(&artifact.name, &artifact.version)
    }

    /// Links a package's dependencies into its own node_modules in the cache.
    async fn link_dependencies(&self, node: &ResolvedNode) -> Result<()> {
        if node.dependencies.is_empty() {
            return Ok(());
        }
        debug::info!("Hydrating package: {}", node.artifact.name);

        let node_modules = self.package_dir(&node.artifact).join("node_modules");
        fsutil::ensure_dir(&node_modules).await?;

        for edge in &node.dependencies {
            self.link_edge(edge, &node_modules).await?;
        }
        Ok(())
    }

    /// Links `node_modules/<edge.name>` to the package the edge resolved to, plus its bins.
    async fn link_edge(&self, edge: &Edge, node_modules: &Path) -> Result<()> {
        let target = self.graph.target(edge)?;
        let source_dir = self.package_dir(&target.artifact);
        let dest_path = node_modules.join(&edge.name);
        fsutil::force_symlink_dir(&source_dir, &dest_path).await?;
        debug::info!("Linked {} -> {:?}", edge.name, source_dir);

        self.link_package_binaries(&edge.name, &target.artifact, node_modules)
            .await
    }

    /// Helper to link binaries for a package into a node_modules directory
    async fn link_package_binaries(
        &self,
        link_name: &str,
        artifact: &ResolvedArtifact,
        node_modules_dir: &Path,
    ) -> Result<()> {
        let Some(pkg_json) = &artifact.package else {
            return Ok(());
        };
        if let Some(bin) = &pkg_json.bin {
            let bin_dir = node_modules_dir.join(".bin");
            fsutil::ensure_dir(&bin_dir).await?;

            let bins = match bin {
                PackageBin::String(path) => {
                    let mut map = HashMap::new();
                    map.insert(artifact.name.clone(), path.clone());
                    map
                }
                PackageBin::Map(map) => map.clone(),
            };

            for (bin_name, bin_path) in bins {
                let target_path = bin_dir.join(&bin_name);
                let source_path = node_modules_dir.join(link_name).join(&bin_path);
                let relative_source = fsutil::relative_target(&target_path, &source_path);
                fsutil::force_symlink_file(&relative_source, &target_path).await?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = fs::metadata(&source_path).await {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&source_path, perms).await;
                    }
                }

                debug::info!("Linked bin {} -> {:?}", bin_name, source_path);
            }
        }
        Ok(())
    }
}

impl Pipeline<()> for LinkerPipe {
    async fn run(&self) -> Result<()> {
        let node_modules = self.context.node_modules_dir();
        fsutil::ensure_dir(&node_modules).await?;

        // 1. Hydrate all packages (link dependencies in cache)
        let nodes: Vec<ResolvedNode> = self.graph.nodes.values().cloned().collect();
        let hydration_results: Vec<Result<()>> = stream::iter(nodes)
            .map(|node| async move { self.link_dependencies(&node).await })
            .buffer_unordered(LINK_CONCURRENCY)
            .collect()
            .await;

        for result in hydration_results {
            result?;
        }

        // 2. Link root packages to project node_modules
        let roots = self.graph.roots.clone();
        let node_modules = &node_modules;
        let linking_results: Vec<Result<()>> = stream::iter(roots)
            .map(|edge| async move { self.link_edge(&edge, node_modules).await })
            .buffer_unordered(LINK_CONCURRENCY)
            .collect()
            .await;

        for result in linking_results {
            result?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolver::{DependencyKind, NodeId};
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

        // Setup cache
        let pkg_name = "test-pkg";
        let pkg_version = "1.0.0";
        let pkg_dir = context.package_dir(pkg_name, pkg_version);
        fs::create_dir_all(&pkg_dir).await?;

        // Create bin script
        let bin_script_path = pkg_dir.join("cli.js");
        {
            let mut file = File::create(&bin_script_path)?;
            writeln!(file, "#!/usr/bin/env node")?;
            writeln!(file, "console.log('hello');")?;
        }

        // Create artifact
        let pkg_json = package::PackageJson {
            name: Some(pkg_name.to_string()),
            version: Some(pkg_version.to_string()),
            description: None,
            scripts: None,
            keywords: None,
            dependencies: None,
            dev_dependencies: None,
            dist: None,
            bin: Some(package::PackageBin::String("cli.js".to_string())),
            other: HashMap::new(),
        };

        let artifact = ResolvedArtifact {
            name: pkg_name.to_string(),
            version: pkg_version.to_string(),
            download_url: "http://example.com".to_string(),
            integrity: None,
            package: Some(pkg_json),
        };

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
