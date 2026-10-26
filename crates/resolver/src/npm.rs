use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::OnceCell;

use crate::{ArtifactSource, ResolvedArtifact};
use anyhow::Result;
use contract::{NetworkError, NetworkMode, RegistryConfig, RegistryError};
use network::Revalidated;
use std::path::PathBuf;

use crate::metadata_cache::{CachedPackument, MetadataCache};
use crate::packument::Packument;
use network::{Integrity, Network};
use node_semver::{Range, Version};
use package::{PackageJson, PlatformConstraints, VersionRequirement};

/// One fetch per package name, shared by every concurrent request for it.
type PackumentCell = Arc<OnceCell<Arc<Packument>>>;

#[derive(Debug)]
pub struct NpmResolver {
    client: Network,
    registry: RegistryConfig,
    packuments: Mutex<HashMap<String, PackumentCell>>,
    cache: MetadataCache,
}

impl NpmResolver {
    pub fn new(registry: RegistryConfig, cache_dir: PathBuf) -> Self {
        Self {
            client: Network::new(&registry),
            registry,
            packuments: Mutex::new(HashMap::new()),
            cache: MetadataCache::new(cache_dir),
        }
    }

    /// The package's metadata, fetched once per install however many ranges ask for it. A
    /// failed fetch is not remembered, so a later request tries again.
    async fn packument(&self, name: &str) -> Result<Arc<Packument>> {
        let cell = {
            let mut packuments = self
                .packuments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            packuments.entry(name.to_string()).or_default().clone()
        };
        let packument = cell.get_or_try_init(|| self.load_packument(name)).await?;
        Ok(Arc::clone(packument))
    }

    /// From the metadata cache when the network mode allows or the registry says it's still
    /// current, from the registry otherwise.
    async fn load_packument(&self, name: &str) -> Result<Arc<Packument>> {
        let registry = self.registry.registry_for(name);
        let url = packument_url(registry, name);
        let cached = self.cache.read(registry, name).await;
        let cached_packument = match &cached {
            Some(cached) => Packument::cached(cached.path.clone()).await.ok(),
            None => None,
        };

        let may_use_cache = match self.registry.network_mode {
            NetworkMode::Offline | NetworkMode::PreferOffline => true,
            NetworkMode::Online => cached.as_ref().is_some_and(CachedPackument::is_fresh),
        };
        let cached_packument = match cached_packument {
            Some(packument) if may_use_cache => {
                debug::info!("Using cached metadata of {name}");
                return Ok(Arc::new(packument));
            }
            other => other,
        };

        debug::info!("Fetching {url}");
        let validators = cached_packument
            .is_some()
            .then(|| cached.as_ref().map(|cached| &cached.validators))
            .flatten();
        let revalidated = self
            .client
            .fetch_packument(&url, validators)
            .await
            .map_err(|error| registry_error(name, registry, error))?;
        match (revalidated, cached_packument) {
            (Revalidated::NotModified(validators), Some(packument)) => {
                self.cache.touch(registry, name, validators).await;
                Ok(Arc::new(packument))
            }
            (Revalidated::Fresh { body, validators }, _) => {
                let encoded = Packument::encode(&body)
                    .map_err(|error| registry_error(name, registry, error))?;
                let packument = match self.cache.write(registry, name, &encoded, validators).await {
                    Some(path) => Packument::cached(path).await?,
                    None => Packument::in_memory(encoded)?,
                };
                Ok(Arc::new(packument))
            }
            (Revalidated::NotModified(_), None) => Err(anyhow::anyhow!(
                "{registry} answered 304 for {name}, which is not cached"
            )),
        }
    }

    pub async fn resolve(
        &self,
        name: &str,
        requirement: &VersionRequirement,
    ) -> Result<ResolvedArtifact> {
        let packument = self.packument(name).await?;

        let version = select_version(&packument, requirement)
            .ok_or_else(|| anyhow::anyhow!("no version of {name} matches {requirement}"))?;

        let pkg_json = packument
            .manifest(&version)
            .await
            .map_err(|error| error.context(format!("could not read {name}@{version}")))?;
        let pkg_json = self.with_libc(name, &version, pkg_json).await;

        let dist = pkg_json
            .dist
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dist info not found for {name}@{version}"))?;

        let resolved = ResolvedArtifact {
            name: pkg_json.name.clone().unwrap_or_else(|| name.to_string()),
            version: pkg_json.version.clone().unwrap_or_else(|| version.clone()),
            source: ArtifactSource::Registry {
                tarball_url: dist.tarball.clone(),
            },
            integrity: Integrity::from_dist(dist.integrity.as_deref(), dist.shasum.as_deref())?,
            platform: PlatformConstraints::from_manifest(&pkg_json),
            package: Some(pkg_json),
        };

        debug::trace!("Resolved {name}@{requirement} to {version}");

        Ok(resolved)
    }

    /// Abbreviated manifests leave out `libc`, which tells a Linux package's glibc and musl
    /// builds apart, so a Linux package's full manifest is fetched once and kept. Without it the
    /// package counts as installable with either libc.
    async fn with_libc(&self, name: &str, version: &str, manifest: PackageJson) -> PackageJson {
        let has_libc = manifest.other.contains_key("libc");
        let is_for_linux = PlatformConstraints::from_manifest(&manifest)
            .os
            .iter()
            .any(|os| os == "linux");
        if has_libc || !is_for_linux {
            return manifest;
        }
        match self.full_manifest(name, version).await {
            Ok(full) => full,
            Err(error) => {
                debug::warning!("Could not read the libc of {name}@{version}: {error:#}");
                manifest
            }
        }
    }

    async fn full_manifest(&self, name: &str, version: &str) -> Result<PackageJson> {
        let registry = self.registry.registry_for(name);
        if let Some(cached) = self.cache.read_version(registry, name, version).await {
            return Ok(serde_json::from_slice(&cached)?);
        }
        let url = format!("{}/{version}", packument_url(registry, name));
        let body = self.client.get_body(&url).await?;
        let manifest = serde_json::from_slice(&body)?;
        self.cache
            .write_version(registry, name, version, &body)
            .await;
        Ok(manifest)
    }
}

/// Scoped names are requested as `@scope%2fname`, as npm does; some registries require it.
fn packument_url(registry: &str, name: &str) -> String {
    format!("{registry}/{}", name.replace('/', "%2f"))
}

fn registry_error(name: &str, registry: &str, error: anyhow::Error) -> anyhow::Error {
    let status = match error.downcast_ref::<NetworkError>() {
        Some(NetworkError::Status { status, .. }) => *status,
        Some(NetworkError::Offline { .. }) | None => {
            return error.context(format!("failed to fetch {name} from {registry}"));
        }
    };
    let name = name.to_string();
    let registry = registry.to_string();

    match status {
        404 => RegistryError::PackageNotFound { name, registry }.into(),
        401 | 403 => RegistryError::AccessDenied {
            name,
            registry,
            status,
        }
        .into(),
        _ => error.context(format!("failed to fetch {name} from {registry}")),
    }
}

/// Picks the published version npm would install: the version a tag points at, or for a
/// range the `latest` tag when it satisfies the range, otherwise the highest satisfying version.
fn select_version(packument: &Packument, requirement: &VersionRequirement) -> Option<String> {
    match requirement {
        VersionRequirement::Exact(version) => published_versions(packument)
            .find(|(published, _)| published == version)
            .map(|(_, key)| key.clone()),
        VersionRequirement::Tag(tag) => packument.dist_tags.get(tag).cloned(),
        VersionRequirement::Range(range) => {
            let latest = packument.dist_tags.get("latest");
            let latest_satisfies = latest
                .and_then(|latest| Version::parse(latest).ok())
                .is_some_and(|latest| range.satisfies(&latest));
            if latest_satisfies {
                return latest.cloned();
            }

            highest_satisfying(packument, range)
        }
    }
}

fn highest_satisfying(packument: &Packument, range: &Range) -> Option<String> {
    published_versions(packument)
        .filter(|(version, _)| range.satisfies(version))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, key)| key.clone())
}

/// Published versions that parse as semver, paired with their key in the packument.
fn published_versions(packument: &Packument) -> impl Iterator<Item = (Version, &String)> {
    packument
        .versions
        .iter()
        .filter_map(|key| Version::parse(key).ok().map(|version| (version, key)))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let resolver = NpmResolver::new(
            RegistryConfig::default(),
            std::env::temp_dir().join("craft-test-metadata"),
        );
        let artifact = resolver
            .resolve("react", &requirement("latest"))
            .await
            .expect("Failed to resolve react package");
        assert_eq!(artifact.name, "react");
        assert!(!artifact.version.is_empty());
        let ArtifactSource::Registry { tarball_url } = &artifact.source else {
            panic!("react resolves from the registry");
        };
        assert!(tarball_url.starts_with("https://registry.npmjs.org/react/-/react-"));
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_react_specific_version() {
        let resolver = NpmResolver::new(
            RegistryConfig::default(),
            std::env::temp_dir().join("craft-test-metadata"),
        );
        let artifact = resolver
            .resolve("react", &requirement("17.0.2"))
            .await
            .expect("Failed to resolve react 17.0.2");
        assert_eq!(artifact.name, "react");
        assert_eq!(artifact.version, "17.0.2");
        assert_eq!(
            artifact.source,
            ArtifactSource::Registry {
                tarball_url: "https://registry.npmjs.org/react/-/react-17.0.2.tgz".to_string()
            }
        );
    }

    #[tokio::test]
    #[ignore = "requires network access to registry.npmjs.org"]
    async fn test_resolve_invalid_package() {
        let resolver = NpmResolver::new(
            RegistryConfig::default(),
            std::env::temp_dir().join("craft-test-metadata"),
        );
        let result = resolver
            .resolve("invalid-package-name-12345", &requirement("latest"))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_scoped_packument_urls_are_encoded() {
        assert_eq!(
            packument_url("https://registry.npmjs.org", "@types/node"),
            "https://registry.npmjs.org/@types%2fnode"
        );
        assert_eq!(
            packument_url("https://registry.npmjs.org", "react"),
            "https://registry.npmjs.org/react"
        );
    }

    #[test]
    fn test_registry_errors_name_the_package() {
        let registry = "http://registry.test";
        let message = |status: u16| {
            let error = NetworkError::Status {
                url: format!("{registry}/pkg"),
                status,
            };
            registry_error("pkg", registry, error.into()).to_string()
        };

        assert_eq!(
            message(404),
            "package pkg not found in http://registry.test"
        );
        assert_eq!(
            message(403),
            "http://registry.test denied access to pkg (HTTP 403); check your registry credentials"
        );
        assert_eq!(
            message(500),
            "failed to fetch pkg from http://registry.test"
        );
    }

    fn select(npm_package: &Packument, spec: &str) -> Option<String> {
        select_version(npm_package, &requirement(spec))
    }

    fn requirement(spec: &str) -> VersionRequirement {
        let dependency = package::DependencySpec::parse("pkg", Some(spec)).unwrap();
        dependency.registry_target().unwrap().1.clone()
    }

    fn packument(latest: &str, versions: &[&str]) -> Packument {
        let versions: serde_json::Map<String, serde_json::Value> = versions
            .iter()
            .map(|version| (version.to_string(), json!({ "version": version })))
            .collect();
        let body = json!({
            "name": "pkg",
            "dist-tags": { "latest": latest },
            "versions": versions,
        });

        Packument::in_memory(Packument::encode(body.to_string().as_bytes()).unwrap()).unwrap()
    }
}
