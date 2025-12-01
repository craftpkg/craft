use crate::{DependencySpec, ResolvedArtifact};
use anyhow::Result;
use network::Network;
use node_semver::{Range, Version};
use package::{InstallPackage, NpmPackage};

#[derive(Debug)]
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
        // Convert to DependencySpec (handles npm aliases and regular versions)
        let dep_spec: DependencySpec = package.into();

        let url = format!("https://registry.npmjs.org/{}", dep_spec.package_name);
        let npm_package = match self.client.fetch::<NpmPackage>(&url).await {
            Ok(package) => package,
            Err(e) => {
                debug::error!("Failed to fetch npm package: {} {}", url, e);
                return Err(e);
            }
        };

        let version = if let Some(ref req_version) = dep_spec.version {
            // Find a version that satisfies the requirement
            npm_package
                .versions
                .keys()
                .filter(|v| {
                    let req = Range::parse(req_version).expect("should not panic");
                    let ver = Version::parse(v).expect("should not panic");
                    req.satisfies(&ver)
                })
                .max_by(|a, b| {
                    let ver_a = Version::parse(a).expect("should not panic");
                    let ver_b = Version::parse(b).expect("should not panic");
                    ver_a.partial_cmp(&ver_b).expect("should not panic")
                })
                .cloned()
        } else {
            // Use latest version from dist-tags
            npm_package.dist_tags.get("latest").cloned()
        };

        let version = version.ok_or_else(|| {
            anyhow::anyhow!("Version not found for package {}", dep_spec.package_name)
        })?;

        let pkg_json = npm_package
            .versions
            .get(&version)
            .ok_or_else(|| anyhow::anyhow!("Package JSON not found for version {}", version))?;

        let dist = pkg_json
            .dist
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dist info not found"))?;

        let resolved = ResolvedArtifact {
            name: pkg_json
                .name
                .clone()
                .expect("Resolved package should have name property"),
            version: pkg_json
                .version
                .clone()
                .expect("Resolved package should have version property"),
            download_url: dist.tarball.clone(),
            package: Some(pkg_json.clone()),
        };

        debug::trace!(
            "Resolved package for {} {}",
            package.name,
            package.version.clone().unwrap_or("".to_string()),
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

    #[tokio::test]
    async fn test_resolve_npm_alias() {
        // Test npm:package@version format (e.g., "wrap-ansi-cjs": "npm:wrap-ansi@^7.0.0")
        let pkg = InstallPackage::new(
            "wrap-ansi-cjs".to_string(),
            Some("npm:wrap-ansi@^7.0.0".to_string()),
            false,
        );
        let resolver = NpmResolver::new();
        let result = resolver.resolve(&pkg).await;
        assert!(result.is_ok());
        let artifact = result.expect("Failed to resolve npm alias");
        assert_eq!(artifact.name, "wrap-ansi");
        assert!(artifact.version.starts_with("7."));
    }
}
