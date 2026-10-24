use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use contract::{Context, Pipeline, Result};
use futures::stream::{self, StreamExt};
use node_semver::{Range, Version};
use package::PackageBin;
use resolver::ResolvedArtifact;
use tokio::fs;

use crate::pipes::install_pipe::{Installation, ResolvedRoot};

pub struct LinkerPipe {
    context: Context,
    artifacts: Vec<ResolvedArtifact>,
    roots: Vec<ResolvedRoot>,
}

impl LinkerPipe {
    pub fn new(context: &Context, installation: Installation) -> Self {
        Self {
            context: context.clone(),
            artifacts: installation.artifacts,
            roots: installation.roots,
        }
    }

    fn package_dir(&self, artifact: &ResolvedArtifact) -> PathBuf {
        self.context
            .package_cache_dir()
            .join(format!("{}-{}/package", artifact.name, artifact.version))
    }

    /// Links dependencies of an artifact into its cached node_modules
    async fn hydrate_artifact(
        &self,
        artifact: &ResolvedArtifact,
        artifact_map: &HashMap<String, Vec<ResolvedArtifact>>,
    ) -> Result<()> {
        debug::info!("Hydrating package: {}", artifact.name);

        let source_dir = self.package_dir(artifact);

        // Link dependencies into the artifact's node_modules
        if let Some(deps) = artifact
            .package
            .as_ref()
            .and_then(|p| p.dependencies.as_ref())
        {
            let artifact_node_modules = source_dir.join("node_modules");
            fs::create_dir_all(&artifact_node_modules).await?;

            for (dep_name, dep_version) in deps {
                // Find best matching artifact
                if let Some(candidates) = artifact_map.get(dep_name) {
                    let req = Range::parse(dep_version).unwrap_or_else(|_| Range::any());

                    let best_match = highest_satisfying(candidates, &req);

                    if let Some(dep_artifact) = best_match {
                        let dep_source_dir = self.package_dir(dep_artifact);
                        let dep_dest_path = artifact_node_modules.join(dep_name);

                        // Create parent dirs for scoped packages
                        if let Some(parent) = dep_dest_path.parent() {
                            fs::create_dir_all(parent).await?;
                        }

                        // Remove existing link if exists
                        if dep_dest_path.exists() {
                            if dep_dest_path.is_symlink() {
                                fs::remove_file(&dep_dest_path).await?;
                            } else {
                                fs::remove_dir_all(&dep_dest_path).await?;
                            }
                        }

                        #[cfg(unix)]
                        tokio::fs::symlink(&dep_source_dir, &dep_dest_path).await?;
                        #[cfg(windows)]
                        tokio::fs::symlink_dir(&dep_source_dir, &dep_dest_path).await?;

                        // Link binaries for this dependency
                        self.link_package_binaries(dep_artifact, &artifact_node_modules)
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Links a root package to the project's node_modules
    async fn link_root_package(&self, root: &ResolvedRoot) -> Result<()> {
        debug::info!("Linking root package: {}", root.package.name);
        let artifact = &root.artifact;

        // Source path in global cache
        let source_dir = self.package_dir(artifact);

        // Destination path in node_modules
        let node_modules = self.context.node_modules_dir();
        let dest_path = node_modules.join(&artifact.name);

        // Create node_modules and parent dirs (for scoped packages)
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Remove existing link/directory if it exists
        if dest_path.exists() {
            if dest_path.is_symlink() {
                fs::remove_file(&dest_path).await?;
            } else {
                fs::remove_dir_all(&dest_path).await?;
            }
        }

        // Create symlink
        #[cfg(unix)]
        {
            tokio::fs::symlink(&source_dir, &dest_path).await?;
        }
        #[cfg(windows)]
        {
            tokio::fs::symlink_dir(&source_dir, &dest_path).await?;
        }

        debug::info!("Linked root {} -> {:?}", artifact.name, source_dir);

        // Link binaries for the root package
        self.link_package_binaries(artifact, &node_modules).await?;

        Ok(())
    }

    /// Helper to link binaries for a package into a node_modules directory
    async fn link_package_binaries(
        &self,
        artifact: &ResolvedArtifact,
        node_modules_dir: &std::path::Path,
    ) -> Result<()> {
        let Some(pkg_json) = &artifact.package else {
            return Ok(());
        };
        if let Some(bin) = &pkg_json.bin {
            let bin_dir = node_modules_dir.join(".bin");
            fs::create_dir_all(&bin_dir).await?;

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

                // The path to the script relative to the package root
                // We need to link to node_modules/<package_name>/<bin_path>
                // But since we are in node_modules/.bin, the relative path is ../<package_name>/<bin_path>

                let package_dir = node_modules_dir.join(&artifact.name);
                let source_path = package_dir.join(&bin_path);

                // Remove existing link if exists
                if target_path.exists() {
                    if target_path.is_symlink() {
                        fs::remove_file(&target_path).await?;
                    } else {
                        fs::remove_dir_all(&target_path).await?;
                    }
                }

                #[cfg(unix)]
                {
                    tokio::fs::symlink(&source_path, &target_path).await?;

                    // Make executable
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = fs::metadata(&source_path).await {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&source_path, perms).await;
                    }
                }
                #[cfg(windows)]
                {
                    // On Windows we might need a shim, but for now let's try symlink
                    tokio::fs::symlink_file(&source_path, &target_path).await?;
                }

                debug::info!("Linked bin {} -> {:?}", bin_name, source_path);
            }
        }
        Ok(())
    }
}

impl Pipeline<()> for LinkerPipe {
    async fn run(&self) -> Result<()> {
        // Ensure node_modules exists
        let node_modules = self.context.node_modules_dir();
        fs::create_dir_all(&node_modules).await?;

        // Index artifacts by name for dependency resolution
        let mut artifact_map: HashMap<String, Vec<ResolvedArtifact>> = HashMap::new();
        for artifact in &self.artifacts {
            artifact_map
                .entry(artifact.name.clone())
                .or_default()
                .push(artifact.clone());
        }
        let artifact_map = Arc::new(artifact_map);

        // 1. Hydrate all artifacts (link dependencies in cache)
        let artifacts = self.artifacts.clone();
        let hydration_results: Vec<Result<()>> = stream::iter(artifacts)
            .map(|artifact| {
                let map = artifact_map.clone();
                async move { self.hydrate_artifact(&artifact, &map).await }
            })
            .buffer_unordered(10)
            .collect()
            .await;

        for result in hydration_results {
            result?;
        }

        debug::trace!("LINKED NON_ROOT PACKAGES");
        // 2. Link root packages to project node_modules
        let roots = self.roots.clone();
        let linking_results: Vec<Result<()>> = stream::iter(roots)
            .map(|root| async move { self.link_root_package(&root).await })
            .buffer_unordered(10)
            .collect()
            .await;

        for result in linking_results {
            result?;
        }

        Ok(())
    }
}

fn highest_satisfying<'a>(
    candidates: &'a [ResolvedArtifact],
    range: &Range,
) -> Option<&'a ResolvedArtifact> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let version = Version::parse(&candidate.version).ok()?;
            range.satisfies(&version).then_some((version, candidate))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use package::InstallPackage;
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
        let cache_dir = context.package_cache_dir();
        let pkg_name = "test-pkg";
        let pkg_version = "1.0.0";
        let pkg_dir = cache_dir.join(format!("{}-{}/package", pkg_name, pkg_version));
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
            package: Some(pkg_json),
        };

        let root_pkg =
            InstallPackage::new(pkg_name.to_string(), Some(pkg_version.to_string()), false);

        let installation = Installation {
            artifacts: vec![artifact.clone()],
            roots: vec![ResolvedRoot {
                package: root_pkg,
                artifact,
            }],
        };
        let pipe = LinkerPipe::new(&context, installation);
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
        let expected_target = temp_cwd
            .path()
            .join("node_modules")
            .join(pkg_name)
            .join("cli.js");

        assert_eq!(target, expected_target);

        Ok(())
    }
}
