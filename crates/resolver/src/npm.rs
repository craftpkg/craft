use crate::ResolvedArtifact;
use anyhow::Result;
use network::Network;
use node_semver::{Range, Version};
use package::{InstallPackage, NpmPackage};

pub struct NpmResolver {
    client: Network,
}

impl NpmResolver {
    pub fn new() -> Self {
        Self {
            client: Network::new(),
        }
    }

    pub async fn resolve(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        let url = format!("https://registry.npmjs.org/{}", package.name);
        let npm_package = self.client.fetch::<NpmPackage>(&url).await?;

        let version = if let Some(ref req_version) = package.version {
            // Find a version that satisfies the requirement
            npm_package
                .versions
                .keys()
                .filter(|v| {
                    let req = Range::parse(req_version).unwrap();
                    let ver = Version::parse(v).unwrap();
                    req.satisfies(&ver)
                })
                .max_by(|a, b| {
                    let ver_a = Version::parse(a).unwrap();
                    let ver_b = Version::parse(b).unwrap();
                    ver_a.partial_cmp(&ver_b).unwrap()
                })
                .cloned()
        } else {
            // Use latest version from dist-tags
            npm_package.dist_tags.get("latest").cloned()
        };

        let version = version
            .ok_or_else(|| anyhow::anyhow!("Version not found for package {}", package.name))?;

        let pkg_json = npm_package
            .versions
            .get(&version)
            .ok_or_else(|| anyhow::anyhow!("Package JSON not found for version {}", version))?;

        let dist = pkg_json
            .dist
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dist info not found"))?;

        let resolved = ResolvedArtifact {
            name: pkg_json.name.clone(),
            version: pkg_json.version.clone(),
            download_url: dist.tarball.clone(),
            package: Some(pkg_json.clone()),
        };

        debug::trace!(
            "Resolved package for {} {} - {:?}",
            package.name,
            package.version.clone().unwrap_or("".to_string()),
            resolved
        );

        Ok(resolved)
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
