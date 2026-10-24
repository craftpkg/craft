use std::collections::{BTreeMap, BTreeSet, HashSet};

use node_semver::{Range, Version};
use resolver::{DependencyKind, Edge, NodeId, PeerDependency, ResolutionGraph, ResolvedNode};

pub(crate) struct PeerResolution {
    pub graph: ResolutionGraph,
    pub warnings: Vec<String>,
}

/// Rewrites the graph so every package sees its peers. A package's peer resolves from its
/// parent's `node_modules` — the parent's dependencies and the parent's own peers — falling back
/// to the project's dependencies, and to the peer's own resolution when nobody provides it. A
/// package whose peers resolve differently in two places becomes two instances, identified as
/// `<id>_<peer id>…`, so each gets its own virtual store entry.
///
/// Because a package's scope is exactly its own dependencies and resolved peers, its instance id
/// determines everything below it, which is what makes memoizing by instance sound.
pub(crate) fn resolve_peers(graph: &ResolutionGraph) -> PeerResolution {
    let mut resolver = PeerResolver {
        source: graph,
        instances: BTreeMap::new(),
        in_progress: HashSet::new(),
        resolving: HashSet::new(),
        warnings: BTreeSet::new(),
        next_scope_id: 1,
    };
    let root_scope = Scope {
        id: 0,
        dependencies: &graph.roots,
        peers: BTreeMap::new(),
    };

    let roots = graph
        .roots
        .iter()
        .map(|root| Edge {
            target: resolver.instance(&root_scope, &root_scope, root),
            ..root.clone()
        })
        .collect();

    PeerResolution {
        graph: ResolutionGraph {
            roots,
            nodes: resolver.instances,
        },
        warnings: resolver.warnings.into_iter().collect(),
    }
}

struct PeerResolver<'g> {
    source: &'g ResolutionGraph,
    instances: BTreeMap<NodeId, ResolvedNode>,
    in_progress: HashSet<NodeId>,
    /// `(scope, name)` pairs whose instance is being worked out, to stop siblings that are each
    /// other's peers from recursing forever.
    resolving: HashSet<(usize, String)>,
    warnings: BTreeSet<String>,
    next_scope_id: usize,
}

/// One `node_modules` directory: the owner's dependency edges and its resolved peers.
struct Scope<'g> {
    id: usize,
    dependencies: &'g [Edge],
    peers: BTreeMap<String, NodeId>,
}

impl<'g> PeerResolver<'g> {
    /// The instance `edge` resolves to inside `scope`, building it (and everything below it) the
    /// first time.
    fn instance(&mut self, scope: &Scope<'g>, root_scope: &Scope<'g>, edge: &Edge) -> NodeId {
        let Some(node) = self.source.nodes.get(&edge.target) else {
            return edge.target.clone();
        };

        let key = (scope.id, edge.name.clone());
        let peers = if self.resolving.insert(key.clone()) {
            let peers = self.resolve_node_peers(scope, root_scope, &edge.target, node);
            self.resolving.remove(&key);
            peers
        } else {
            BTreeMap::new()
        };
        self.build(root_scope, edge, node, peers)
    }

    /// Creates the instance of `edge`'s package with `peers`, unless it already exists or is
    /// being created further up the stack (a dependency cycle).
    fn build(
        &mut self,
        root_scope: &Scope<'g>,
        edge: &Edge,
        node: &'g ResolvedNode,
        peers: BTreeMap<String, NodeId>,
    ) -> NodeId {
        let instance_id = edge.target.with_peers(peers.values());
        let is_known = self.instances.contains_key(&instance_id);
        if is_known || !self.in_progress.insert(instance_id.clone()) {
            return instance_id;
        }

        let child_scope = Scope {
            id: self.next_scope_id,
            dependencies: &node.dependencies,
            peers: peers.clone(),
        };
        self.next_scope_id += 1;

        let mut dependencies: Vec<Edge> = node
            .dependencies
            .iter()
            .map(|dependency| Edge {
                target: self.instance(&child_scope, root_scope, dependency),
                ..dependency.clone()
            })
            .collect();
        dependencies.extend(peers.into_iter().map(|(name, target)| Edge {
            name,
            spec: None,
            target,
            kind: DependencyKind::Peer,
        }));
        dependencies.sort_by(|a, b| a.name.cmp(&b.name));

        self.in_progress.remove(&instance_id);
        self.instances.insert(
            instance_id.clone(),
            ResolvedNode {
                artifact: node.artifact.clone(),
                dependencies,
                peers: node.peers.clone(),
            },
        );
        instance_id
    }

    fn resolve_node_peers(
        &mut self,
        scope: &Scope<'g>,
        root_scope: &Scope<'g>,
        node_id: &NodeId,
        node: &ResolvedNode,
    ) -> BTreeMap<String, NodeId> {
        let mut resolved = BTreeMap::new();
        for peer in &node.peers {
            let Some((target, version)) = self.resolve_peer(scope, root_scope, peer) else {
                if !peer.optional {
                    self.warnings.insert(format!(
                        "{node_id} needs peer {}@{}, which could not be resolved",
                        peer.name, peer.range
                    ));
                }
                continue;
            };
            self.check_range(node_id, peer, &version);
            resolved.insert(peer.name.clone(), target);
        }
        resolved
    }

    /// The parent's dependency or peer of that name, then the project's, then the peer's own
    /// resolution (npm ≥ 7 installs missing peers); optional peers are only ever provided.
    /// Returns the peer's instance and version.
    fn resolve_peer(
        &mut self,
        scope: &Scope<'g>,
        root_scope: &Scope<'g>,
        peer: &PeerDependency,
    ) -> Option<(NodeId, String)> {
        if let Some(target) = scope.peers.get(&peer.name) {
            return Some((target.clone(), self.version_of(target)?));
        }
        for candidate_scope in [scope, root_scope] {
            let provided = candidate_scope
                .dependencies
                .iter()
                .find(|edge| edge.name == peer.name);
            if let Some(edge) = provided {
                let version = self.version_of(&edge.target)?;
                return Some((self.instance(candidate_scope, root_scope, edge), version));
            }
        }

        let candidate = peer.candidate.clone()?;
        let version = self.version_of(&candidate)?;
        let edge = Edge {
            name: peer.name.clone(),
            spec: Some(peer.range.clone()),
            target: candidate,
            kind: DependencyKind::Peer,
        };
        Some((self.instance(scope, root_scope, &edge), version))
    }

    /// The version behind a source node or an instance built from one.
    fn version_of(&self, id: &NodeId) -> Option<String> {
        self.source
            .nodes
            .get(id)
            .or_else(|| self.instances.get(id))
            .map(|node| node.artifact.version.clone())
    }

    fn check_range(&mut self, node_id: &NodeId, peer: &PeerDependency, version: &str) {
        let satisfies = match (Range::parse(&peer.range), Version::parse(version)) {
            (Ok(range), Ok(parsed)) => range.satisfies(&parsed),
            _ => true,
        };
        if !satisfies {
            self.warnings.insert(format!(
                "{node_id} wants peer {}@{} but gets {version}",
                peer.name, peer.range
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolver::{ArtifactSource, ResolvedArtifact};

    struct GraphBuilder {
        graph: ResolutionGraph,
    }

    impl GraphBuilder {
        fn new() -> Self {
            Self {
                graph: ResolutionGraph::default(),
            }
        }

        fn package(
            mut self,
            id: &str,
            dependencies: &[&str],
            peers: &[(&str, &str, Option<&str>)],
        ) -> Self {
            let (name, version) = id.rsplit_once('@').unwrap();
            let node = ResolvedNode {
                artifact: ResolvedArtifact {
                    name: name.to_string(),
                    version: version.to_string(),
                    source: ArtifactSource::Registry {
                        tarball_url: String::new(),
                    },
                    integrity: None,
                    package: None,
                },
                dependencies: dependencies.iter().map(|target| edge(target)).collect(),
                peers: peers
                    .iter()
                    .map(|(name, range, candidate)| PeerDependency {
                        name: name.to_string(),
                        range: range.to_string(),
                        optional: candidate.is_none(),
                        candidate: candidate.map(node_id),
                    })
                    .collect(),
            };
            self.graph.nodes.insert(node_id(id), node);
            self
        }

        fn roots(mut self, roots: &[&str]) -> ResolutionGraph {
            self.graph.roots = roots.iter().map(|target| edge(target)).collect();
            self.graph
        }
    }

    fn node_id(id: &str) -> NodeId {
        let (name, version) = id.rsplit_once('@').unwrap();
        NodeId::new(name, version)
    }

    fn edge(target: &str) -> Edge {
        Edge {
            name: target.rsplit_once('@').unwrap().0.to_string(),
            spec: None,
            target: node_id(target),
            kind: DependencyKind::Production,
        }
    }

    fn instance<'a>(resolution: &'a PeerResolution, id: &str) -> &'a ResolvedNode {
        resolution
            .graph
            .nodes
            .iter()
            .find(|(instance_id, _)| instance_id.to_string() == id)
            .map(|(_, node)| node)
            .unwrap_or_else(|| panic!("no instance {id}"))
    }

    fn peer_of(resolution: &PeerResolution, id: &str, peer: &str) -> String {
        instance(resolution, id)
            .dependencies
            .iter()
            .find(|edge| edge.name == peer && edge.kind == DependencyKind::Peer)
            .unwrap_or_else(|| panic!("{id} has no peer {peer}"))
            .target
            .to_string()
    }

    #[test]
    fn test_peer_resolves_to_the_projects_dependency() {
        let graph = GraphBuilder::new()
            .package("react@19.2.0", &[], &[])
            .package(
                "query@5.0.0",
                &[],
                &[("react", "^18 || ^19", Some("react@19.2.0"))],
            )
            .roots(&["react@19.2.0", "query@5.0.0"]);

        let resolution = resolve_peers(&graph);

        assert_eq!(
            resolution.graph.roots[1].target.to_string(),
            "query@5.0.0_react@19.2.0"
        );
        assert_eq!(
            peer_of(&resolution, "query@5.0.0_react@19.2.0", "react"),
            "react@19.2.0"
        );
        assert!(resolution.warnings.is_empty(), "{:?}", resolution.warnings);
    }

    #[test]
    fn test_subtrees_with_different_peers_get_separate_instances() {
        let graph = GraphBuilder::new()
            .package("react@18.3.1", &[], &[])
            .package("react@19.2.0", &[], &[])
            .package("lib@1.0.0", &[], &[("react", "*", Some("react@19.2.0"))])
            .package("app-a@1.0.0", &["react@18.3.1", "lib@1.0.0"], &[])
            .package("app-b@1.0.0", &["react@19.2.0", "lib@1.0.0"], &[])
            .roots(&["app-a@1.0.0", "app-b@1.0.0"]);

        let resolution = resolve_peers(&graph);

        let instances: Vec<String> = resolution
            .graph
            .nodes
            .keys()
            .map(NodeId::to_string)
            .filter(|id| id.starts_with("lib@"))
            .collect();
        assert_eq!(
            instances,
            ["lib@1.0.0_react@18.3.1", "lib@1.0.0_react@19.2.0"]
        );
        let app_a = instance(&resolution, "app-a@1.0.0");
        let lib_of_a = app_a
            .dependencies
            .iter()
            .find(|edge| edge.name == "lib")
            .unwrap();
        assert_eq!(lib_of_a.target.to_string(), "lib@1.0.0_react@18.3.1");
    }

    #[test]
    fn test_missing_peer_is_installed_and_optional_peer_skipped() {
        let graph = GraphBuilder::new()
            .package("react@19.2.0", &[], &[])
            .package(
                "query@5.0.0",
                &[],
                &[
                    ("react", "^19", Some("react@19.2.0")),
                    ("react-native", "*", None),
                ],
            )
            .roots(&["query@5.0.0"]);

        let resolution = resolve_peers(&graph);

        let instance = "query@5.0.0_react@19.2.0";
        assert_eq!(peer_of(&resolution, instance, "react"), "react@19.2.0");
        assert!(
            resolution
                .graph
                .nodes
                .contains_key(&node_id("react@19.2.0"))
        );
        assert!(resolution.warnings.is_empty(), "{:?}", resolution.warnings);
    }

    #[test]
    fn test_unmet_peer_range_warns() {
        let graph = GraphBuilder::new()
            .package("react@17.0.2", &[], &[])
            .package("query@5.0.0", &[], &[("react", "^18 || ^19", None)])
            .roots(&["react@17.0.2", "query@5.0.0"]);

        let resolution = resolve_peers(&graph);

        assert_eq!(
            resolution.warnings,
            ["query@5.0.0 wants peer react@^18 || ^19 but gets 17.0.2"]
        );
    }

    #[test]
    fn test_siblings_that_are_each_others_peers_terminate() {
        let graph = GraphBuilder::new()
            .package("a@1.0.0", &[], &[("b", "*", Some("b@1.0.0"))])
            .package("b@1.0.0", &[], &[("a", "*", Some("a@1.0.0"))])
            .roots(&["a@1.0.0", "b@1.0.0"]);

        let resolution = resolve_peers(&graph);

        assert_eq!(resolution.graph.roots.len(), 2);
        let has_dangling_target = resolution
            .graph
            .nodes
            .values()
            .flat_map(|node| &node.dependencies)
            .any(|edge| !resolution.graph.nodes.contains_key(&edge.target));
        assert!(!has_dangling_target);
    }
}
