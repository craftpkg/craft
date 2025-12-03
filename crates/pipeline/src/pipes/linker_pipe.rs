use std::collections::HashMap;
use std::sync::Arc;

use contract::{Pipeline, Result, get_package_cache_dir};
use futures::stream::{self, StreamExt};
use node_semver::{Range, Version};
use package::{InstallPackage, PackageBin};
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
                            let ver_a = Version::parse(&a.version).expect("should not panic");
                            let ver_b = Version::parse(&b.version).expect("should not panic");
                            ver_a.partial_cmp(&ver_b).expect("should not panic")
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
                    let ver_a = Version::parse(&a.version).expect("should not panic");
                    let ver_b = Version::parse(&b.version).expect("should not panic");
                    ver_a.partial_cmp(&ver_b).expect("should not panic")
                })
        } else {
            // If no version specified, pick the highest version available
            candidates.iter().max_by(|a, b| {
                let ver_a = Version::parse(&a.version).expect("should not panic");
                let ver_b = Version::parse(&b.version).expect("should not panic");
                ver_a.partial_cmp(&ver_b).expect("should not panic")
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
        if artifact.package.is_none() {
            return Ok(());
        }
        let pkg_json = artifact.package.as_ref().expect("package is not present");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_link_binaries() -> Result<()> {
        let temp_home = tempdir()?;
        let temp_cwd = tempdir()?;

        // Mock HOME
        unsafe {
            env::set_var("HOME", temp_home.path());
        }
        env::set_current_dir(&temp_cwd)?;

        // Setup cache
        let cache_dir = temp_home.path().join(".craft").join("packages");
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
            integrity: Some("sha512-test".to_string()),
            package: Some(pkg_json),
        };

        let root_pkg =
            InstallPackage::new(pkg_name.to_string(), Some(pkg_version.to_string()), false);

        let pipe = LinkerPipe::new(vec![artifact], vec![root_pkg]);
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

        // Handle /var vs /private/var on macOS
        let target_str = target.to_string_lossy();
        let expected_str = expected_target.to_string_lossy();

        if target_str.starts_with("/private/var") && expected_str.starts_with("/var") {
            assert_eq!(target_str, format!("/private{}", expected_str));
        } else {
            assert_eq!(target, expected_target);
        }

        Ok(())
    }
}
