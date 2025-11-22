use crate::ResolvedArtifact;
use anyhow::{Context, Result};
use package::InstallPackage;

pub struct GitResolver;

impl GitResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        let git_url = &package.name;

        // Parse git URL to extract repository information
        let (name, version, download_url) = self.parse_git_url(git_url)?;

        Ok(ResolvedArtifact {
            name,
            version,
            download_url,
        })
    }

    fn parse_git_url(&self, git_url: &str) -> Result<(String, String, String)> {
        // Remove git protocol prefixes
        let url = git_url
            .trim_start_matches("git:")
            .trim_start_matches("git+ssh:")
            .trim_start_matches("git+http:")
            .trim_start_matches("git+https:")
            .trim_start_matches("ssh:");

        // Extract name from URL (last part of path without .git)
        let name = url
            .split('/')
            .next_back()
            .context("Invalid git URL: no path segments")?
            .trim_end_matches(".git")
            .to_string();

        // For now, use "git" as version identifier
        let version = "git".to_string();

        // Normalize the download URL
        let download_url = if git_url.starts_with("git:")
            || git_url.starts_with("git+")
            || git_url.starts_with("ssh:")
        {
            git_url.to_string()
        } else if git_url.ends_with(".git") {
            format!("git+https://{}", url)
        } else {
            git_url.to_string()
        };

        Ok((name, version, download_url))
    }
}

impl Default for GitResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_git_url() {
        let pkg = InstallPackage::new("git:github.com/user/repo.git".to_string(), None, false);
        let resolver = GitResolver::new();
        let result = resolver.resolve(&pkg);

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve git package");
        assert_eq!(artifact.name, "repo");
        assert_eq!(artifact.version, "git");
        assert_eq!(artifact.download_url, "git:github.com/user/repo.git");
    }

    #[test]
    fn test_resolve_git_ssh_url() {
        let pkg = InstallPackage::new(
            "git+ssh:git@github.com:user/repo.git".to_string(),
            None,
            false,
        );
        let resolver = GitResolver::new();
        let result = resolver.resolve(&pkg);

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve git ssh package");
        assert_eq!(artifact.name, "repo");
        assert_eq!(
            artifact.download_url,
            "git+ssh:git@github.com:user/repo.git"
        );
    }

    #[test]
    fn test_resolve_https_git_url() {
        let pkg = InstallPackage::new("https://github.com/user/repo.git".to_string(), None, false);
        let resolver = GitResolver::new();
        let result = resolver.resolve(&pkg);

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve https git package");
        assert_eq!(artifact.name, "repo");
        assert!(artifact.download_url.contains("github.com/user/repo.git"));
    }

    #[test]
    fn test_resolve_gitlab_url() {
        let pkg = InstallPackage::new("git:gitlab.com/package/psc".to_string(), None, false);
        let resolver = GitResolver::new();
        let result = resolver.resolve(&pkg);

        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve gitlab package");
        assert_eq!(artifact.name, "psc");
    }
}
