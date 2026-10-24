use std::ffi::OsString;
use std::path::PathBuf;

use crate::constants::CRAFT_HOME;
use crate::{ContextError, Result};

#[derive(Debug, Clone)]
pub struct Context {
    pub cwd: PathBuf,
    pub craft_home: PathBuf,
    pub verbose: bool,
}

impl Context {
    pub fn new(cwd: PathBuf, craft_home: PathBuf, verbose: bool) -> Self {
        Self {
            cwd,
            craft_home,
            verbose,
        }
    }

    /// `craft_home` comes from `CRAFT_HOME`, falling back to `~/.craft`.
    pub fn from_env(verbose: bool) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let user_home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        let craft_home = resolve_craft_home(std::env::var_os(CRAFT_HOME), user_home)?;

        Ok(Self::new(cwd, craft_home, verbose))
    }

    pub fn package_cache_dir(&self) -> PathBuf {
        self.craft_home.join("packages")
    }

    pub fn node_modules_dir(&self) -> PathBuf {
        self.cwd.join("node_modules")
    }

    pub fn package_json_path(&self) -> PathBuf {
        self.cwd.join("package.json")
    }
}

fn resolve_craft_home(
    craft_home: Option<OsString>,
    user_home: Option<OsString>,
) -> Result<PathBuf> {
    if let Some(craft_home) = craft_home.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(craft_home));
    }

    match user_home.filter(|path| !path.is_empty()) {
        Some(user_home) => Ok(PathBuf::from(user_home).join(".craft")),
        None => Err(ContextError::NoHomeDir.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_craft_home_env_takes_precedence() -> Result<()> {
        let craft_home = resolve_craft_home(Some("/opt/craft".into()), Some("/home/me".into()))?;
        assert_eq!(craft_home, PathBuf::from("/opt/craft"));
        Ok(())
    }

    #[test]
    fn test_craft_home_defaults_to_user_home() -> Result<()> {
        let craft_home = resolve_craft_home(None, Some("/home/me".into()))?;
        assert_eq!(craft_home, PathBuf::from("/home/me/.craft"));
        Ok(())
    }

    #[test]
    fn test_empty_craft_home_is_ignored() -> Result<()> {
        let craft_home = resolve_craft_home(Some("".into()), Some("/home/me".into()))?;
        assert_eq!(craft_home, PathBuf::from("/home/me/.craft"));
        Ok(())
    }

    #[test]
    fn test_missing_home_is_an_error() {
        let result = resolve_craft_home(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_paths_derive_from_context() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        assert_eq!(
            context.package_cache_dir(),
            PathBuf::from("/home/me/.craft/packages")
        );
        assert_eq!(
            context.node_modules_dir(),
            PathBuf::from("/work/app/node_modules")
        );
        assert_eq!(
            context.package_json_path(),
            PathBuf::from("/work/app/package.json")
        );
    }
}
