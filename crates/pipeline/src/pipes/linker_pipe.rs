use std::collections::HashMap;
use std::sync::Arc;

use contract::{Pipeline, Result, get_package_cache_dir};
use futures::stream::{self, StreamExt};
use node_semver::{Range, Version};
use package::InstallPackage;
use resolver::ResolvedArtifact;
use tokio::fs;

pub struct LinkerPipe {
    artifacts: Vec<ResolvedArtifact>,
    root_packages: Vec<InstallPackage>,
}

impl LinkerPipe {
    pub fn new(artifacts: Vec<ResolvedArtifact>, root_packages: Vec<InstallPackage>) -> Self {
        Self {
            artifacts,
            root_packages,
        }
    }

    /// Links dependencies of an artifact into its cached node_modules
    async fn hydrate_artifact(
        &self,
        artifact: &ResolvedArtifact,
        artifact_map: &HashMap<String, Vec<ResolvedArtifact>>,
    ) -> Result<()> {
        debug::info!("Hydrating package: {}", artifact.name);

        let source_dir =
            get_package_cache_dir().join(format!("{}-{}/package", artifact.name, artifact.version));

        // Link dependencies into the artifact's node_modules
        if let Some(pkg_json) = &artifact.package {
            if let Some(deps) = &pkg_json.dependencies {
                let artifact_node_modules = source_dir.join("node_modules");
                fs::create_dir_all(&artifact_node_modules).await?;

                for (dep_name, dep_version) in deps {
                    // Find best matching artifact
                    if let Some(candidates) = artifact_map.get(dep_name) {
                        let req = Range::parse(dep_version).unwrap_or_else(|_| Range::any());

                        // Find the candidate that satisfies the version requirement
                        // We prefer the highest version that satisfies it
                        let best_match = candidates
                            .iter()
                            .filter(|c| {
                                if let Ok(ver) = Version::parse(&c.version) {
                                    req.satisfies(&ver)
                                } else {
                                    false
                                }
                            })
                            .max_by(|a, b| {
                                let ver_a = Version::parse(&a.version).unwrap();
                                let ver_b = Version::parse(&b.version).unwrap();
                                ver_a.partial_cmp(&ver_b).unwrap()
                            });

                        if let Some(dep_artifact) = best_match {
                            let dep_source_dir = get_package_cache_dir().join(format!(
                                "{}-{}/package",
                                dep_artifact.name, dep_artifact.version
                            ));
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
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Links a root package to the project's node_modules
    async fn link_root_package(
        &self,
        package: &InstallPackage,
        artifact_map: &HashMap<String, Vec<ResolvedArtifact>>,
    ) -> Result<()> {
        debug::info!("Linking root package: {}", package.name);

        // Find the artifact for this package
        // For root packages, we usually want the one that was resolved for it.
        // Since we don't have the exact resolution map here, we'll find the best match
        // based on the requested version (or latest if not specified).
        // In practice, InstallPipe resolved it, so it must be in artifacts.

        let candidates = match artifact_map.get(&package.name) {
            Some(c) => c,
            None => {
                debug::warning!("Root package {} not found in artifacts", package.name);
                return Ok(());
            }
        };

        let best_match = if let Some(req_version) = &package.version {
            let req = Range::parse(req_version).unwrap_or_else(|_| Range::any());
            candidates
                .iter()
                .filter(|c| {
                    if let Ok(ver) = Version::parse(&c.version) {
                        req.satisfies(&ver)
                    } else {
                        false
                    }
                })
                .max_by(|a, b| {
                    let ver_a = Version::parse(&a.version).unwrap();
                    let ver_b = Version::parse(&b.version).unwrap();
                    ver_a.partial_cmp(&ver_b).unwrap()
                })
        } else {
            // If no version specified, pick the highest version available
            candidates.iter().max_by(|a, b| {
                let ver_a = Version::parse(&a.version).unwrap();
                let ver_b = Version::parse(&b.version).unwrap();
                ver_a.partial_cmp(&ver_b).unwrap()
            })
        };

        let artifact = match best_match {
            Some(a) => a,
            None => {
                debug::warning!(
                    "No matching artifact found for root package {}",
                    package.name
                );
                return Ok(());
            }
        };

        // Source path in global cache
        let source_dir =
            get_package_cache_dir().join(format!("{}-{}/package", artifact.name, artifact.version));

        // Destination path in node_modules
        let cwd = std::env::current_dir()?;
        let node_modules = cwd.join("node_modules");
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

        Ok(())
    }
}

impl Pipeline<()> for LinkerPipe {
    async fn run(&self) -> Result<()> {
        // Ensure node_modules exists
        let cwd = std::env::current_dir()?;
        let node_modules = cwd.join("node_modules");
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
        let root_packages = self.root_packages.clone();
        let linking_results: Vec<Result<()>> = stream::iter(root_packages)
            .map(|pkg| {
                let map = artifact_map.clone();
                async move { self.link_root_package(&pkg, &map).await }
            })
            .buffer_unordered(10)
            .collect()
            .await;

        for result in linking_results {
            result?;
        }

        Ok(())
    }
}
