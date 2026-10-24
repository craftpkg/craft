use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::constants::{
    CRAFT_CPU, CRAFT_HOME, CRAFT_LIBC, CRAFT_OS, CRAFT_REGISTRY, STORE_VERSION,
};
use crate::{ContextError, Result};
use crate::{HostPlatform, RegistryConfig};
use progress::Reporter;

#[derive(Debug, Clone)]
pub struct Context {
    pub cwd: PathBuf,
    pub craft_home: PathBuf,
    pub registry: RegistryConfig,
    pub debug: bool,
    /// Whether craft runs in CI (`CI` is set and not `0`/`false`), where installs are frozen.
    pub ci: bool,
    /// The platform optional and platform-specific packages are chosen for.
    pub platform: HostPlatform,
    /// Where install progress and results go.
    pub reporter: Reporter,
}

impl Context {
    pub fn new(cwd: PathBuf, craft_home: PathBuf, debug: bool) -> Self {
        Self {
            cwd,
            craft_home,
            registry: RegistryConfig::default(),
            debug,
            ci: false,
            platform: HostPlatform::detect(),
            reporter: Reporter::default(),
        }
    }

    /// `craft_home` comes from `CRAFT_HOME`, falling back to `~/.craft`. Registry settings come
    /// from `.npmrc` files and `npm_config_*` variables, with `CRAFT_REGISTRY` overriding the
    /// default registry.
    pub fn from_env(debug: bool) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let user_home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        let env: HashMap<String, String> = std::env::vars().collect();
        let registry = RegistryConfig::load(&cwd, user_home.as_deref().map(Path::new), &env);
        let craft_home = resolve_craft_home(std::env::var_os(CRAFT_HOME), user_home)?;
        let platform = HostPlatform::detect().with_overrides(
            std::env::var(CRAFT_OS).ok(),
            std::env::var(CRAFT_CPU).ok(),
            std::env::var(CRAFT_LIBC).ok(),
        );
        let context = Self {
            ci: is_ci(std::env::var("CI").ok().as_deref()),
            platform,
            registry,
            ..Self::new(cwd, craft_home, debug)
        };

        match std::env::var(CRAFT_REGISTRY) {
            Ok(registry) if !registry.is_empty() => Ok(context.with_registry(&registry)),
            _ => Ok(context),
        }
    }

    pub fn with_registry(mut self, registry: &str) -> Self {
        self.registry = self.registry.with_default_registry(registry);
        self
    }

    /// The global store of extracted packages, shared by every project.
    pub fn store_dir(&self) -> PathBuf {
        self.craft_home.join("store").join(STORE_VERSION)
    }

    /// Registry metadata kept between installs, revalidated with ETags.
    pub fn metadata_cache_dir(&self) -> PathBuf {
        self.craft_home.join("cache").join("metadata")
    }

    /// Where packages were cached before the store; no longer written, only cleaned.
    pub fn legacy_cache_dir(&self) -> PathBuf {
        self.craft_home.join("packages")
    }

    pub fn node_modules_dir(&self) -> PathBuf {
        self.cwd.join("node_modules")
    }

    pub fn package_json_path(&self) -> PathBuf {
        self.cwd.join("package.json")
    }
}

fn is_ci(value: Option<&str>) -> bool {
    let value = value.unwrap_or_default().trim();
    let is_disabled = value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false");
    !is_disabled
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
    fn test_ci_detection() {
        assert!(is_ci(Some("true")));
        assert!(is_ci(Some("1")));
        assert!(!is_ci(Some("false")));
        assert!(!is_ci(Some("0")));
        assert!(!is_ci(Some("")));
        assert!(!is_ci(None));
    }

    #[test]
    fn test_missing_home_is_an_error() {
        let result = resolve_craft_home(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_defaults_to_npm() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        assert_eq!(
            context.registry.default_registry,
            "https://registry.npmjs.org"
        );
    }

    #[test]
    fn test_with_registry_strips_trailing_slash() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false)
            .with_registry("http://127.0.0.1:4873/");
        assert_eq!(context.registry.default_registry, "http://127.0.0.1:4873");
    }

    #[test]
    fn test_paths_derive_from_context() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        assert_eq!(
            context.store_dir(),
            PathBuf::from("/home/me/.craft/store/v1")
        );
        assert_eq!(
            context.node_modules_dir(),
            PathBuf::from("/work/app/node_modules")
        );
        assert_eq!(
            context.package_json_path(),
            PathBuf::from("/work/app/package.json")
        );
        assert_eq!(
            context.legacy_cache_dir(),
            PathBuf::from("/home/me/.craft/packages")
        );
    }
}
