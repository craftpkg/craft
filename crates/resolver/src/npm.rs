use crate::ResolvedArtifact;
use anyhow::Result;
use network::Network;
use node_semver::{Range, Version};
use package::{NpmPackage, VersionRequirement};

#[derive(Debug)]
pub struct NpmResolver {
    client: Network,
    registry: String,
}

impl NpmResolver {
    pub fn new(registry: String) -> Self {
        Self {
            client: Network::new(),
            registry,
        }
    }

    pub async fn resolve(
        &self,
        name: &str,
        requirement: &VersionRequirement,
    ) -> Result<ResolvedArtifact> {
        let url = format!("{}/{}", self.registry, name);
        let npm_package = match self.client.fetch::<NpmPackage>(&url).await {
            Ok(package) => package,
            Err(e) => {
                debug::error!("Failed to fetch npm package: {} {}", url, e);
                return Err(e);
            }
        };

        let version = select_version(&npm_package, requirement)
            .ok_or_else(|| anyhow::anyhow!("no version of {name} matches {requirement}"))?;

        let pkg_json = npm_package
            .versions
            .get(&version)
            .ok_or_else(|| anyhow::anyhow!("Package JSON not found for {name}@{version}"))?;

        let dist = pkg_json
            .dist
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dist info not found for {name}@{version}"))?;

        let resolved = ResolvedArtifact {
            name: pkg_json.name.clone().unwrap_or_else(|| name.to_string()),
            version: pkg_json.version.clone().unwrap_or_else(|| version.clone()),
            download_url: dist.tarball.clone(),
            package: Some(pkg_json.clone()),
        };

        debug::trace!("Resolved {name}@{requirement} to {version}");

        Ok(resolved)
    }
}

/// Picks the published version npm would install: the version a tag points at, or for a
/// range the `latest` tag when it satisfies the range, otherwise the highest satisfying version.
fn select_version(npm_package: &NpmPackage, requirement: &VersionRequirement) -> Option<String> {
    match requirement {
        VersionRequirement::Exact(version) => published_versions(npm_package)
            .find(|(published, _)| published == version)
            .map(|(_, key)| key.clone()),
        VersionRequirement::Tag(tag) => npm_package.dist_tags.get(tag).cloned(),
        VersionRequirement::Range(range) => {
            let latest = npm_package.dist_tags.get("latest");
            let latest_satisfies = latest
                .and_then(|latest| Version::parse(latest).ok())
                .is_some_and(|latest| range.satisfies(&latest));
            if latest_satisfies {
                return latest.cloned();
            }

            highest_satisfying(npm_package, range)
        }
    }
}

fn highest_satisfying(npm_package: &NpmPackage, range: &Range) -> Option<String> {
    published_versions(npm_package)
        .filter(|(version, _)| range.satisfies(version))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, key)| key.clone())
}

/// Published versions that parse as semver, paired with their key in the packument.
fn published_versions(npm_package: &NpmPackage) -> impl Iterator<Item = (Version, &String)> {
    npm_package
        .versions
        .keys()
        .filter_map(|key| Version::parse(key).ok().map(|version| (version, key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::DEFAULT_REGISTRY;
    use serde_json::json;

    #[test]
    fn test_range_prefers_latest_when_it_satisfies() {
        let npm_package = packument("2.1.3", &["2.0.0", "2.1.3", "2.2.0", "3.0.0"]);
        assert_eq!(select(&npm_package, "^2.0.0"), Some("2.1.3".to_string()));
    }

    #[test]
    fn test_range_falls_back_to_highest_satisfying() {
        let npm_package = packument("3.0.0", &["2.0.0", "2.1.3", "3.0.0"]);
        assert_eq!(select(&npm_package, "^2.0.0"), Some("2.1.3".to_string()));
    }

    #[test]
    fn test_range_skips_prereleases_and_unparsable_versions() {
        let npm_package = packument("1.0.0", &["1.0.0", "2.0.0-beta.1", "not-a-version"]);
        assert_eq!(select(&npm_package, "*"), Some("1.0.0".to_string()));
        assert_eq!(select(&npm_package, ">=1.0.0"), Some("1.0.0".to_string()));
    }

    #[test]
    fn test_tag_selects_tagged_version() {
        let mut npm_package = packument("15.0.0", &["15.0.0", "16.0.0-canary.1"]);
        npm_package
            .dist_tags
            .insert("canary".to_string(), "16.0.0-canary.1".to_string());

        assert_eq!(
            select(&npm_package, "canary"),
            Some("16.0.0-canary.1".to_string())
        );
        assert_eq!(select(&npm_package, "missing-tag"), None);
    }

    #[test]
    fn test_exact_version() {
        let npm_package = packument("2.1.3", &["2.0.0", "2.1.3"]);
        assert_eq!(select(&npm_package, "2.0.0"), Some("2.0.0".to_string()));
        assert_eq!(select(&npm_package, "9.9.9"), None);
    }

    #[test]
    fn test_no_match_is_none() {
        let npm_package = packument("1.0.0", &["1.0.0"]);
        assert_eq!(select(&npm_package, "^2.0.0"), None);
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_react_latest() {
        let resolver = NpmResolver::new(DEFAULT_REGISTRY.to_string());
        let artifact = resolver
            .resolve("react", &requirement("latest"))
            .await
            .expect("Failed to resolve react package");
        assert_eq!(artifact.name, "react");
        assert!(!artifact.version.is_empty());
        assert!(
            artifact
                .download_url
                .starts_with("https://registry.npmjs.org/react/-/react-")
        );
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_react_specific_version() {
        let resolver = NpmResolver::new(DEFAULT_REGISTRY.to_string());
        let artifact = resolver
            .resolve("react", &requirement("17.0.2"))
            .await
            .expect("Failed to resolve react 17.0.2");
        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
        assert_eq!(
            artifact.download_url,
            "https://registry.npmjs.org/react/-/react-17.0.2.tgz"
        );
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_invalid_package() {
        let resolver = NpmResolver::new(DEFAULT_REGISTRY.to_string());
        let result = resolver
            .resolve("invalid-package-name-12345", &requirement("latest"))
            .await;
        assert!(result.is_err());
    }

    fn select(npm_package: &NpmPackage, spec: &str) -> Option<String> {
        select_version(npm_package, &requirement(spec))
    }

    fn requirement(spec: &str) -> VersionRequirement {
        let dependency = package::DependencySpec::parse("pkg", Some(spec)).unwrap();
        dependency.registry_target().unwrap().1.clone()
    }

    fn packument(latest: &str, versions: &[&str]) -> NpmPackage {
        let versions: serde_json::Map<String, serde_json::Value> = versions
            .iter()
            .map(|version| (version.to_string(), json!({ "version": version })))
            .collect();

        serde_json::from_value(json!({
            "name": "pkg",
            "dist-tags": { "latest": latest },
            "versions": versions,
        }))
        .unwrap()
    }
}
