use crate::locked::LockPreferences;
use crate::sources::{
    resolve_directory, resolve_git, resolve_link, resolve_local_tarball, resolve_path,
    resolve_remote_tarball,
};
use crate::{ArtifactSource, NpmResolver, ResolvedArtifact, download_artifact::DownloadArtifact};
use anyhow::{Context as _, anyhow};
use contract::{Context, Result, SpecError};
use lockfile::Lockfile;
use network::Network;
use package::{DependencySpec, InstallPackage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Resolver {
    npm_resolver: NpmResolver,
    locked: LockPreferences,
    network: Network,
    context: Context,
    // File-level locks to prevent concurrent downloads to the same file
    download_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl Resolver {
    pub fn new(context: &Context) -> Self {
        Self {
            npm_resolver: NpmResolver::new(context.registry.clone()),
            locked: LockPreferences::default(),
            network: Network::new(),
            context: context.clone(),
            download_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_lockfile(mut self, lockfile: &Lockfile) -> Self {
        self.locked = LockPreferences::from_lockfile(lockfile);
        self
    }

    /// Resolves registry specs against craft.lock, then the registry; local and URL specs are copied or
    /// extracted into the store here, since their name, version and dependencies come from
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
                let locked = package
                    .prefer_locked
                    .then(|| self.locked.registry(name, requirement))
                    .flatten();
                if let Some(artifact) = locked {
                    debug::info!("Using locked {name}@{}", artifact.version);
                    return Ok(artifact);
                }
                debug::info!("Resolving npm package: {name}@{requirement}");
                self.npm_resolver.resolve(name, requirement).await
            }
            DependencySpec::File { path, .. } => {
                let path = resolve_path(base_dir, path);
                if path.is_file() {
                    resolve_local_tarball(&self.context, requested_name, path).await
                } else {
                    resolve_directory(requested_name, path).await
                }
            }
            DependencySpec::Link { path, .. } => {
                resolve_link(requested_name, resolve_path(base_dir, path)).await
            }
            DependencySpec::Tarball { url, .. } => {
                resolve_remote_tarball(&self.context, &self.network, requested_name, url).await
            }
            DependencySpec::Git { url, .. } => {
                let locked = package.prefer_locked.then_some(&self.locked);
                resolve_git(&self.context, &self.network, requested_name, url, locked).await
            }
            DependencySpec::Workspace { .. } => Err(SpecError::Unsupported {
                kind: spec.kind(),
                dependency: package.to_string(),
            }
            .into()),
        }
    }

    pub async fn download(
        &self,
        artifact: &ResolvedArtifact,
    ) -> contract::Result<DownloadArtifact> {
        let ArtifactSource::Registry { tarball_url } = &artifact.source else {
            return Err(anyhow!("{artifact} is not a registry package"));
        };
        let file_path = tarball_path(&artifact.package_dir(&self.context));

        let file_lock = {
            let mut locks = self.download_locks.lock().await;
            locks
                .entry(file_path.display().to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = file_lock.lock().await;

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

/// Where a store entry's tarball is downloaded to before extraction: beside the entry, so the
/// rename that publishes it never crosses filesystems.
fn tarball_path(package_dir: &Path) -> PathBuf {
    let mut path = package_dir.as_os_str().to_os_string();
    path.push(".tgz");
    PathBuf::from(path)
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
        let cases = [(
            "lib",
            Some("workspace:*"),
            "workspace: dependencies are not supported yet: lib@workspace:*",
        )];

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
