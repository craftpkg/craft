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
    pub body: Vec<u8>,
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
        let (body_path, meta_path) = self.paths(registry, name);
        let meta: CacheMeta =
            serde_json::from_slice(&tokio::fs::read(meta_path).await.ok()?).ok()?;
        let body = tokio::fs::read(body_path).await.ok()?;
        Some(CachedPackument {
            body,
            validators: meta.validators,
            fetched_at: meta.fetched_at,
        })
    }

    pub async fn write(&self, registry: &str, name: &str, body: &[u8], validators: Validators) {
        let (body_path, _) = self.paths(registry, name);
        if let Err(error) = fsutil::write_if_changed(&body_path, body).await {
            debug::warning!("Could not cache metadata of {name}: {error}");
            return;
        }
        self.touch(registry, name, validators).await;
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

    fn paths(&self, registry: &str, name: &str) -> (PathBuf, PathBuf) {
        let file = name.replace('/', "+");
        let dir = self.dir.join(short_hash(registry));
        (
            dir.join(format!("{file}.json")),
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
    async fn test_round_trips_bodies_and_validators() {
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
        cache
            .write(
                "https://registry.test",
                "@types/node",
                b"{}",
                validators.clone(),
            )
            .await;

        let cached = cache
            .read("https://registry.test", "@types/node")
            .await
            .unwrap();
        assert_eq!(cached.body, b"{}");
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
            body: Vec::new(),
            validators: Validators::default(),
            fetched_at: now(),
        };
        assert!(!cached.is_fresh());
    }
}
