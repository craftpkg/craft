mod download_artifact;
mod git;
pub mod graph;
mod locked;
mod metadata_cache;
pub mod npm;
pub mod resolved_artifact;
pub mod resolver;
mod sources;

pub use graph::{DependencyKind, Edge, NodeId, PeerDependency, ResolutionGraph, ResolvedNode};
pub use locked::LockPreferences;
pub use npm::NpmResolver;
pub use resolved_artifact::{ArtifactSource, ResolvedArtifact, short_hash};
pub use resolver::Resolver;
