use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::constants::DEFAULT_REGISTRY;

/// Registry settings from `.npmrc` files and the environment: where each package is fetched
/// from, which credentials go to which host, and how to reach the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    pub default_registry: String,
    /// `@scope` → registry URL.
    pub scoped_registries: HashMap<String, String>,
    /// `//host/path/` → credentials for URLs under it.
    credentials: HashMap<String, Credentials>,
    pub https_proxy: Option<String>,
    pub http_proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub strict_ssl: bool,
    pub cafile: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Credentials {
    Token(String),
    /// Base64 of `username:password`, sent as HTTP Basic auth.
    Basic(String),
}

/// Never print the secret itself.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.write_str("Token(<redacted>)"),
            Self::Basic(_) => f.write_str("Basic(<redacted>)"),
        }
    }
}

impl Credentials {
    /// The `Authorization` header value.
    pub fn header(&self) -> String {
        match self {
            Self::Token(token) => format!("Bearer {token}"),
            Self::Basic(encoded) => format!("Basic {encoded}"),
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            default_registry: DEFAULT_REGISTRY.to_string(),
            scoped_registries: HashMap::new(),
            credentials: HashMap::new(),
            https_proxy: None,
            http_proxy: None,
            no_proxy: None,
            strict_ssl: true,
            cafile: None,
        }
    }
}

impl RegistryConfig {
    /// Reads `~/.npmrc` (or `NPM_CONFIG_USERCONFIG`) then `<project>/.npmrc`, each overriding
    /// the last, then `npm_config_*` variables; `${VAR}` in values is taken from `env`.
    pub fn load(
        project_dir: &Path,
        user_home: Option<&Path>,
        env: &HashMap<String, String>,
    ) -> Self {
        let mut config = Self::default();
        let user_config = env_value(env, "npm_config_userconfig")
            .map(PathBuf::from)
            .or_else(|| user_home.map(|home| home.join(".npmrc")));
        for path in user_config.iter().chain([&project_dir.join(".npmrc")]) {
            if let Ok(content) = std::fs::read_to_string(path) {
                config.apply_npmrc(&content, env);
            }
        }
        for (key, value) in env {
            let lowercase = key.to_ascii_lowercase();
            if let Some(setting) = lowercase.strip_prefix("npm_config_") {
                config.apply(&setting.replace('_', "-"), value);
            }
        }
        config
    }

    pub fn with_default_registry(mut self, registry: &str) -> Self {
        self.default_registry = trim_registry(registry);
        self
    }

    /// The registry a package is fetched from: its scope's, else the default.
    pub fn registry_for(&self, package_name: &str) -> &str {
        package_name
            .split_once('/')
            .filter(|(scope, _)| scope.starts_with('@'))
            .and_then(|(scope, _)| self.scoped_registries.get(scope))
            .unwrap_or(&self.default_registry)
    }

    /// The credentials configured for the longest `//host/path/` prefix of `url`, if any.
    pub fn credentials_for(&self, url: &str) -> Option<&Credentials> {
        let without_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
        let target = format!("//{without_scheme}");
        self.credentials
            .iter()
            .filter(|(prefix, _)| target.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, credentials)| credentials)
    }

    fn apply_npmrc(&mut self, content: &str, env: &HashMap<String, String>) {
        let mut usernames: HashMap<String, String> = HashMap::new();
        let mut passwords: HashMap<String, String> = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            let is_skipped = line.is_empty() || line.starts_with(['#', ';', '[']);
            if is_skipped {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = expand_env(unquote(value.trim()), env);
            let key = key.trim();
            match key.rsplit_once(":") {
                Some((prefix, "username")) if prefix.starts_with("//") => {
                    usernames.insert(normalize_prefix(prefix), value);
                }
                Some((prefix, "_password")) if prefix.starts_with("//") => {
                    passwords.insert(normalize_prefix(prefix), value);
                }
                _ => self.apply(key, &value),
            }
        }

        for (prefix, username) in usernames {
            let Some(password) = passwords
                .get(&prefix)
                .and_then(|password| decode_base64(password))
            else {
                continue;
            };
            let encoded = encode_base64(format!("{username}:{password}").as_bytes());
            self.credentials.insert(prefix, Credentials::Basic(encoded));
        }
    }

    fn apply(&mut self, key: &str, value: &str) {
        if let Some((prefix, setting)) = key.rsplit_once(':')
            && prefix.starts_with("//")
        {
            let credentials = match setting {
                "_authToken" => Credentials::Token(value.to_string()),
                "_auth" => Credentials::Basic(value.to_string()),
                _ => return,
            };
            self.credentials
                .insert(normalize_prefix(prefix), credentials);
            return;
        }
        if let Some(scope) = key
            .strip_suffix(":registry")
            .filter(|scope| scope.starts_with('@'))
        {
            self.scoped_registries
                .insert(scope.to_string(), trim_registry(value));
            return;
        }

        match key {
            "registry" => self.default_registry = trim_registry(value),
            "_authToken" => {
                let prefix = normalize_prefix(&nerf_dart(&self.default_registry));
                self.credentials
                    .insert(prefix, Credentials::Token(value.to_string()));
            }
            "https-proxy" => self.https_proxy = Some(value.to_string()),
            "proxy" => self.http_proxy = Some(value.to_string()),
            "noproxy" | "no-proxy" => self.no_proxy = Some(value.to_string()),
            "strict-ssl" => self.strict_ssl = value != "false",
            "cafile" => self.cafile = Some(PathBuf::from(value)),
            _ => {}
        }
    }
}

fn env_value(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty())
}

/// `https://npm.example.com/path` → `//npm.example.com/path/`, the key npm stores credentials
/// under.
fn nerf_dart(registry: &str) -> String {
    let without_scheme = registry
        .split_once("://")
        .map_or(registry, |(_, rest)| rest);
    format!("//{without_scheme}")
}

fn normalize_prefix(prefix: &str) -> String {
    if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    }
}

fn trim_registry(registry: &str) -> String {
    registry.trim_end_matches('/').to_string()
}

fn unquote(value: &str) -> &str {
    let is_quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if is_quoted {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// Replaces `${NAME}` with the variable's value, or nothing when unset, as npm does.
fn expand_env(value: &str, env: &HashMap<String, String>) -> String {
    let mut expanded = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let Some(length) = rest[start..].find('}') else {
            break;
        };
        expanded.push_str(&rest[..start]);
        let name = &rest[start + 2..start + length];
        expanded.push_str(env.get(name).map(String::as_str).unwrap_or_default());
        rest = &rest[start + length + 1..];
    }
    expanded.push_str(rest);
    expanded
}

fn encode_base64(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

fn decode_base64(encoded: &str) -> Option<String> {
    String::from_utf8(BASE64.decode(encoded).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(npmrc: &str, env: &[(&str, &str)]) -> RegistryConfig {
        let env: HashMap<String, String> = env
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        let mut config = RegistryConfig::default();
        config.apply_npmrc(npmrc, &env);
        config
    }

    #[test]
    fn test_scoped_registries_and_defaults() {
        let config = config(
            "registry=https://registry.example.com/\n@acme:registry=https://npm.pkg.github.com\n",
            &[],
        );
        assert_eq!(config.registry_for("react"), "https://registry.example.com");
        assert_eq!(
            config.registry_for("@acme/ui"),
            "https://npm.pkg.github.com"
        );
        assert_eq!(
            config.registry_for("@types/node"),
            "https://registry.example.com"
        );
    }

    #[test]
    fn test_tokens_match_the_longest_host_prefix_and_expand_env() {
        let config = config(
            "# comment\n; also a comment\n//npm.pkg.github.com/:_authToken=${GH_TOKEN}\n//npm.pkg.github.com/acme/:_authToken=\"scoped\"\n",
            &[("GH_TOKEN", "ghp_secret")],
        );
        let header = |url: &str| config.credentials_for(url).map(Credentials::header);

        assert_eq!(
            header("https://npm.pkg.github.com/@other%2fpkg"),
            Some("Bearer ghp_secret".to_string())
        );
        assert_eq!(
            header("https://npm.pkg.github.com/acme/pkg/-/pkg-1.0.0.tgz"),
            Some("Bearer scoped".to_string())
        );
        assert_eq!(header("https://registry.npmjs.org/react"), None);
    }

    #[test]
    fn test_basic_auth_from_username_and_password() {
        let config = config(
            "//nexus.example.com/repository/npm/:username=ci\n//nexus.example.com/repository/npm/:_password=c2VjcmV0\n//legacy.example.com/:_auth=Y2k6c2VjcmV0\n",
            &[],
        );
        let header = |url: &str| config.credentials_for(url).map(Credentials::header);

        assert_eq!(
            header("https://nexus.example.com/repository/npm/react"),
            Some(format!("Basic {}", encode_base64(b"ci:secret")))
        );
        assert_eq!(
            header("https://legacy.example.com/react"),
            Some("Basic Y2k6c2VjcmV0".to_string())
        );
    }

    #[test]
    fn test_legacy_token_applies_to_the_default_registry() {
        let config = config(
            "registry=https://registry.example.com/npm/\n_authToken=abc\n",
            &[],
        );
        assert_eq!(
            config
                .credentials_for("https://registry.example.com/npm/react")
                .map(Credentials::header),
            Some("Bearer abc".to_string())
        );
    }

    #[test]
    fn test_network_settings() {
        let config = config(
            "https-proxy=http://proxy.corp:8080\nstrict-ssl=false\ncafile=/etc/corp-ca.pem\nnoproxy=localhost\n",
            &[],
        );
        assert_eq!(
            config.https_proxy.as_deref(),
            Some("http://proxy.corp:8080")
        );
        assert!(!config.strict_ssl);
        assert_eq!(config.cafile, Some(PathBuf::from("/etc/corp-ca.pem")));
        assert_eq!(config.no_proxy.as_deref(), Some("localhost"));
    }

    #[test]
    fn test_load_layers_user_project_and_env() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join(".npmrc"),
            "registry=https://user.example.com\n@acme:registry=https://user-acme.example.com\n",
        )
        .unwrap();
        std::fs::write(
            project.path().join(".npmrc"),
            "@acme:registry=https://project-acme.example.com\n",
        )
        .unwrap();
        let env = HashMap::from([(
            "NPM_CONFIG_REGISTRY".to_string(),
            "https://env.example.com/".to_string(),
        )]);

        let config = RegistryConfig::load(project.path(), Some(home.path()), &env);

        assert_eq!(config.default_registry, "https://env.example.com");
        assert_eq!(
            config.registry_for("@acme/ui"),
            "https://project-acme.example.com"
        );
    }

    #[test]
    fn test_credentials_never_print() {
        let credentials = Credentials::Token("ghp_secret".to_string());
        assert_eq!(format!("{credentials:?}"), "Token(<redacted>)");
    }

    #[test]
    fn test_base64_round_trip() {
        for text in ["", "a", "ab", "abc", "ci:secret"] {
            assert_eq!(
                decode_base64(&encode_base64(text.as_bytes())).as_deref(),
                Some(text)
            );
        }
    }
}
