use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

use contract::TarballError;
use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

const EXECUTABLE_MODE: u32 = 0o755;
const EXECUTE_BITS: u32 = 0o111;

/// Extracts an npm tarball into `dest`, dropping the root folder its entries sit under:
/// `package/` for most packages, but `@types/*` use `node/` and older packages other names.
pub async fn unzip(source: PathBuf, dest: PathBuf) -> contract::Result<()> {
    tokio::task::spawn_blocking(move || extract(&source, &dest)).await?
}

fn extract(source: &Path, dest: &Path) -> contract::Result<()> {
    let mut archive = Archive::new(GzDecoder::new(File::open(source)?));
    fs::create_dir_all(dest)?;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let Some(relative_path) = strip_root(&entry_path)? else {
            continue;
        };
        let path = dest.join(relative_path);

        match entry.header().entry_type() {
            EntryType::Directory => fs::create_dir_all(&path)?,
            EntryType::Regular => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut file = File::create(&path)?;
                io::copy(&mut entry, &mut file)?;

                let is_executable = entry
                    .header()
                    .mode()
                    .is_ok_and(|mode| mode & EXECUTE_BITS != 0);
                if is_executable {
                    make_executable(&path)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// The entry's path below the tarball's root folder, or `None` for the root folder itself.
fn strip_root(entry_path: &Path) -> Result<Option<PathBuf>, TarballError> {
    let unsafe_path = || TarballError::UnsafePath {
        path: entry_path.display().to_string(),
    };
    let mut components = entry_path
        .components()
        .skip_while(|component| *component == Component::CurDir);

    match components.next() {
        Some(Component::Normal(_)) => {}
        None => return Ok(None),
        Some(_) => return Err(unsafe_path()),
    }

    let mut relative_path = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(part) => relative_path.push(part),
            Component::CurDir => {}
            _ => return Err(unsafe_path()),
        }
    }

    let is_root_folder = relative_path.as_os_str().is_empty();
    Ok((!is_root_folder).then_some(relative_path))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(EXECUTABLE_MODE))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    #[tokio::test]
    async fn test_strips_the_package_root() {
        let dir = tempfile::tempdir().unwrap();
        let source = tarball(
            dir.path(),
            &[
                Entry::dir("package/"),
                Entry::file("package/package.json", "{}"),
                Entry::file("package/lib/index.js", "module.exports = 1;"),
            ],
        );
        let dest = dir.path().join("output");

        unzip(source, dest.clone()).await.unwrap();

        assert_eq!(fs::read_to_string(dest.join("package.json")).unwrap(), "{}");
        assert_eq!(
            fs::read_to_string(dest.join("lib/index.js")).unwrap(),
            "module.exports = 1;"
        );
        assert!(!dest.join("package").exists());
    }

    #[tokio::test]
    async fn test_strips_a_non_standard_root() {
        let dir = tempfile::tempdir().unwrap();
        let source = tarball(
            dir.path(),
            &[
                Entry::file("./node/index.d.ts", "export {};"),
                Entry::file("node/package.json", "{}"),
            ],
        );
        let dest = dir.path().join("output");

        unzip(source, dest.clone()).await.unwrap();

        assert!(dest.join("index.d.ts").exists());
        assert!(dest.join("package.json").exists());
        assert!(!dest.join("node").exists());
    }

    #[tokio::test]
    async fn test_rejects_entries_escaping_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = tarball(dir.path(), &[Entry::file("package/../../evil.js", "")]);
        let dest = dir.path().join("output");

        let error = unzip(source, dest).await.unwrap_err();

        assert_eq!(
            error.to_string(),
            "tarball entry escapes the package directory: package/../../evil.js"
        );
        assert!(!dir.path().join("evil.js").exists());
    }

    #[tokio::test]
    async fn test_skips_links() {
        let dir = tempfile::tempdir().unwrap();
        let source = tarball(
            dir.path(),
            &[
                Entry::file("package/index.js", ""),
                Entry::symlink("package/passwd", "/etc/passwd"),
            ],
        );
        let dest = dir.path().join("output");

        unzip(source, dest.clone()).await.unwrap();

        assert!(dest.join("index.js").exists());
        assert!(fs::symlink_metadata(dest.join("passwd")).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_normalizes_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let source = tarball(
            dir.path(),
            &[
                Entry::dir("package/locked/").mode(0o000),
                Entry::file("package/locked/index.js", "").mode(0o000),
                Entry::file("package/cli.js", "").mode(0o700),
            ],
        );
        let dest = dir.path().join("output");

        unzip(source, dest.clone()).await.unwrap();

        let mode = |path: &str| fs::metadata(dest.join(path)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("cli.js"), 0o755);
        assert_ne!(mode("locked") & 0o700, 0);
        assert_ne!(mode("locked/index.js") & 0o600, 0);
    }

    struct Entry {
        path: &'static str,
        kind: EntryType,
        contents: &'static str,
        mode: u32,
    }

    impl Entry {
        fn file(path: &'static str, contents: &'static str) -> Self {
            Self {
                path,
                kind: EntryType::Regular,
                contents,
                mode: 0o644,
            }
        }

        fn dir(path: &'static str) -> Self {
            Self {
                path,
                kind: EntryType::Directory,
                contents: "",
                mode: 0o755,
            }
        }

        fn symlink(path: &'static str, target: &'static str) -> Self {
            Self {
                path,
                kind: EntryType::Symlink,
                contents: target,
                mode: 0o777,
            }
        }

        fn mode(self, mode: u32) -> Self {
            Self { mode, ..self }
        }
    }

    /// Writes entry paths verbatim, since `tar::Builder` refuses the unsafe ones under test.
    fn tarball(dir: &Path, entries: &[Entry]) -> PathBuf {
        let source = dir.join("test.tgz");
        let mut builder = tar::Builder::new(GzEncoder::new(
            File::create(&source).unwrap(),
            Compression::default(),
        ));

        for entry in entries {
            let mut header = tar::Header::new_gnu();
            let name = &mut header.as_old_mut().name;
            name[..entry.path.len()].copy_from_slice(entry.path.as_bytes());
            header.set_entry_type(entry.kind);
            header.set_mode(entry.mode);

            if entry.kind == EntryType::Symlink {
                header.set_link_name(entry.contents).unwrap();
                header.set_size(0);
                header.set_cksum();
                builder.append(&header, io::empty()).unwrap();
            } else {
                header.set_size(entry.contents.len() as u64);
                header.set_cksum();
                builder.append(&header, entry.contents.as_bytes()).unwrap();
            }
        }

        builder.into_inner().unwrap().finish().unwrap();
        source
    }
}
