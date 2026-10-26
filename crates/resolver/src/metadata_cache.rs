use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use network::Validators;
use serde::{Deserialize, Serialize};

use crate::resolved_artifact::short_hash;

/// Packuments kept between installs, keyed by registry and package, with what is needed to
/// revalidate them. Best effort: an unreadable or unwritable entry just means a fetch.
#[derive(Debug)]
pub(crate) struct MetadataCache {
    dir: PathBuf,
}

pub(crate) struct CachedPackument {
    /// The packument, encoded as [`Packument`](crate::packument::Packument) reads it.
    pub path: PathBuf,
    pub validators: Validators,
    fetched_at: u64,
}

#[derive(Serialize, Deserialize)]
struct CacheMeta {
    validators: Validators,
    fetched_at: u64,
}

impl CachedPackument {
    /// Within the `max-age` the registry gave it, so usable without asking again.
    pub fn is_fresh(&self) -> bool {
        self.validators
            .max_age
            .is_some_and(|max_age| now() < self.fetched_at.saturating_add(max_age))
    }
}

impl MetadataCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub async fn read(&self, registry: &str, name: &str) -> Option<CachedPackument> {
        let (path, meta_path) = self.paths(registry, name);
        let meta: CacheMeta =
            serde_json::from_slice(&tokio::fs::read(meta_path).await.ok()?).ok()?;
        let is_cached = tokio::fs::try_exists(&path).await.unwrap_or(false);
        is_cached.then_some(CachedPackument {
            path,
            validators: meta.validators,
            fetched_at: meta.fetched_at,
        })
    }

    /// Caches an encoded packument, returning where it was written. Drops the full packument
    /// cached before packuments were encoded.
    pub async fn write(
        &self,
        registry: &str,
        name: &str,
        encoded: &[u8],
        validators: Validators,
    ) -> Option<PathBuf> {
        let (path, _) = self.paths(registry, name);
        if let Err(error) = fsutil::write_if_changed(&path, encoded).await {
            debug::warning!("Could not cache metadata of {name}: {error}");
            return None;
        }
        let _ = tokio::fs::remove_file(path.with_extension("json")).await;
        self.touch(registry, name, validators).await;
        Some(path)
    }

    /// Records that the cached body was just confirmed current.
    pub async fn touch(&self, registry: &str, name: &str, validators: Validators) {
        let (_, meta_path) = self.paths(registry, name);
        let meta = CacheMeta {
            validators,
            fetched_at: now(),
        };
        let Ok(encoded) = serde_json::to_vec(&meta) else {
            return;
        };
        if let Err(error) = fsutil::write_if_changed(&meta_path, &encoded).await {
            debug::warning!("Could not cache metadata of {name}: {error}");
        }
    }

    /// The full manifest of one published version, which never changes once cached.
    pub async fn read_version(&self, registry: &str, name: &str, version: &str) -> Option<Vec<u8>> {
        tokio::fs::read(self.version_path(registry, name, version))
            .await
            .ok()
    }

    pub async fn write_version(&self, registry: &str, name: &str, version: &str, manifest: &[u8]) {
        let path = self.version_path(registry, name, version);
        if let Err(error) = fsutil::write_if_changed(&path, manifest).await {
            debug::warning!("Could not cache metadata of {name}@{version}: {error}");
        }
    }

    fn version_path(&self, registry: &str, name: &str, version: &str) -> PathBuf {
        let file = name.replace('/', "+");
        self.dir
            .join(short_hash(registry))
            .join(format!("{file}@{version}.json"))
    }

    fn paths(&self, registry: &str, name: &str) -> (PathBuf, PathBuf) {
        let file = name.replace('/', "+");
        let dir = self.dir.join(short_hash(registry));
        (
            dir.join(format!("{file}.packument")),
            dir.join(format!("{file}.meta.json")),
        )
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_round_trips_packuments_and_validators() {
        let dir = tempfile::tempdir().unwrap();
        let cache = MetadataCache::new(dir.path().to_path_buf());
        assert!(
            cache
                .read("https://registry.test", "@types/node")
                .await
                .is_none()
        );

        let validators = Validators {
            etag: Some(r#""abc""#.to_string()),
            last_modified: None,
            max_age: Some(300),
        };
        let written = cache
            .write(
                "https://registry.test",
                "@types/node",
                b"encoded",
                validators.clone(),
            )
            .await;

        let cached = cache
            .read("https://registry.test", "@types/node")
            .await
            .unwrap();
        assert_eq!(written.as_ref(), Some(&cached.path));
        assert_eq!(std::fs::read(&cached.path).unwrap(), b"encoded");
        assert_eq!(cached.validators, validators);
        assert!(cached.is_fresh());
        assert!(
            cache
                .read("https://other.test", "@types/node")
                .await
                .is_none()
        );
    }

    #[test]
    fn test_entries_without_max_age_are_never_fresh() {
        let cached = CachedPackument {
            path: PathBuf::new(),
            validators: Validators::default(),
            fetched_at: now(),
        };
        assert!(!cached.is_fresh());
    }
}
