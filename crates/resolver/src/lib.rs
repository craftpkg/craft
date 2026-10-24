mod download_artifact;
mod git;
pub mod graph;
pub mod npm;
pub mod resolved_artifact;
pub mod resolver;
mod sources;

pub use graph::{DependencyKind, Edge, NodeId, ResolutionGraph, ResolvedNode};
pub use npm::NpmResolver;
pub use resolved_artifact::{ArtifactSource, ResolvedArtifact, short_hash};
pub use resolver::Resolver;
