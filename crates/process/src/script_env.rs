use std::ffi::OsString;
use std::path::Path;

use contract::{Result, user_agent};

use crate::Process;

/// The package a script belongs to, for the `npm_package_*` variables.
pub struct ScriptPackage<'a> {
    pub name: Option<&'a str>,
    pub version: Option<&'a str>,
    pub manifest_path: &'a Path,
}

impl Process {
    /// Runs in `dir` with the environment npm gives scripts: every `node_modules/.bin` from
    /// `dir` upwards in front of `PATH`, `INIT_CWD`, `npm_config_user_agent` and the package's
    /// `npm_package_*` variables.
    pub fn script_env(self, dir: &Path, init_cwd: &Path, package: &ScriptPackage) -> Result<Self> {
        let mut process = self
            .current_dir(dir)
            .env("PATH", script_path(dir)?)
            .env("INIT_CWD", init_cwd)
            .env("npm_config_user_agent", user_agent())
            .env("npm_package_json", package.manifest_path);
        if let Some(name) = package.name {
            process = process.env("npm_package_name", name);
        }
        if let Some(version) = package.version {
            process = process.env("npm_package_version", version);
        }
        Ok(process)
    }

    /// Sets the variables naming the lifecycle event being run.
    pub fn lifecycle(self, event: &str, script: &str) -> Self {
        self.env("npm_lifecycle_event", event)
            .env("npm_lifecycle_script", script)
    }
}

/// `PATH` with every `node_modules/.bin` from `dir` up to the root in front, as npm sets it.
pub fn script_path(dir: &Path) -> Result<OsString> {
    let bin_dirs = dir
        .ancestors()
        .map(|ancestor| ancestor.join("node_modules").join(".bin"));
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(bin_dirs.chain(std::env::split_paths(&inherited)))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_path_starts_with_bin_dirs_from_dir_up() {
        let path = script_path(Path::new("/work/app")).unwrap();
        let entries: Vec<PathBuf> = std::env::split_paths(&path).take(3).collect();
        assert_eq!(
            entries,
            [
                PathBuf::from("/work/app/node_modules/.bin"),
                PathBuf::from("/work/node_modules/.bin"),
                PathBuf::from("/node_modules/.bin"),
            ]
        );
    }
}
