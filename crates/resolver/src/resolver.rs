use crate::{GitResolver, NpmResolver, ResolvedArtifact, download_artifact::DownloadArtifact};
use contract::{Result, get_package_cache_dir};
use network::Network;
use package::InstallPackage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Resolver {
    npm_resolver: NpmResolver,
    git_resolver: GitResolver,
    network: Network,
    // File-level locks to prevent concurrent downloads to the same file
    download_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            npm_resolver: NpmResolver::new(),
            git_resolver: GitResolver::new(),
            network: Network::new(),
            download_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        if package.is_git() {
            debug::info!("Resolving git package: {}", package.name);
            self.git_resolver.resolve(package)
        } else {
            debug::info!(
                "Resolving npm package: {} {}",
                package.name,
                package.version.clone().unwrap_or("".to_string()),
            );
            self.npm_resolver.resolve(package).await
        }
    }

    pub async fn download(
        &self,
        artifact: &ResolvedArtifact,
    ) -> contract::Result<DownloadArtifact> {
        let cache_dir = get_package_cache_dir();

        tokio::fs::create_dir_all(&cache_dir).await?;

        // Construct the file path: ~/.craft/packages/{name}-{version}.tgz
        let filename = format!("{}-{}.tgz", artifact.name, artifact.version);
        let file_path = cache_dir.join(&filename);

        // Get or create a lock for this specific file
        let file_lock = {
            let mut locks = self.download_locks.lock().await;
            locks
                .entry(filename.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Acquire the file-specific lock
        let _guard = file_lock.lock().await;

        // Re-check if file exists after acquiring lock (another thread might have downloaded it)
        if file_path.exists() {
            debug::info!(
                "Package {} already downloaded at: {:?}",
                artifact.name,
                file_path
            );
            return Ok(DownloadArtifact {
                key: artifact.to_cache_key(),
                path: file_path,
            });
        }

        debug::info!("Downloading {} to: {:?}", artifact.name, file_path);

        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        self.network
            .download(&artifact.download_url, file_path.clone())
            .await?;

        debug::info!("Successfully downloaded {}", artifact.name);

        Ok(DownloadArtifact {
            key: artifact.to_cache_key(),
            path: file_path,
        })
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_npm_package() {
        let pkg = InstallPackage::new("react".to_string(), Some("17.0.2".to_string()), false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve npm package");
        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
    }

    #[tokio::test]
    async fn test_resolve_git_package() {
        let pkg = InstallPackage::new("git:github.com/user/repo.git".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve git package");
        assert_eq!(artifact.name, "repo");
        assert_eq!(artifact.version, "git");
    }

    #[tokio::test]
    async fn test_resolve_npm_latest() {
        let pkg = InstallPackage::new("express".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve express package");
        assert_eq!(artifact.name, "express");
        assert!(!artifact.version.is_empty());
    }

    #[tokio::test]
    async fn test_resolve_gitlab_package() {
        let pkg = InstallPackage::new("git:gitlab.com/package/psc".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve gitlab package");
        assert_eq!(artifact.name, "psc");
    }
}
