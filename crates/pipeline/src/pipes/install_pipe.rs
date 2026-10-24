use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use contract::{Context, Pipeline, Result};
use futures::stream::{self, StreamExt};
use lockfile::Lockfile;
use package::{DependencySpec, InstallPackage, PackageJson};
use resolver::{
    ArtifactSource, DependencyKind, Edge, NodeId, PeerDependency, ResolutionGraph,
    ResolvedArtifact, ResolvedNode, Resolver,
};

use super::peers::resolve_peers;
use tarball::gzip::unzip;
use tokio::sync::Mutex;

const DEPENDENCY_CONCURRENCY: usize = 10;

type RequestSlot = Arc<Mutex<Option<NodeId>>>;

pub struct InstallPipe {
    packages: Vec<InstallPackage>,
    context: Context,
    resolver: Resolver,
    // One slot per distinct name + spec; it holds the node that request resolved to.
    requests: Mutex<HashMap<String, RequestSlot>>,
    // Nodes already being fetched, so each version is downloaded and walked once.
    claimed_nodes: Mutex<HashSet<NodeId>>,
    nodes: Mutex<BTreeMap<NodeId, ResolvedNode>>,
}

impl InstallPipe {
    pub fn new(context: &Context, packages: Vec<InstallPackage>) -> Self {
        Self {
            packages,
            context: context.clone(),
            resolver: Resolver::new(context),
            requests: Mutex::new(HashMap::new()),
            claimed_nodes: Mutex::new(HashSet::new()),
            nodes: Mutex::new(BTreeMap::new()),
        }
    }

    /// Resolves against `lockfile` first, so locked versions need no registry metadata.
    pub fn with_lockfile(mut self, lockfile: Option<&Lockfile>) -> Self {
        if let Some(lockfile) = lockfile {
            self.resolver = self.resolver.with_lockfile(lockfile);
        }
        self
    }

    /// Resolves only from the lockfile; anything it doesn't record is an error.
    pub fn frozen(mut self) -> Self {
        self.resolver = self.resolver.frozen();
        self
    }

    #[async_recursion::async_recursion]
    async fn resolve_package(
        &self,
        package: &InstallPackage,
        ancestry: &[NodeId],
    ) -> Result<NodeId> {
        let slot = {
            let mut requests = self.requests.lock().await;
            requests.entry(package.to_cache_key()).or_default().clone()
        };

        let mut resolved = slot.lock().await;
        if let Some(node_id) = resolved.as_ref() {
            return Ok(node_id.clone());
        }

        debug::info!("Resolving package: {package:?}");
        let artifact = self
            .resolver
            .resolve(package)
            .await
            .with_context(|| failure_message(package, ancestry))?;
        let node_id = artifact.node_id();
        *resolved = Some(node_id.clone());
        // Released before recursing so cyclic dependencies find this node instead of waiting on it.
        drop(resolved);

        let is_first_request_for_node = self.claimed_nodes.lock().await.insert(node_id.clone());
        if !is_first_request_for_node {
            return Ok(node_id);
        }

        let is_supported = artifact.platform.supports(&self.context.platform);
        if !is_supported {
            debug::info!("Not fetching {node_id}: not for {}", self.context.platform);
            let node = ResolvedNode {
                artifact,
                dependencies: Vec::new(),
                peers: Vec::new(),
            };
            self.record(node_id.clone(), node).await;
            return Ok(node_id);
        }

        let artifact = self
            .fetch(artifact)
            .await
            .with_context(|| failure_message(package, ancestry))?;
        let ancestry = [ancestry, std::slice::from_ref(&node_id)].concat();
        let dependencies = self.resolve_dependencies(&artifact, &ancestry).await?;
        let peers = self
            .resolve_peer_candidates(&artifact, &dependencies, &ancestry)
            .await;

        let node = ResolvedNode {
            artifact,
            dependencies,
            peers,
        };
        self.record(node_id.clone(), node).await;
        Ok(node_id)
    }

    /// Records the package's peers and resolves each required one on its own, so it can be
    /// installed when nothing above the package provides it. A peer the package also lists as a
    /// dependency is just that dependency. A candidate that fails to resolve is not an error:
    /// the peer is usually provided.
    async fn resolve_peer_candidates(
        &self,
        artifact: &ResolvedArtifact,
        dependencies: &[Edge],
        ancestry: &[NodeId],
    ) -> Vec<PeerDependency> {
        let Some(manifest) = &artifact.package else {
            return Vec::new();
        };
        let is_link = matches!(artifact.source, ArtifactSource::Link { .. });
        if is_link {
            return Vec::new();
        }

        let declared = manifest
            .peer_dependencies()
            .into_iter()
            .filter(|peer| !dependencies.iter().any(|edge| edge.name == peer.name));
        let resolved = declared.map(|peer| async move {
            let candidate = if peer.optional {
                None
            } else {
                let package =
                    InstallPackage::new(peer.name.clone(), Some(peer.range.clone()), false);
                match self.resolve_package(&package, ancestry).await {
                    Ok(node_id) => Some(node_id),
                    Err(error) => {
                        debug::warning!(
                            "Peer {} of {} not resolvable: {error:#}",
                            peer.name,
                            artifact.name
                        );
                        None
                    }
                }
            };
            PeerDependency {
                name: peer.name,
                range: peer.range,
                optional: peer.optional,
                candidate,
            }
        });
        stream::iter(resolved)
            .buffered(DEPENDENCY_CONCURRENCY)
            .collect()
            .await
    }

    async fn record(&self, node_id: NodeId, node: ResolvedNode) {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(node_id, node);
        let count = nodes.len();
        let noun = if count == 1 { "package" } else { "packages" };
        self.context
            .reporter
            .status(format!("Resolving dependencies… {count} {noun}"));
    }

    /// Downloads and extracts a registry package into the store; other sources are
    /// materialized while resolving. A package resolved from craft.lock arrives without its
    /// manifest, which is then read from the extracted package.
    async fn fetch(&self, mut artifact: ResolvedArtifact) -> Result<ResolvedArtifact> {
        let is_registry_package = matches!(artifact.source, ArtifactSource::Registry { .. });
        if !is_registry_package {
            return Ok(artifact);
        }

        let package_dir = artifact.package_dir(&self.context);
        if !package_dir.exists() {
            let download_artifact = self.resolver.download(&artifact).await?;
            let extracted = unzip(download_artifact.path.clone(), package_dir.clone()).await;
            let _ = tokio::fs::remove_file(&download_artifact.path).await;
            extracted?;
        }

        if artifact.package.is_none() {
            artifact.package =
                Some(PackageJson::from_file(&package_dir.join("package.json")).await?);
        }
        Ok(artifact)
    }

    async fn resolve_dependencies(
        &self,
        artifact: &ResolvedArtifact,
        ancestry: &[NodeId],
    ) -> Result<Vec<Edge>> {
        let is_link = matches!(artifact.source, ArtifactSource::Link { .. });
        if is_link {
            return Ok(Vec::new());
        }
        let Some(manifest) = &artifact.package else {
            return Ok(Vec::new());
        };
        let mut declared: BTreeMap<String, (String, DependencyKind)> = manifest
            .dependencies
            .iter()
            .flatten()
            .map(|(name, spec)| (name.clone(), (spec.clone(), DependencyKind::Production)))
            .collect();
        for (name, spec) in manifest.optional_dependencies() {
            declared.insert(name, (spec, DependencyKind::Optional));
        }
        let base_dir = &dependency_base_dir(artifact, &self.context);
        debug::info!(
            "Resolving dependencies for {}: {:?}",
            artifact.name,
            declared
        );

        let results: Vec<Result<Option<Edge>>> = stream::iter(declared)
            .map(|(name, (spec, kind))| async move {
                let package = InstallPackage::new(name.clone(), Some(spec.clone()), false)
                    .with_base_dir(base_dir.clone());
                let target = match self.resolve_package(&package, ancestry).await {
                    Ok(target) => target,
                    Err(error) if kind == DependencyKind::Optional => {
                        self.context.reporter.warn(format!(
                            "skipping optional dependency {package} of {}: {error:#}",
                            artifact.name
                        ));
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                };
                Ok(Some(Edge {
                    name,
                    spec: Some(spec),
                    target,
                    kind,
                }))
            })
            .buffer_unordered(DEPENDENCY_CONCURRENCY)
            .collect()
            .await;

        let mut edges: Vec<Edge> = results
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
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

        let results: Vec<Result<Option<Edge>>> = stream::iter(packages)
            .map(|package| async move {
                let target = match self.resolve_package(&package, &[]).await {
                    Ok(target) => target,
                    Err(error) if package.is_optional => {
                        self.context
                            .reporter
                            .warn(format!("skipping optional dependency {package}: {error:#}"));
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                };
                let kind = if package.is_optional {
                    DependencyKind::Optional
                } else if package.is_dev {
                    DependencyKind::Development
                } else {
                    DependencyKind::Production
                };
                Ok(Some(Edge {
                    name: package.name,
                    spec: package.version,
                    target,
                    kind,
                }))
            })
            .buffered(concurrency)
            .collect()
            .await;

        let mut roots: Vec<Edge> = results
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let nodes = self.nodes.lock().await.clone();
        for root in &mut roots {
            name_bare_location(root, &nodes);
        }

        let resolved = resolve_peers(&ResolutionGraph { roots, nodes });
        for warning in &resolved.warnings {
            self.context.reporter.warn(warning);
        }
        Ok(resolved.graph)
    }
}

fn failure_message(package: &InstallPackage, ancestry: &[NodeId]) -> String {
    if ancestry.is_empty() {
        return format!("failed to install {package}");
    }

    let chain = ancestry
        .iter()
        .map(NodeId::to_string)
        .collect::<Vec<_>>()
        .join(" > ");
    format!("failed to install {package} (required by {chain})")
}

/// Where a package's relative `file:` and `link:` dependencies point from: the source directory
/// for local packages, the extracted package otherwise.
fn dependency_base_dir(artifact: &ResolvedArtifact, context: &Context) -> PathBuf {
    match &artifact.source {
        ArtifactSource::Directory { path } | ArtifactSource::Link { path } => path.clone(),
        _ => artifact.package_dir(context),
    }
}

/// `craft add ./libs/x` requests the path itself; the root is named after the package found
/// there and keeps the location as its spec.
fn name_bare_location(root: &mut Edge, nodes: &BTreeMap<NodeId, ResolvedNode>) {
    if root.spec.is_some() {
        return;
    }
    let location = DependencySpec::parse(&root.name, None)
        .ok()
        .and_then(|spec| spec.location_spec());
    let (Some(location), Some(node)) = (location, nodes.get(&root.target)) else {
        return;
    };

    root.name = node.artifact.name.clone();
    root.spec = Some(location);
}
