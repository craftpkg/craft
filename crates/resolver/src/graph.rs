use std::collections::BTreeMap;
use std::fmt::Display;

use contract::Result;

use crate::ResolvedArtifact;

/// Identifies one resolved package version: `name@version`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(name: &str, version: &str) -> Self {
        Self(format!("{name}@{version}"))
    }

    /// The id of this package resolved with `peers`, as `<id>_<peer id>_<peer id>…` in peer-name
    /// order; the plain id when there are none.
    pub fn with_peers<'a>(&self, peers: impl IntoIterator<Item = &'a NodeId>) -> Self {
        let mut id = self.0.clone();
        for peer in peers {
            id.push('_');
            id.push_str(&peer.0);
        }
        Self(id)
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Production,
    Development,
    Peer,
    /// `optionalDependencies`: skipped when unsupported on the host or failing to install.
    Optional,
}

/// A dependency as its dependent declared it, and the node it resolved to.
#[derive(Debug, Clone)]
pub struct Edge {
    /// The name the dependent requires it by — the alias for `npm:` specs.
    pub name: String,
    pub spec: Option<String>,
    pub target: NodeId,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone)]
pub struct ResolvedNode {
    pub artifact: ResolvedArtifact,
    /// Dependencies, plus `Peer` edges once peers are resolved.
    pub dependencies: Vec<Edge>,
    pub peers: Vec<PeerDependency>,
}

/// A peer a package declares. `candidate` is what the peer range resolves to on its own, used
/// when nothing above the package provides it; optional peers have none.
#[derive(Debug, Clone)]
pub struct PeerDependency {
    pub name: String,
    pub range: String,
    pub optional: bool,
    pub candidate: Option<NodeId>,
}

/// The full dependency tree of an install: the project's own dependencies (`roots`) and every
/// package version they pull in, each listing the edges to its own dependencies.
#[derive(Debug, Clone, Default)]
pub struct ResolutionGraph {
    pub roots: Vec<Edge>,
    pub nodes: BTreeMap<NodeId, ResolvedNode>,
}

impl ResolutionGraph {
    pub fn target(&self, edge: &Edge) -> Result<&ResolvedNode> {
        self.nodes.get(&edge.target).ok_or_else(|| {
            anyhow::anyhow!(
                "{} resolved to {}, which is missing from the resolution graph",
                edge.name,
                edge.target
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_is_name_at_version() {
        assert_eq!(
            NodeId::new("@types/node", "22.0.0").to_string(),
            "@types/node@22.0.0"
        );
    }

    #[test]
    fn test_node_id_with_peers() {
        let query = NodeId::new("@tanstack/react-query", "5.90.10");
        let react = NodeId::new("react", "19.2.0");
        assert_eq!(
            query.with_peers([&react]).to_string(),
            "@tanstack/react-query@5.90.10_react@19.2.0"
        );
        assert_eq!(query.with_peers([]), query);
    }

    #[test]
    fn test_target_of_missing_node_is_an_error() {
        let graph = ResolutionGraph::default();
        let edge = Edge {
            name: "ms".to_string(),
            spec: Some("^2.0.0".to_string()),
            target: NodeId::new("ms", "2.1.3"),
            kind: DependencyKind::Production,
        };

        let error = graph.target(&edge).unwrap_err();

        assert_eq!(
            error.to_string(),
            "ms resolved to ms@2.1.3, which is missing from the resolution graph"
        );
    }
}
