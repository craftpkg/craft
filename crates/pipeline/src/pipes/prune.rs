use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use tokio::fs;

use super::virtual_store::{HOIST_DIR, INSTALLED_STATE, VIRTUAL_STORE_DIR};

/// Removes what the project no longer uses from `node_modules`: top-level links to packages
/// outside `keep_roots` (when given), every virtual store entry no top-level link reaches,
/// and links left dangling by those removals. Only symlinks craft could have made are removed at
/// the top level; dot entries such as `.cache` and real directories are left alone.
pub async fn prune_node_modules(
    node_modules: &Path,
    keep_roots: Option<&HashSet<String>>,
) -> io::Result<()> {
    if let Some(keep_roots) = keep_roots {
        for (name, path) in package_entries(node_modules).await? {
            let is_kept = keep_roots.contains(&name);
            if !is_kept && fs::symlink_metadata(&path).await?.is_symlink() {
                debug::info!("Pruning {}", name);
                fsutil::remove_entry(&path).await?;
            }
        }
    }

    let virtual_store = node_modules.join(VIRTUAL_STORE_DIR);
    let reachable = reachable_entries(node_modules, &virtual_store).await?;
    let mut entries = match fs::read_dir(&virtual_store).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_craft_file = name == HOIST_DIR || name == INSTALLED_STATE;
        let is_unused = !is_craft_file && !reachable.contains(&name);
        if is_unused {
            debug::info!("Pruning {}", name);
            fsutil::remove_entry(&entry.path()).await?;
        }
    }

    let hoist_dir = virtual_store.join(HOIST_DIR);
    remove_dangling_links(&hoist_dir).await?;
    remove_dangling_links(&node_modules.join(".bin")).await?;
    remove_dangling_links(node_modules).await?;
    remove_empty_scopes(&hoist_dir).await?;
    remove_empty_scopes(node_modules).await
}

/// Virtual store entries reachable from the top-level links, following each entry's own
/// dependency links.
async fn reachable_entries(
    node_modules: &Path,
    virtual_store: &Path,
) -> io::Result<HashSet<String>> {
    let mut reachable = HashSet::new();
    let mut pending: Vec<String> = Vec::new();
    for (_, path) in package_entries(node_modules).await? {
        pending.extend(store_entry_of(&path, virtual_store).await);
    }

    while let Some(entry) = pending.pop() {
        if !reachable.insert(entry.clone()) {
            continue;
        }
        let modules_dir = virtual_store.join(&entry).join("node_modules");
        for (_, path) in package_entries(&modules_dir).await? {
            pending.extend(store_entry_of(&path, virtual_store).await);
        }
    }
    Ok(reachable)
}

/// The virtual store entry a symlink points into, if any.
async fn store_entry_of(link: &Path, virtual_store: &Path) -> Option<String> {
    let target = fs::read_link(link).await.ok()?;
    let resolved = fsutil::normalize_lexically(&link.parent()?.join(target));
    let inside = resolved.strip_prefix(virtual_store).ok()?;
    let entry = inside.components().next()?.as_os_str().to_string_lossy();
    let is_hoist_dir = entry == HOIST_DIR;
    (!is_hoist_dir).then(|| entry.into_owned())
}

/// Package-named entries of a `node_modules` directory, with scoped packages listed as
/// `@scope/name`; dot entries (`.bin`, `.craft`, tool caches) are skipped.
async fn package_entries(node_modules: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut packages = Vec::new();
    for (name, path) in dir_entries(node_modules).await? {
        if name.starts_with('.') {
            continue;
        }
        let is_scope = name.starts_with('@') && !fs::symlink_metadata(&path).await?.is_symlink();
        if !is_scope {
            packages.push((name, path));
            continue;
        }
        for (scoped_name, scoped_path) in dir_entries(&path).await? {
            packages.push((format!("{name}/{scoped_name}"), scoped_path));
        }
    }
    Ok(packages)
}

/// Removes symlinks whose target no longer exists, directly in `dir` and inside its `@scope`
/// directories.
async fn remove_dangling_links(dir: &Path) -> io::Result<()> {
    for (_, path) in package_entries_including_bins(dir).await? {
        let is_dangling = fs::symlink_metadata(&path).await?.is_symlink() && !path.exists();
        if is_dangling {
            fsutil::remove_entry(&path).await?;
        }
    }
    Ok(())
}

async fn remove_empty_scopes(dir: &Path) -> io::Result<()> {
    for (name, path) in dir_entries(dir).await? {
        let is_scope_dir =
            name.starts_with('@') && !fs::symlink_metadata(&path).await?.is_symlink();
        if is_scope_dir && dir_entries(&path).await?.is_empty() {
            fs::remove_dir(&path).await?;
        }
    }
    Ok(())
}

/// Like [`package_entries`], but bin names may start with a dot.
async fn package_entries_including_bins(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let is_bin_dir = dir.file_name().is_some_and(|name| name == ".bin");
    if is_bin_dir {
        return dir_entries(dir).await;
    }
    package_entries(dir).await
}

async fn dir_entries(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut listed = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        listed.push((
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        ));
    }
    Ok(listed)
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// `app -> a -> b`, plus an orphaned `c` and a hoisted link to it.
    fn layout(node_modules: &Path) {
        let craft = node_modules.join(".craft");
        for entry in ["a@1.0.0", "b@1.0.0", "c@1.0.0"] {
            let name = entry.split('@').next().unwrap();
            std::fs::create_dir_all(craft.join(entry).join("node_modules").join(name)).unwrap();
        }
        std::fs::create_dir_all(craft.join("node_modules")).unwrap();
        symlink(".craft/a@1.0.0/node_modules/a", node_modules.join("a")).unwrap();
        symlink(
            "../../b@1.0.0/node_modules/b",
            craft.join("a@1.0.0/node_modules/b"),
        )
        .unwrap();
        symlink("../c@1.0.0/node_modules/c", craft.join("node_modules/c")).unwrap();
        std::fs::create_dir_all(node_modules.join(".cache/tool")).unwrap();
    }

    #[tokio::test]
    async fn test_removes_unreachable_entries_and_their_links() {
        let dir = tempfile::tempdir().unwrap();
        let node_modules = dir.path().join("node_modules");
        layout(&node_modules);

        prune_node_modules(&node_modules, None).await.unwrap();

        let craft = node_modules.join(".craft");
        assert!(craft.join("a@1.0.0").exists());
        assert!(craft.join("b@1.0.0").exists());
        assert!(!craft.join("c@1.0.0").exists());
        assert!(std::fs::symlink_metadata(craft.join("node_modules/c")).is_err());
        assert!(node_modules.join(".cache/tool").exists());
    }

    #[tokio::test]
    async fn test_removes_top_level_links_outside_the_kept_roots() {
        let dir = tempfile::tempdir().unwrap();
        let node_modules = dir.path().join("node_modules");
        layout(&node_modules);
        std::fs::create_dir_all(node_modules.join("@scope")).unwrap();
        symlink("/nonexistent/old-layout", node_modules.join("@scope/old")).unwrap();
        std::fs::create_dir_all(node_modules.join("real-dir")).unwrap();
        std::fs::create_dir_all(node_modules.join(".bin")).unwrap();
        symlink("../a/cli.js", node_modules.join(".bin/a")).unwrap();

        prune_node_modules(&node_modules, Some(&HashSet::new()))
            .await
            .unwrap();

        assert!(std::fs::symlink_metadata(node_modules.join("a")).is_err());
        assert!(!node_modules.join("@scope").exists());
        assert!(node_modules.join("real-dir").exists());
        assert!(std::fs::symlink_metadata(node_modules.join(".bin/a")).is_err());
        let remaining: Vec<String> = std::fs::read_dir(node_modules.join(".craft"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(remaining, ["node_modules"]);
    }
}
