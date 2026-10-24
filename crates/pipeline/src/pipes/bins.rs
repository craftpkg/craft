use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use package::{PackageBin, PackageJson};

/// The bins a package declares, normalized the way npm does: each name is reduced to its last
/// path segment (`@scope/tool` → `tool`, `../../x` → `x`) and each target is kept inside the
/// package. Without a `bin` field, every file under `directories.bin` becomes a bin.
pub(crate) fn package_bins(
    package_name: &str,
    manifest: &PackageJson,
    package_dir: &Path,
) -> BTreeMap<String, PathBuf> {
    let declared: Vec<(String, String)> = match &manifest.bin {
        Some(PackageBin::String(target)) => vec![(package_name.to_string(), target.clone())],
        Some(PackageBin::Map(bins)) => bins
            .iter()
            .map(|(name, target)| (name.clone(), target.clone()))
            .collect(),
        None => return directory_bins(manifest, package_dir),
    };

    declared
        .into_iter()
        .filter_map(|(name, target)| Some((bin_name(&name)?, contained_path(&target)?)))
        .collect()
}

fn directory_bins(manifest: &PackageJson, package_dir: &Path) -> BTreeMap<String, PathBuf> {
    let Some(bin_dir) = manifest
        .other
        .get("directories")
        .and_then(|directories| directories.get("bin"))
        .and_then(|bin_dir| bin_dir.as_str())
        .and_then(contained_path)
    else {
        return BTreeMap::new();
    };

    let mut files = Vec::new();
    collect_files(package_dir, &bin_dir, &mut files);
    files
        .into_iter()
        .filter_map(|file| {
            let name = bin_name(&file.file_name()?.to_string_lossy())?;
            Some((name, file))
        })
        .collect()
}

/// Files under `package_dir/relative_dir`, as paths relative to `package_dir`.
fn collect_files(package_dir: &Path, relative_dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(package_dir.join(relative_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let relative = relative_dir.join(entry.file_name());
        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => collect_files(package_dir, &relative, files),
            Ok(_) => files.push(relative),
            Err(_) => {}
        }
    }
}

fn bin_name(raw: &str) -> Option<String> {
    let normalized = raw.replace(['\\', ':'], "/");
    let name = normalized.rsplit('/').next()?;
    let is_usable = !name.is_empty() && name != "." && name != "..";
    is_usable.then(|| name.to_string())
}

/// `raw` as a relative path that cannot climb out of the package: `..` stops at the package
/// root, as `path.join('/', raw)` does in npm.
fn contained_path(raw: &str) -> Option<PathBuf> {
    let normalized = raw.replace('\\', "/");
    let mut contained = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => contained.push(part),
            Component::ParentDir => {
                contained.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }

    let is_empty = contained.as_os_str().is_empty();
    (!is_empty).then_some(contained)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(value: serde_json::Value) -> PackageJson {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn test_string_bin_of_a_scoped_package_uses_the_unscoped_name() {
        let bins = package_bins(
            "@scope/tool",
            &manifest(serde_json::json!({ "bin": "./cli.js" })),
            Path::new("/unused"),
        );
        assert_eq!(
            bins,
            BTreeMap::from([("tool".to_string(), PathBuf::from("cli.js"))])
        );
    }

    #[test]
    fn test_names_and_targets_cannot_escape() {
        let bins = package_bins(
            "evil",
            &manifest(serde_json::json!({
                "bin": {
                    "../../outside": "../../../etc/passwd",
                    "C:\\\\win": "bin\\\\win.js",
                    "..": "cli.js",
                    "empty-target": "..",
                }
            })),
            Path::new("/unused"),
        );
        assert_eq!(
            bins,
            BTreeMap::from([
                ("outside".to_string(), PathBuf::from("etc/passwd")),
                ("win".to_string(), PathBuf::from("bin/win.js")),
            ])
        );
    }

    #[test]
    fn test_directories_bin_lists_every_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("scripts/nested")).unwrap();
        std::fs::write(dir.path().join("scripts/build"), "").unwrap();
        std::fs::write(dir.path().join("scripts/nested/deploy"), "").unwrap();

        let bins = package_bins(
            "tool",
            &manifest(serde_json::json!({ "directories": { "bin": "./scripts" } })),
            dir.path(),
        );

        assert_eq!(
            bins,
            BTreeMap::from([
                ("build".to_string(), PathBuf::from("scripts/build")),
                ("deploy".to_string(), PathBuf::from("scripts/nested/deploy")),
            ])
        );
    }

    #[test]
    fn test_bin_field_takes_precedence_over_directories_bin() {
        let bins = package_bins(
            "tool",
            &manifest(serde_json::json!({
                "bin": { "tool": "cli.js" },
                "directories": { "bin": "scripts" },
            })),
            Path::new("/unused"),
        );
        assert_eq!(bins.keys().collect::<Vec<_>>(), ["tool"]);
    }
}
