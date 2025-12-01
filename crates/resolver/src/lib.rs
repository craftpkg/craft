pub mod dependency;
mod download_artifact;
pub mod git;
pub mod npm;
pub mod resolved_artifact;
pub mod resolver;

pub use dependency::DependencySpec;
pub use git::GitResolver;
pub use npm::NpmResolver;
pub use resolved_artifact::ResolvedArtifact;
pub use resolver::Resolver;
