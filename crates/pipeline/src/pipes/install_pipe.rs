use std::collections::HashMap;

use contract::{Pipeline, Result};
use futures::stream::{self, StreamExt};
use package::InstallPackage;
use resolver::{ResolvedArtifact, Resolver};
use tokio::sync::RwLock;

pub struct InstallPipe {
    packages: Vec<InstallPackage>,
    resolver: Resolver,
    locked_packages: RwLock<HashMap<String, ResolvedArtifact>>,
}

impl InstallPipe {
    pub fn new(packages: Vec<InstallPackage>) -> Self {
        Self {
            packages,
            resolver: Resolver::new(),
            locked_packages: RwLock::new(HashMap::new()),
        }
    }

    async fn resolve_package(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        debug::info!("Resolving package: {package:?}");

        let is_locked = self
            .locked_packages
            .read()
            .await
            .contains_key(&package.to_cache_key());

        if is_locked {
            debug::info!("Package {} is already locked", package.to_cache_key());

            let artifact = self
                .locked_packages
                .read()
                .await
                .get(&package.to_cache_key())
                .expect("Package not found in locked packages")
                .clone();

            return Ok(artifact);
        }

        let artifact = self.resolver.resolve(package).await?;
        self.locked_packages
            .write()
            .await
            .insert(package.to_cache_key(), artifact.clone());

        self.resolver.download(&artifact).await?;

        Ok(artifact)
    }
}

impl Pipeline<()> for InstallPipe {
    async fn run(&self) -> Result<()> {
        debug::trace!("Installing packages: {pkgs:?}", pkgs = self.packages);

        // Clone packages to avoid lifetime issues with async closures
        let packages = self.packages.clone();

        // Calculate concurrency limit: CPU cores * 2, with a minimum of 2
        let concurrency = std::thread::available_parallelism()
            .map(|n| n.get() * 2)
            .unwrap_or(4)
            .max(2);

        debug::trace!(
            "Processing packages with concurrency limit: {}",
            concurrency
        );

        // Process packages in parallel with dynamic concurrency limit
        let results: Vec<Result<ResolvedArtifact>> = stream::iter(packages)
            .map(|pkg| async move { self.resolve_package(&pkg).await })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        // Check for any errors
        for result in results {
            result?;
        }

        Ok(())
    }
}
