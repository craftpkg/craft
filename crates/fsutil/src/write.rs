use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::context::PathContext;

static STAGING_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Writes `contents` to `path` through a temp file renamed into place, so readers never see a
/// partial file. Returns `false` without writing when `path` already holds `contents`.
pub async fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<bool> {
    write_if_changed_inner(path, contents)
        .await
        .at("write", path)
}

pub(crate) async fn write_if_changed_inner(path: &Path, contents: &[u8]) -> io::Result<bool> {
    let is_unchanged = tokio::fs::read(path)
        .await
        .is_ok_and(|existing| existing == contents);
    if is_unchanged {
        return Ok(false);
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staging = parent.join(format!(
        ".{file_name}.craft-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    tokio::fs::write(&staging, contents).await?;
    if let Err(error) = tokio::fs::rename(&staging, path).await {
        let _ = tokio::fs::remove_file(&staging).await;
        return Err(error);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_writes_only_when_contents_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("craft.lock");

        assert!(write_if_changed(&path, b"one").await.unwrap());
        assert!(!write_if_changed(&path, b"one").await.unwrap());
        assert!(write_if_changed(&path, b"two").await.unwrap());

        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
