use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use contract::{Context, Pipeline, Result};
use futures::stream::{self, StreamExt};
use package::InstallPackage;
use resolver::{
    DependencyKind, Edge, NodeId, ResolutionGraph, ResolvedArtifact, ResolvedNode, Resolver,
};
use tarball::gzip::unzip;
use tokio::sync::Mutex;

const DEPENDENCY_CONCURRENCY: usize = 10;

type RequestSlot = Arc<Mutex<Option<NodeId>>>;

pub struct InstallPipe {
    packages: Vec<InstallPackage>,
    cache_dir: PathBuf,
    resolver: Resolver,
    // One slot per distinct name + spec; it holds the node that request resolved to.
    requests: Mutex<HashMap<String, RequestSlot>>,
    // Nodes already being fetched, so each version is downloaded and walked once.
    claimed_nodes: Mutex<HashSet<NodeId>>,
    nodes: Mutex<BTreeMap<NodeId, ResolvedNode>>,
}

impl InstallPipe {
    pub fn new(context: &Context, packages: Vec<InstallPackage>) -> Self {
        let cache_dir = context.package_cache_dir();

        Self {
            packages,
            resolver: Resolver::new(cache_dir.clone(), context.registry.clone()),
            cache_dir,
            requests: Mutex::new(HashMap::new()),
            claimed_nodes: Mutex::new(HashSet::new()),
            nodes: Mutex::new(BTreeMap::new()),
        }
    }

    #[async_recursion::async_recursion]
    async fn resolve_package(&self, package: &InstallPackage) -> Result<NodeId> {
        let slot = {
            let mut requests = self.requests.lock().await;
            requests.entry(package.to_cache_key()).or_default().clone()
        };

        let mut resolved = slot.lock().await;
        if let Some(node_id) = resolved.as_ref() {
            return Ok(node_id.clone());
        }

        debug::info!("Resolving package: {package:?}");
        let artifact = self.resolver.resolve(package).await?;
        let node_id = NodeId::new(&artifact.name, &artifact.version);
        *resolved = Some(node_id.clone());
        // Released before recursing so cyclic dependencies find this node instead of waiting on it.
        drop(resolved);

        let is_first_request_for_node = self.claimed_nodes.lock().await.insert(node_id.clone());
        if !is_first_request_for_node {
            return Ok(node_id);
        }

        self.fetch(&artifact).await?;
        let dependencies = self.resolve_dependencies(&artifact).await?;

        self.nodes.lock().await.insert(
            node_id.clone(),
            ResolvedNode {
                artifact,
                dependencies,
            },
        );
        Ok(node_id)
    }

    async fn fetch(&self, artifact: &ResolvedArtifact) -> Result<()> {
        let download_artifact = self.resolver.download(artifact).await?;
        let unzip_dir = self
            .cache_dir
            .join(format!("{}-{}", artifact.name, artifact.version));

        if unzip_dir.exists() {
            debug::info!("Package {} already unzipped", artifact.name);
            return Ok(());
        }
        unzip(download_artifact.path, unzip_dir).await
    }

    async fn resolve_dependencies(&self, artifact: &ResolvedArtifact) -> Result<Vec<Edge>> {
        let Some(dependencies) = artifact
            .package
            .as_ref()
            .and_then(|package| package.dependencies.clone())
        else {
            return Ok(Vec::new());
        };
        debug::info!(
            "Resolving dependencies for {}: {:?}",
            artifact.name,
            dependencies
        );

        let results: Vec<Result<Edge>> = stream::iter(dependencies)
            .map(|(name, spec)| async move {
                let package = InstallPackage::new(name.clone(), Some(spec.clone()), false);
                let target = self.resolve_package(&package).await?;
                Ok(Edge {
                    name,
                    spec: Some(spec),
                    target,
                    kind: DependencyKind::Production,
                })
            })
            .buffer_unordered(DEPENDENCY_CONCURRENCY)
            .collect()
            .await;

        let mut edges = results.into_iter().collect::<Result<Vec<_>>>()?;
        edges.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(edges)
    }
}

impl Pipeline<ResolutionGraph> for InstallPipe {
    async fn run(&self) -> Result<ResolutionGraph> {
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

        let results: Vec<Result<Edge>> = stream::iter(packages)
            .map(|package| async move {
                let target = self.resolve_package(&package).await?;
                let kind = if package.is_dev {
                    DependencyKind::Development
                } else {
                    DependencyKind::Production
                };
                Ok(Edge {
                    name: package.name,
                    spec: package.version,
                    target,
                    kind,
                })
            })
            .buffered(concurrency)
            .collect()
            .await;

        let roots = results.into_iter().collect::<Result<Vec<_>>>()?;
        let nodes = self.nodes.lock().await.clone();

        Ok(ResolutionGraph { roots, nodes })
    }
}
