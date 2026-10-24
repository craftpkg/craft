use crate::sources::{
    resolve_directory, resolve_link, resolve_local_tarball, resolve_path, resolve_remote_tarball,
};
use crate::{ArtifactSource, NpmResolver, ResolvedArtifact, download_artifact::DownloadArtifact};
use anyhow::{Context as _, anyhow};
use contract::{Context, Result, SpecError};
use network::Network;
use package::{DependencySpec, InstallPackage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Resolver {
    npm_resolver: NpmResolver,
    network: Network,
    context: Context,
    cache_dir: PathBuf,
    // File-level locks to prevent concurrent downloads to the same file
    download_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl Resolver {
    pub fn new(context: &Context) -> Self {
        Self {
            npm_resolver: NpmResolver::new(context.registry.clone()),
            network: Network::new(),
            context: context.clone(),
            cache_dir: context.package_cache_dir(),
            download_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Resolves registry specs against the registry; local and URL specs are copied or
    /// extracted into the cache here, since their name, version and dependencies come from
    /// their own package.json.
    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        let spec = DependencySpec::parse(&package.name, package.version.as_deref())?;
        let requested_name = package.version.is_some().then_some(package.name.as_str());
        let base_dir = package.base_dir.as_deref().unwrap_or(&self.context.cwd);

        match &spec {
            DependencySpec::Registry { name, requirement }
            | DependencySpec::Alias {
                name, requirement, ..
            } => {
                debug::info!("Resolving npm package: {name}@{requirement}");
                self.npm_resolver.resolve(name, requirement).await
            }
            DependencySpec::File { path, .. } => {
                let path = resolve_path(base_dir, path);
                if path.is_file() {
                    resolve_local_tarball(&self.context, requested_name, path).await
                } else {
                    resolve_directory(&self.context, requested_name, path).await
                }
            }
            DependencySpec::Link { path, .. } => {
                resolve_link(requested_name, resolve_path(base_dir, path)).await
            }
            DependencySpec::Tarball { url, .. } => {
                resolve_remote_tarball(&self.context, &self.network, requested_name, url).await
            }
            DependencySpec::Git { .. } | DependencySpec::Workspace { .. } => {
                Err(SpecError::Unsupported {
                    kind: spec.kind(),
                    dependency: package.to_string(),
                }
                .into())
            }
        }
    }

    pub async fn download(
        &self,
        artifact: &ResolvedArtifact,
    ) -> contract::Result<DownloadArtifact> {
        let ArtifactSource::Registry { tarball_url } = &artifact.source else {
            return Err(anyhow!("{artifact} is not a registry package"));
        };
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

        if is_cached(&file_path, artifact).await {
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
            .download(tarball_url, file_path.clone(), artifact.integrity.as_ref())
            .await
            .with_context(|| {
                format!("failed to download {}@{}", artifact.name, artifact.version)
            })?;

        debug::info!("Successfully downloaded {}", artifact.name);

        Ok(DownloadArtifact {
            key: artifact.to_cache_key(),
            path: file_path,
        })
    }
}

/// A cached tarball counts only if it still matches the artifact's integrity; a mismatching one
/// is downloaded again and replaced.
async fn is_cached(file_path: &Path, artifact: &ResolvedArtifact) -> bool {
    let Some(integrity) = &artifact.integrity else {
        return file_path.exists();
    };
    let Ok(bytes) = tokio::fs::read(file_path).await else {
        return false;
    };

    let is_intact = integrity.verify(&bytes).is_ok();
    if !is_intact {
        debug::warning!("Cached {} does not match its integrity", artifact);
    }
    is_intact
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_context() -> Context {
        Context::new(
            std::env::temp_dir(),
            std::env::temp_dir().join(".craft"),
            false,
        )
    }

    #[tokio::test]
    async fn test_unsupported_specs_are_reported() {
        let resolver = Resolver::new(&temp_context());
        let cases = [
            (
                "repo",
                Some("github:user/repo"),
                "git dependencies are not supported yet: repo@github:user/repo",
            ),
            (
                "lib",
                Some("workspace:*"),
                "workspace: dependencies are not supported yet: lib@workspace:*",
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
        let resolver = Resolver::new(&temp_context());
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
        let resolver = Resolver::new(&temp_context());

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
        let resolver = Resolver::new(&temp_context());

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
        let resolver = Resolver::new(&temp_context());

        let artifact = resolver
            .resolve(&pkg)
            .await
            .expect("Failed to resolve npm alias");

        assert_eq!(artifact.name, "wrap-ansi");
        assert!(artifact.version.starts_with("7."));
    }
}
