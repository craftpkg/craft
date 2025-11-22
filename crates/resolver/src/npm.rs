use crate::ResolvedArtifact;
use anyhow::{Context, Result};
use package::{InstallPackage, NpmPackage};
use reqwest::Client;

pub struct NpmResolver {
    client: Client,
}

impl NpmResolver {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        let url = format!("https://registry.npmjs.org/{}", package.name);
        let resp = self.client.get(&url).send().await?;
        let npm_package = resp.json::<NpmPackage>().await?;

        // Determine version to use
        let version = if let Some(ref req_version) = package.version {
            // Find a version that satisfies the requirement
            npm_package
                .versions
                .keys()
                .find(|v| package.satisfies(v))
                .context(format!(
                    "Version {} not found for package {}",
                    req_version, package.name
                ))?
                .clone()
        } else {
            // Use latest version if no version specified
            npm_package
                .dist_tags
                .get("latest")
                .context("No latest version found")?
                .clone()
        };

        let package_json = npm_package
            .versions
            .get(&version)
            .cloned()
            .context(format!(
                "Version {} not found for package {}",
                version, package.name
            ))?;

        let download_url = package_json
            .dist
            .as_ref()
            .context("No dist info found")?
            .tarball
            .clone();

        Ok(ResolvedArtifact {
            name: package_json.name,
            version: package_json.version,
            download_url,
        })
    }
}

impl Default for NpmResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_react_latest() {
        let pkg = InstallPackage::new("react".to_string(), None, false);
        let resolver = NpmResolver::new();
        let result = resolver.resolve(&pkg).await;
        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve react package");
        assert_eq!(artifact.name, "react");
        assert!(!artifact.version.is_empty());
        assert!(
            artifact
                .download_url
                .starts_with("https://registry.npmjs.org/react/-/react-")
        );
    }

    #[tokio::test]
    async fn test_resolve_react_specific_version() {
        let pkg = InstallPackage::new("react".to_string(), Some("17.0.2".to_string()), false);
        let resolver = NpmResolver::new();
        let result = resolver.resolve(&pkg).await;
        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve react 17.0.2");
        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
        assert_eq!(
            artifact.download_url,
            "https://registry.npmjs.org/react/-/react-17.0.2.tgz"
        );
    }

    #[tokio::test]
    async fn test_resolve_invalid_package() {
        let pkg = InstallPackage::new("invalid-package-name-12345".to_string(), None, false);
        let resolver = NpmResolver::new();
        let result = resolver.resolve(&pkg).await;
        assert!(result.is_err());
    }
}
