use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::remove_entry;

static STAGING_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Replaces `dest` with a fresh copy of the directory `source`, skipping top-level entries named
/// in `excluded`. The copy is staged beside `dest` and swapped in, so `dest` is never partial.
pub async fn replace_with_copy(source: &Path, dest: &Path, excluded: &[&str]) -> io::Result<()> {
    let parent = dest.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;
    let staging = parent.join(format!(
        ".copy-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let (source_dir, staging_dir) = (source.to_path_buf(), staging.clone());
    let excluded: Vec<String> = excluded.iter().map(|name| name.to_string()).collect();
    let copied =
        tokio::task::spawn_blocking(move || copy_tree(&source_dir, &staging_dir, &excluded))
            .await
            .map_err(io::Error::other)?;
    if let Err(error) = copied {
        remove_entry(&staging).await?;
        return Err(error);
    }

    remove_entry(dest).await?;
    tokio::fs::rename(&staging, dest).await
}

fn copy_tree(source: &Path, dest: &Path, excluded: &[String]) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let is_excluded = excluded.iter().any(|excluded| name == excluded.as_str());
        if is_excluded {
            continue;
        }

        let from = entry.path();
        let to = dest.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_tree(&from, &to, &[])?;
        } else if file_type.is_symlink() {
            copy_symlink(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target: PathBuf = fs::read_link(from)?;
    std::os::unix::fs::symlink(target, to)
}

#[cfg(windows)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target: PathBuf = fs::read_link(from)?;
    if from.is_dir() {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replaces_dest_with_a_filtered_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("libs/local-lib");
        fs::create_dir_all(source.join("src")).unwrap();
        fs::create_dir_all(source.join("node_modules/dep")).unwrap();
        fs::create_dir_all(source.join(".git")).unwrap();
        fs::write(source.join("package.json"), "{}").unwrap();
        fs::write(source.join("src/index.js"), "module.exports = 1;").unwrap();
        let dest = dir.path().join("cache/local-lib/package");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("stale.js"), "").unwrap();

        replace_with_copy(&source, &dest, &["node_modules", ".git"])
            .await
            .unwrap();

        assert!(dest.join("package.json").exists());
        assert!(dest.join("src/index.js").exists());
        assert!(!dest.join("node_modules").exists());
        assert!(!dest.join(".git").exists());
        assert!(!dest.join("stale.js").exists());
        assert_eq!(fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn test_missing_source_leaves_dest_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("cache/pkg/package");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("index.js"), "").unwrap();

        let result = replace_with_copy(&dir.path().join("missing"), &dest, &[]).await;

        assert!(result.is_err());
        assert!(dest.join("index.js").exists());
        assert_eq!(fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }
}
