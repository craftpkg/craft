use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::fs;

use crate::context::PathContext;

static STAGING_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Removes whatever is at `path` without following it: a file, a directory tree, or a symlink,
/// including one whose target no longer exists. Returns whether anything was there.
pub async fn remove_entry(path: &Path) -> io::Result<bool> {
    remove_entry_inner(path).await.at("remove", path)
}

pub(crate) async fn remove_entry_inner(path: &Path) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };

    let result = if metadata.is_dir() {
        fs::remove_dir_all(path).await
    } else {
        remove_link_or_file(path).await
    };
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        other => other.map(|()| true),
    }
}

/// Creates `path` and its parents as directories, replacing anything in the way that does not
/// resolve to a directory, such as a dangling symlink or a file.
pub async fn ensure_dir(path: &Path) -> io::Result<()> {
    ensure_dir_inner(path).await.at("create directory", path)
}

pub(crate) async fn ensure_dir_inner(path: &Path) -> io::Result<()> {
    match fs::create_dir_all(path).await {
        Err(error) if is_blocked(&error) => {}
        other => return other,
    }
    let is_dir = fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_dir());
    if is_dir {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        Box::pin(ensure_dir_inner(parent)).await?;
    }
    remove_entry_inner(path).await?;
    fs::create_dir_all(path).await
}

/// Points `link` at the directory `target`, replacing whatever `link` was.
pub async fn force_symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    force_symlink(target, link, LinkKind::Dir)
        .await
        .at("link", link)
}

/// Where the symlink or junction at `link` points, as an absolute path with `.` and `..`
/// resolved.
pub async fn read_link_resolved(link: &Path) -> io::Result<PathBuf> {
    let target = fs::read_link(link).await?;
    let base = link.parent().unwrap_or(Path::new(""));
    Ok(normalize_lexically(
        &base.join(without_verbatim_prefix(&target)),
    ))
}

/// `target` expressed relative to the directory holding `link`, so the link survives moving the
/// tree they share. Falls back to `target` when the two have no common root.
pub fn relative_target(link: &Path, target: &Path) -> PathBuf {
    let Some(link_dir) = link.parent() else {
        return target.to_path_buf();
    };
    let link_components: Vec<Component> = link_dir.components().collect();
    let target_components: Vec<Component> = target.components().collect();
    let shared = link_components
        .iter()
        .zip(&target_components)
        .take_while(|(link_component, target_component)| link_component == target_component)
        .count();
    if shared == 0 {
        return target.to_path_buf();
    }

    let mut relative = PathBuf::new();
    for _ in shared..link_components.len() {
        relative.push("..");
    }
    for component in &target_components[shared..] {
        relative.push(component);
    }
    relative
}

/// Resolves `.` and `..` without touching the filesystem, for paths that may not exist.
pub fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[derive(Clone, Copy)]
pub(crate) enum LinkKind {
    Dir,
    #[cfg(unix)]
    File,
}

/// Creates the link under a unique staging name and renames it over `link`, so concurrent
/// writers and stale or dangling entries never surface as `EEXIST`.
pub(crate) async fn force_symlink(target: &Path, link: &Path, kind: LinkKind) -> io::Result<()> {
    if let Some(parent) = link.parent() {
        ensure_dir_inner(parent).await?;
    }
    let staging = staging_path(link);
    create_symlink(target, &staging, kind).await?;

    let moved = move_into_place(&staging, link).await;
    if moved.is_err() {
        let _ = remove_link_or_file(&staging).await;
    }
    moved
}

#[cfg(unix)]
async fn move_into_place(staging: &Path, link: &Path) -> io::Result<()> {
    match fs::rename(staging, link).await {
        Ok(()) => Ok(()),
        Err(_) => {
            remove_entry_inner(link).await?;
            fs::rename(staging, link).await
        }
    }
}

/// Windows cannot rename onto an existing directory (and Wine reports success without moving
/// anything), so the old entry is removed first and the move confirmed. Another writer may put
/// a link in place in between, and a link another writer is deleting stays "delete pending"
/// for a moment, refusing both removal and renaming with "Access is denied"; so this retries,
/// and once `link` points where `staging` does, the work is done.
#[cfg(windows)]
async fn move_into_place(staging: &Path, link: &Path) -> io::Result<()> {
    const ATTEMPTS: u64 = 32;

    let wanted = read_link_resolved(staging).await?;
    let mut last_error = None;
    for attempt in 1..=ATTEMPTS {
        let is_in_place = read_link_resolved(link)
            .await
            .is_ok_and(|current| current == wanted);
        if is_in_place {
            return remove_link_or_file(staging).await;
        }
        let moved = match remove_entry_inner(link).await {
            Ok(_) => fs::rename(staging, link).await,
            Err(error) => Err(error),
        };
        match moved {
            Ok(()) if fs::symlink_metadata(staging).await.is_err() => return Ok(()),
            Ok(()) => {}
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(std::time::Duration::from_millis(attempt)).await;
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "it kept being replaced by another process",
        )
    }))
}

fn is_blocked(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::AlreadyExists | io::ErrorKind::NotADirectory
    )
}

fn staging_path(link: &Path) -> PathBuf {
    let name = link
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unique = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
    link.with_file_name(format!(".{name}.craft-{}-{unique}", std::process::id()))
}

#[cfg(unix)]
async fn create_symlink(target: &Path, link: &Path, _kind: LinkKind) -> io::Result<()> {
    fs::symlink(target, link).await
}

/// Directories are linked with junctions, which unlike symlinks need neither administrator
/// rights nor Developer Mode, but only take absolute targets.
#[cfg(windows)]
async fn create_symlink(target: &Path, link: &Path, _kind: LinkKind) -> io::Result<()> {
    let base = link.parent().unwrap_or(Path::new(""));
    let target = std::path::absolute(normalize_lexically(&base.join(target)))?;
    let link = link.to_path_buf();
    tokio::task::spawn_blocking(move || junction::create(&target, &link))
        .await
        .map_err(io::Error::other)?
}

#[cfg(unix)]
fn without_verbatim_prefix(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Junction targets read back as `\\?\C:\…`, which never compares equal to the `C:\…`
/// paths craft builds.
#[cfg(windows)]
fn without_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => PathBuf::from(format!(r"\\{share}")),
        None => PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)),
    }
}

#[cfg(unix)]
async fn remove_link_or_file(path: &Path) -> io::Result<()> {
    fs::remove_file(path).await
}

#[cfg(windows)]
async fn remove_link_or_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(_) => fs::remove_dir(path).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_remove_entry_handles_every_kind_of_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        let tree = dir.path().join("tree");
        let dangling = dir.path().join("dangling");
        std::fs::write(&file, "").unwrap();
        std::fs::create_dir_all(tree.join("nested")).unwrap();
        create_symlink(Path::new("/nonexistent"), &dangling, LinkKind::Dir)
            .await
            .unwrap();

        for path in [&file, &tree, &dangling] {
            assert!(remove_entry(path).await.unwrap());
            assert!(std::fs::symlink_metadata(path).is_err());
        }
        assert!(!remove_entry(&dir.path().join("missing")).await.unwrap());
    }

    #[tokio::test]
    async fn test_remove_entry_does_not_follow_links() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.js"), "").unwrap();
        create_symlink(&target, &link, LinkKind::Dir).await.unwrap();

        remove_entry(&link).await.unwrap();

        assert!(target.join("keep.js").exists());
    }

    #[tokio::test]
    async fn test_ensure_dir_replaces_what_is_in_the_way() {
        let dir = tempfile::tempdir().unwrap();
        let node_modules = dir.path().join("node_modules");
        std::fs::create_dir_all(&node_modules).unwrap();
        create_symlink(
            Path::new("/nonexistent"),
            &node_modules.join(".bin"),
            LinkKind::Dir,
        )
        .await
        .unwrap();
        std::fs::write(node_modules.join("@types"), "").unwrap();

        ensure_dir(&node_modules.join(".bin")).await.unwrap();
        ensure_dir(&node_modules.join("@types/node")).await.unwrap();
        ensure_dir(&node_modules).await.unwrap();

        assert!(node_modules.join(".bin").is_dir());
        assert!(node_modules.join("@types/node").is_dir());
    }

    #[tokio::test]
    async fn test_ensure_dir_keeps_links_to_directories() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.js"), "").unwrap();
        create_symlink(&target, &link, LinkKind::Dir).await.unwrap();

        ensure_dir(&link).await.unwrap();

        assert!(link.join("keep.js").exists());
    }

    #[tokio::test]
    async fn test_force_symlink_replaces_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("pkg");
        std::fs::create_dir_all(&target).unwrap();
        let dangling = dir.path().join("node_modules/dangling");
        let real_dir = dir.path().join("node_modules/real");
        let file = dir.path().join("node_modules/file");
        force_symlink_dir(Path::new("/nonexistent"), &dangling)
            .await
            .unwrap();
        std::fs::create_dir_all(real_dir.join("nested")).unwrap();
        std::fs::write(&file, "").unwrap();

        for link in [&dangling, &real_dir, &file] {
            force_symlink_dir(&target, link).await.unwrap();
            assert_eq!(read_link_resolved(link).await.unwrap(), target);
        }
        let entries = std::fs::read_dir(dir.path().join("node_modules"))
            .unwrap()
            .count();
        assert_eq!(entries, 3);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_force_symlinks_to_one_link_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("pkg");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("node_modules/pkg");

        let tasks: Vec<_> = (0..32)
            .map(|_| {
                let (target, link) = (target.clone(), link.clone());
                tokio::spawn(async move { force_symlink_dir(&target, &link).await })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }

        assert_eq!(read_link_resolved(&link).await.unwrap(), target);
    }

    #[test]
    fn test_normalize_lexically() {
        assert_eq!(
            normalize_lexically(Path::new("/app/node_modules/.bin/../cowsay/./cli.js")),
            PathBuf::from("/app/node_modules/cowsay/cli.js")
        );
    }

    #[test]
    fn test_relative_target() {
        assert_eq!(
            relative_target(
                Path::new("/app/node_modules/.bin/tsc"),
                Path::new("/app/node_modules/typescript/bin/tsc")
            ),
            PathBuf::from("../typescript/bin/tsc")
        );
        assert_eq!(
            relative_target(
                Path::new("/app/node_modules/.bin/tsc"),
                Path::new("/app/node_modules/@scope/cli/bin.js")
            ),
            PathBuf::from("../@scope/cli/bin.js")
        );
        assert_eq!(
            relative_target(Path::new("link"), Path::new("/abs/target")),
            PathBuf::from("/abs/target")
        );
    }
}
