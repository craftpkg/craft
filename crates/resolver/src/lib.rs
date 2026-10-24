mod download_artifact;
pub mod npm;
pub mod resolved_artifact;
pub mod resolver;

pub use npm::NpmResolver;
pub use resolved_artifact::ResolvedArtifact;
pub use resolver::Resolver;
