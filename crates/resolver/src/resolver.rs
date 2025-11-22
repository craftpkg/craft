use crate::{GitResolver, NpmResolver, ResolvedArtifact};
use contract::Result;
use package::InstallPackage;

pub struct Resolver {
    npm_resolver: NpmResolver,
    git_resolver: GitResolver,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            npm_resolver: NpmResolver::new(),
            git_resolver: GitResolver::new(),
        }
    }

    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        if package.is_git() {
            debug::info!("Resolving git package: {}", package.name);
            self.git_resolver.resolve(package)
        } else {
            debug::info!(
                "Resolving npm package: {} {}",
                package.name,
                package.version.clone().unwrap_or("".to_string()),
            );
            self.npm_resolver.resolve(package).await
        }
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_npm_package() {
        let pkg = InstallPackage::new("react".to_string(), Some("17.0.2".to_string()), false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve npm package");
        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
    }

    #[tokio::test]
    async fn test_resolve_git_package() {
        let pkg = InstallPackage::new("git:github.com/user/repo.git".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve git package");
        assert_eq!(artifact.name, "repo");
        assert_eq!(artifact.version, "git");
    }

    #[tokio::test]
    async fn test_resolve_npm_latest() {
        let pkg = InstallPackage::new("express".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve express package");
        assert_eq!(artifact.name, "express");
        assert!(!artifact.version.is_empty());
    }

    #[tokio::test]
    async fn test_resolve_gitlab_package() {
        let pkg = InstallPackage::new("git:gitlab.com/package/psc".to_string(), None, false);
        let resolver = Resolver::new();
        let result = resolver.resolve(&pkg).await;

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve gitlab package");
        assert_eq!(artifact.name, "psc");
    }
}
