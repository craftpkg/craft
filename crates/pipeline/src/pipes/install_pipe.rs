use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use contract::{Pipeline, Result, get_package_cache_dir};
use futures::stream::{self, StreamExt};
use package::InstallPackage;
use resolver::{ResolvedArtifact, Resolver};
use tarball::gzip::unzip;
use tokio::sync::Mutex;

type LockedPackage = Arc<Mutex<HashMap<String, Arc<Mutex<Option<ResolvedArtifact>>>>>>;

pub struct InstallPipe {
    packages: Vec<InstallPackage>,
    resolver: Resolver,
    // - None means resolution is in progress
    // - Some(artifact) means resolution is complete
    locked_packages: LockedPackage,
    // Lock for unzipping artifacts to prevent race conditions
    unzip_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl InstallPipe {
    pub fn new(packages: Vec<InstallPackage>) -> Self {
        Self {
            packages,
            resolver: Resolver::new(),
            locked_packages: Arc::new(Mutex::new(HashMap::new())),
            unzip_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[async_recursion::async_recursion]
    async fn resolve_package(&self, package: &InstallPackage) -> Result<()> {
        debug::info!("Resolving package: {package:?}");

        let cache_key = package.to_cache_key();

        // Get or create a lock for this specific package
        let package_lock = {
            let mut cache = self.locked_packages.lock().await;

            // If already cached, return immediately
            if let Some(existing_lock) = cache.get(&cache_key) {
                existing_lock.clone()
            } else {
                // Create a new lock with None (indicating work in progress)
                let new_lock = Arc::new(Mutex::new(None));
                cache.insert(cache_key.clone(), new_lock.clone());
                new_lock
            }
        };

        // Lock this specific package (wait if another thread is working on it)
        let mut artifact_slot = package_lock.lock().await;

        // If already resolved by another thread, return it
        if let Some(_artifact) = artifact_slot.as_ref() {
            debug::info!("Package {} already resolved by another thread", cache_key);
            return Ok(());
        }

        // This thread won the race - do the actual work
        debug::info!("This thread will resolve {}", cache_key);
        let artifact = self.resolver.resolve(package).await?;
        let download_artifact = self.resolver.download(&artifact).await?;

        // Store the result so other threads can use it
        *artifact_slot = Some(artifact.clone());

        // Release lock before recursion to avoid deadlocks
        drop(artifact_slot);

        // Synchronize unzipping
        let unzip_key = format!("{}@{}", artifact.name, artifact.version);
        let unzip_lock = {
            let mut locks = self.unzip_locks.lock().await;
            locks
                .entry(unzip_key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _unzip_guard = unzip_lock.lock().await;

        let unzip_dir =
            get_package_cache_dir().join(format!("{}-{}", artifact.name, artifact.version));

        if !unzip_dir.exists() {
            unzip(download_artifact.path, unzip_dir).await?;
        } else {
            debug::info!("Package {} already unzipped", artifact.name);
        }
        drop(_unzip_guard);

        if let Some(deps) = artifact.package.and_then(|p| p.dependencies) {
            debug::info!("Installing dependencies for {}: {:?}", artifact.name, deps);

            let dep_packages: Vec<InstallPackage> = deps
                .into_iter()
                .map(|(name, version)| InstallPackage::new(name, Some(version), false))
                .collect();

            // Process dependencies in parallel
            let results: Vec<Result<()>> = stream::iter(dep_packages)
                .map(|pkg| async move { self.resolve_package(&pkg).await })
                .buffer_unordered(10) // Concurrency limit for dependencies
                .collect()
                .await;

            for result in results {
                result?;
            }
        }

        Ok(())
    }
}

impl Pipeline<Vec<ResolvedArtifact>> for InstallPipe {
    async fn run(&self) -> Result<Vec<ResolvedArtifact>> {
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
        let results: Vec<Result<()>> = stream::iter(packages)
            .map(|pkg| async move { self.resolve_package(&pkg).await })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        // Check for any errors
        for result in results {
            result?;
        }

        // Collect all resolved artifacts and deduplicate
        let locked = self.locked_packages.lock().await;
        let mut artifacts = Vec::new();
        let mut seen = HashSet::new();

        for lock in locked.values() {
            let artifact_guard = lock.lock().await;
            if let Some(artifact) = artifact_guard.as_ref() {
                let key = format!("{}@{}", artifact.name, artifact.version);
                if !seen.contains(&key) {
                    seen.insert(key);
                    artifacts.push(artifact.clone());
                }
            }
        }

        Ok(artifacts)
    }
}
