//! Package bins in `node_modules/.bin`: relative symlinks on Unix, `cmd-shim` style shims on
//! Windows. Callers name a bin by its plain name either way.

use std::io;
use std::path::{Path, PathBuf};

use crate::context::PathContext;

/// Makes `link` run `source`, replacing whatever was there.
pub async fn link_bin(source: &Path, link: &Path) -> io::Result<()> {
    platform::link_bin(source, link).await.at("link bin", link)
}

/// The file the bin at `link` runs, as an absolute path; `None` when `link` isn't a bin.
pub async fn bin_target(link: &Path) -> Option<PathBuf> {
    platform::bin_target(link).await
}

pub async fn remove_bin(link: &Path) -> io::Result<()> {
    platform::remove_bin(link).await.at("remove bin", link)
}

/// The names of the bins in `bin_dir`, sorted; empty when it doesn't exist.
pub async fn bin_names(bin_dir: &Path) -> io::Result<Vec<String>> {
    let mut entries = match tokio::fs::read_dir(bin_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).at("read", bin_dir),
    };
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        names.extend(platform::bin_name(&file_name));
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// The file to execute for the bin `name` in `bin_dir`.
pub fn bin_command(bin_dir: &Path, name: &str) -> PathBuf {
    platform::bin_command(bin_dir, name)
}

#[cfg(unix)]
mod platform {
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use tokio::fs;

    use crate::links::{LinkKind, force_symlink, read_link_resolved, remove_entry_inner};
    use crate::relative_target;

    pub(super) async fn link_bin(source: &Path, link: &Path) -> io::Result<()> {
        force_symlink(&relative_target(link, source), link, LinkKind::File).await?;
        if let Ok(metadata) = fs::metadata(source).await {
            let mode = metadata.permissions().mode() | 0o111;
            let _ = fs::set_permissions(source, std::fs::Permissions::from_mode(mode)).await;
        }
        Ok(())
    }

    pub(super) async fn bin_target(link: &Path) -> Option<PathBuf> {
        read_link_resolved(link).await.ok()
    }

    pub(super) async fn remove_bin(link: &Path) -> io::Result<()> {
        remove_entry_inner(link).await.map(|_| ())
    }

    pub(super) fn bin_name(file_name: &str) -> Option<String> {
        Some(file_name.to_string())
    }

    pub(super) fn bin_command(bin_dir: &Path, name: &str) -> PathBuf {
        bin_dir.join(name)
    }
}

#[cfg(windows)]
mod platform {
    use std::io::{self, Read};
    use std::path::{Path, PathBuf};

    use crate::links::{ensure_dir_inner, remove_entry_inner};
    use crate::{normalize_lexically, relative_target, shims, write::write_if_changed_inner};

    const SHIM_EXTENSIONS: [&str; 2] = ["cmd", "ps1"];

    pub(super) async fn link_bin(source: &Path, link: &Path) -> io::Result<()> {
        if let Some(parent) = link.parent() {
            ensure_dir_inner(parent).await?;
        }
        let target = relative_target(link, source)
            .to_string_lossy()
            .replace('\\', "/");
        let program = shims::shebang_program(&first_line(source));
        let program = program.as_ref();
        write_if_changed_inner(
            &link.with_extension("cmd"),
            shims::cmd(&target, program).as_bytes(),
        )
        .await?;
        write_if_changed_inner(
            &link.with_extension("ps1"),
            shims::powershell(&target, program).as_bytes(),
        )
        .await?;
        write_if_changed_inner(link, shims::sh(&target, program).as_bytes()).await?;
        Ok(())
    }

    pub(super) async fn bin_target(link: &Path) -> Option<PathBuf> {
        let shim = tokio::fs::read_to_string(link.with_extension("cmd"))
            .await
            .ok()?;
        let target = shims::cmd_target(&shim)?;
        Some(normalize_lexically(&link.parent()?.join(target)))
    }

    pub(super) async fn remove_bin(link: &Path) -> io::Result<()> {
        remove_entry_inner(link).await?;
        for extension in SHIM_EXTENSIONS {
            remove_entry_inner(&link.with_extension(extension)).await?;
        }
        Ok(())
    }

    pub(super) fn bin_name(file_name: &str) -> Option<String> {
        file_name.strip_suffix(".cmd").map(str::to_string)
    }

    pub(super) fn bin_command(bin_dir: &Path, name: &str) -> PathBuf {
        bin_dir.join(format!("{name}.cmd"))
    }

    fn first_line(source: &Path) -> String {
        let mut head = [0; 256];
        let read = std::fs::File::open(source)
            .and_then(|mut file| file.read(&mut head))
            .unwrap_or(0);
        let head = String::from_utf8_lossy(&head[..read]);
        head.lines().next().unwrap_or_default().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_links_lists_reads_and_removes_bins() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("node_modules/cowsay");
        std::fs::create_dir_all(&package).unwrap();
        let source = package.join("cli.js");
        std::fs::write(&source, "#!/usr/bin/env node\nconsole.log('moo')\n").unwrap();
        let bin_dir = dir.path().join("node_modules/.bin");

        link_bin(&source, &bin_dir.join("cowsay")).await.unwrap();
        link_bin(&source, &bin_dir.join("cowthink")).await.unwrap();
        link_bin(&source, &bin_dir.join("cowsay")).await.unwrap();

        assert_eq!(bin_names(&bin_dir).await.unwrap(), ["cowsay", "cowthink"]);
        assert_eq!(
            bin_target(&bin_dir.join("cowsay")).await,
            Some(source.clone())
        );
        assert!(bin_command(&bin_dir, "cowsay").is_file());

        remove_bin(&bin_dir.join("cowsay")).await.unwrap();
        assert_eq!(bin_names(&bin_dir).await.unwrap(), ["cowthink"]);
        assert_eq!(bin_target(&bin_dir.join("cowsay")).await, None);
        assert!(
            bin_names(&dir.path().join("missing"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_bin_targets_become_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("cli.js");
        std::fs::write(&source, "").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o640)).unwrap();

        link_bin(&source, &dir.path().join(".bin/cli"))
            .await
            .unwrap();

        let mode = std::fs::metadata(&source).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o751);
    }
}
