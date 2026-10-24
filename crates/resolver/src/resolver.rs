use crate::{NpmResolver, ResolvedArtifact, download_artifact::DownloadArtifact};
use contract::{Result, SpecError};
use network::Network;
use package::{DependencySpec, InstallPackage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Resolver {
    npm_resolver: NpmResolver,
    network: Network,
    cache_dir: PathBuf,
    // File-level locks to prevent concurrent downloads to the same file
    download_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl Resolver {
    pub fn new(cache_dir: PathBuf, registry: String) -> Self {
        Self {
            npm_resolver: NpmResolver::new(registry),
            network: Network::new(),
            cache_dir,
            download_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        let spec = DependencySpec::parse(&package.name, package.version.as_deref())?;

        let Some((name, requirement)) = spec.registry_target() else {
            let dependency = match &package.version {
                Some(version) => format!("{}@{}", package.name, version),
                None => package.name.clone(),
            };
            return Err(SpecError::Unsupported {
                kind: spec.kind(),
                dependency,
            }
            .into());
        };

        debug::info!("Resolving npm package: {name}@{requirement}");
        self.npm_resolver.resolve(name, requirement).await
    }

    pub async fn download(
        &self,
        artifact: &ResolvedArtifact,
    ) -> contract::Result<DownloadArtifact> {
        tokio::fs::create_dir_all(&self.cache_dir).await?;

        // Construct the file path: ~/.craft/packages/{name}-{version}.tgz
        let filename = format!("{}-{}.tgz", artifact.name, artifact.version);
        let file_path = self.cache_dir.join(&filename);

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

#[cfg(test)]
mod tests {
    use super::*;
    use contract::DEFAULT_REGISTRY;

    #[tokio::test]
    async fn test_unsupported_specs_are_reported() {
        let resolver = Resolver::new(std::env::temp_dir(), DEFAULT_REGISTRY.to_string());
        let cases = [
            (
                "repo",
                Some("github:user/repo"),
                "git dependencies are not supported yet: repo@github:user/repo",
            ),
            (
                "local",
                Some("file:../local"),
                "file: dependencies are not supported yet: local@file:../local",
            ),
            (
                "git+https://github.com/user/repo.git",
                None,
                "git dependencies are not supported yet: git+https://github.com/user/repo.git",
            ),
        ];

        for (name, spec, message) in cases {
            let pkg = InstallPackage::new(name.to_string(), spec.map(str::to_string), false);
            let error = resolver.resolve(&pkg).await.unwrap_err();
            assert_eq!(error.to_string(), message);
        }
    }

    #[tokio::test]
    async fn test_invalid_spec_is_reported() {
        let resolver = Resolver::new(std::env::temp_dir(), DEFAULT_REGISTRY.to_string());
        let pkg = InstallPackage::new(
            "react".to_string(),
            Some("not a version".to_string()),
            false,
        );

        let error = resolver.resolve(&pkg).await.unwrap_err();

        assert_eq!(
            error.to_string(),
            "invalid dependency spec: react@not a version"
        );
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_npm_package() {
        let pkg = InstallPackage::new("react".to_string(), Some("17.0.2".to_string()), false);
        let resolver = Resolver::new(std::env::temp_dir(), DEFAULT_REGISTRY.to_string());

        let artifact = resolver
            .resolve(&pkg)
            .await
            .expect("Failed to resolve npm package");

        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_npm_latest() {
        let pkg = InstallPackage::new("express".to_string(), None, false);
        let resolver = Resolver::new(std::env::temp_dir(), DEFAULT_REGISTRY.to_string());

        let artifact = resolver
            .resolve(&pkg)
            .await
            .expect("Failed to resolve express package");

        assert_eq!(artifact.name, "express");
        assert!(!artifact.version.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_npm_alias() {
        let pkg = InstallPackage::new(
            "wrap-ansi-cjs".to_string(),
            Some("npm:wrap-ansi@^7.0.0".to_string()),
            false,
        );
        let resolver = Resolver::new(std::env::temp_dir(), DEFAULT_REGISTRY.to_string());

        let artifact = resolver
            .resolve(&pkg)
            .await
            .expect("Failed to resolve npm alias");

        assert_eq!(artifact.name, "wrap-ansi");
        assert!(artifact.version.starts_with("7."));
    }
}
