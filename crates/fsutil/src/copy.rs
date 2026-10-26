use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use crate::context::PathContext;
use crate::links::remove_entry_inner;

static STAGING_COUNTER: AtomicUsize = AtomicUsize::new(0);
/// The cheapest method that has worked so far this process; once cloning or hardlinking fails
/// (unsupported filesystem, different device) it is not retried for every file.
static STORE_IMPORT_METHOD: AtomicU8 = AtomicU8::new(Method::Clone as u8);
static SOURCE_IMPORT_METHOD: AtomicU8 = AtomicU8::new(Method::Clone as u8);
/// Whether whole directories can still be cloned in one call; off once the filesystem says it
/// can't, like the per-file methods.
#[cfg(target_os = "macos")]
static TREE_CLONE_WORKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum Method {
    Clone,
    Hardlink,
    Copy,
}

/// Which ways a file may be imported, cheapest first.
#[derive(Clone, Copy)]
enum Import {
    /// Clone, then hardlink, then copy: for immutable store entries, where sharing inodes is safe.
    FromStore,
    /// Clone, then copy: for the user's own files, which must never share inodes with
    /// `node_modules`.
    FromSource,
}

/// Imports the store entry `source` as `dest`, unless `dest` already exists. The import is
/// staged beside `dest` and renamed in, so `dest` is never partial; if another process finishes
/// first, its copy is kept. Without `allow_hardlinks`, files are cloned or copied, for packages
/// whose install scripts will write to them.
pub async fn import_dir(source: &Path, dest: &Path, allow_hardlinks: bool) -> io::Result<()> {
    import_dir_inner(source, dest, allow_hardlinks)
        .await
        .at("import into", dest)
}

async fn import_dir_inner(source: &Path, dest: &Path, allow_hardlinks: bool) -> io::Result<()> {
    if dest.exists() {
        return Ok(());
    }
    let import = if allow_hardlinks {
        Import::FromStore
    } else {
        Import::FromSource
    };
    let staging = stage(source, dest, &[], import).await?;

    match tokio::fs::rename(&staging, dest).await {
        Ok(()) => Ok(()),
        Err(_) if dest.exists() => remove_entry_inner(&staging).await.map(|_| ()),
        Err(error) => {
            remove_entry_inner(&staging).await?;
            Err(error)
        }
    }
}

/// Replaces `dest` with a fresh copy of the directory `source`, skipping top-level entries named
/// in `excluded`. The copy is staged beside `dest` and swapped in, so `dest` is never partial.
pub async fn replace_with_copy(source: &Path, dest: &Path, excluded: &[&str]) -> io::Result<()> {
    replace_with_copy_inner(source, dest, excluded)
        .await
        .at("copy into", dest)
}

async fn replace_with_copy_inner(source: &Path, dest: &Path, excluded: &[&str]) -> io::Result<()> {
    let staging = stage(source, dest, excluded, Import::FromSource).await?;
    remove_entry_inner(dest).await?;
    tokio::fs::rename(&staging, dest).await
}

async fn stage(
    source: &Path,
    dest: &Path,
    excluded: &[&str],
    import: Import,
) -> io::Result<PathBuf> {
    let parent = dest.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;
    let staging = parent.join(format!(
        ".import-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let (source_dir, staging_dir) = (source.to_path_buf(), staging.clone());
    let excluded: Vec<String> = excluded.iter().map(|name| name.to_string()).collect();
    let copied = tokio::task::spawn_blocking(move || {
        let is_whole_tree = excluded.is_empty();
        if is_whole_tree && clone_tree(&source_dir, &staging_dir) {
            return Ok(());
        }
        copy_tree(&source_dir, &staging_dir, &excluded, import)
    })
    .await
    .map_err(io::Error::other)?;

    if let Err(error) = copied {
        remove_entry_inner(&staging).await?;
        return Err(error);
    }
    Ok(staging)
}

/// Clones the directory `source` as `dest` with a single `clonefile`, far cheaper than a clone per
/// file. Leaves nothing at `dest` when it fails, so the caller can copy instead.
#[cfg(target_os = "macos")]
fn clone_tree(source: &Path, dest: &Path) -> bool {
    if !TREE_CLONE_WORKS.load(Ordering::Relaxed) {
        return false;
    }
    let Err(error) = reflink_copy::reflink(source, dest) else {
        return true;
    };
    let is_unsupported = matches!(
        error.kind(),
        io::ErrorKind::Unsupported | io::ErrorKind::CrossesDevices
    );
    if is_unsupported {
        TREE_CLONE_WORKS.store(false, Ordering::Relaxed);
    }
    let _ = fs::remove_dir_all(dest);
    false
}

#[cfg(not(target_os = "macos"))]
fn clone_tree(_source: &Path, _dest: &Path) -> bool {
    false
}

fn copy_tree(source: &Path, dest: &Path, excluded: &[String], import: Import) -> io::Result<()> {
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
            copy_tree(&from, &to, &[], import)?;
        } else if file_type.is_symlink() {
            copy_symlink(&from, &to)?;
        } else {
            import_file(&from, &to, import)?;
        }
    }
    Ok(())
}

fn import_file(from: &Path, to: &Path, import: Import) -> io::Result<()> {
    let current = match import {
        Import::FromStore => &STORE_IMPORT_METHOD,
        Import::FromSource => &SOURCE_IMPORT_METHOD,
    };

    loop {
        let method = method_from(current.load(Ordering::Relaxed));
        let result = match method {
            Method::Clone => reflink_copy::reflink(from, to),
            Method::Hardlink => fs::hard_link(from, to),
            Method::Copy => fs::copy(from, to).map(|_| ()),
        };
        let Err(error) = result else {
            return Ok(());
        };
        if method == Method::Copy {
            return Err(error);
        }

        let next = next_method(method, import);
        let _ = current.compare_exchange(
            method as u8,
            next as u8,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        let _ = fs::remove_file(to);
    }
}

fn next_method(method: Method, import: Import) -> Method {
    match (method, import) {
        (Method::Clone, Import::FromStore) => Method::Hardlink,
        _ => Method::Copy,
    }
}

fn method_from(value: u8) -> Method {
    match value {
        0 => Method::Clone,
        1 => Method::Hardlink,
        _ => Method::Copy,
    }
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target: PathBuf = fs::read_link(from)?;
    std::os::unix::fs::symlink(target, to)
}

/// Without Developer Mode Windows refuses file symlinks, so the file is copied instead;
/// directory links become junctions, which need no privileges.
#[cfg(windows)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target: PathBuf = fs::read_link(from)?;
    if from.is_dir() {
        let base = from.parent().unwrap_or(Path::new(""));
        let absolute = std::path::absolute(crate::normalize_lexically(&base.join(target)))?;
        return junction::create(absolute, to);
    }
    match std::os::windows::fs::symlink_file(&target, to) {
        Ok(()) => Ok(()),
        Err(_) => fs::copy(from, to).map(|_| ()),
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

    #[tokio::test]
    async fn test_import_dir_imports_files_and_links_once() {
        let dir = tempfile::tempdir().unwrap();
        let store_entry = dir.path().join("store/ms@2.1.3-abc");
        fs::create_dir_all(store_entry.join("lib")).unwrap();
        fs::write(store_entry.join("package.json"), "{}").unwrap();
        fs::write(store_entry.join("lib/index.js"), "module.exports = 1;").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("lib/index.js", store_entry.join("main.js")).unwrap();
        let dest = dir
            .path()
            .join("app/node_modules/.craft/ms@2.1.3/node_modules/ms");

        import_dir(&store_entry, &dest, true).await.unwrap();
        fs::write(dest.join("marker"), "").unwrap();
        import_dir(&store_entry, &dest, true).await.unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("lib/index.js")).unwrap(),
            "module.exports = 1;"
        );
        #[cfg(unix)]
        assert_eq!(
            fs::read_link(dest.join("main.js")).unwrap(),
            PathBuf::from("lib/index.js")
        );
        assert!(dest.join("marker").exists());
        assert_eq!(fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn test_imports_without_hardlinks_never_share_the_store_files() {
        let dir = tempfile::tempdir().unwrap();
        let store_entry = dir.path().join("store/pkg@1.0.0-abc");
        fs::create_dir_all(store_entry.join("lib")).unwrap();
        fs::write(store_entry.join("lib/index.js"), "original").unwrap();
        let dest = dir
            .path()
            .join("app/node_modules/.craft/pkg@1.0.0/node_modules/pkg");

        import_dir(&store_entry, &dest, false).await.unwrap();
        fs::write(dest.join("lib/index.js"), "built").unwrap();

        assert_eq!(
            fs::read_to_string(store_entry.join("lib/index.js")).unwrap(),
            "original"
        );
        assert_eq!(fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn test_falls_back_from_cheaper_methods() {
        assert!(next_method(Method::Clone, Import::FromStore) == Method::Hardlink);
        assert!(next_method(Method::Hardlink, Import::FromStore) == Method::Copy);
        assert!(next_method(Method::Clone, Import::FromSource) == Method::Copy);
    }
}
