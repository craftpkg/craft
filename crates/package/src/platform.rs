use contract::HostPlatform;
use serde_json::Value;

use crate::PackageJson;

/// A manifest's `os`, `cpu` and `libc` lists. Each list allows any value it names, or anything
/// but the `!`-negated ones; an empty list allows everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformConstraints {
    pub os: Vec<String>,
    pub cpu: Vec<String>,
    pub libc: Vec<String>,
}

impl PlatformConstraints {
    pub fn from_manifest(manifest: &PackageJson) -> Self {
        Self {
            os: string_list(manifest.other.get("os")),
            cpu: string_list(manifest.other.get("cpu")),
            libc: string_list(manifest.other.get("libc")),
        }
    }

    /// Whether the package can be installed on `host`. `libc` only applies on Linux, as in npm.
    pub fn supports(&self, host: &HostPlatform) -> bool {
        let libc_allowed = match &host.libc {
            Some(libc) if host.os == "linux" => allows(&self.libc, libc),
            _ => true,
        };
        allows(&self.os, &host.os) && allows(&self.cpu, &host.cpu) && libc_allowed
    }
}

fn allows(list: &[String], value: &str) -> bool {
    let is_excluded = list
        .iter()
        .any(|entry| entry.strip_prefix('!') == Some(value));
    if is_excluded {
        return false;
    }
    let mut included = list
        .iter()
        .filter(|entry| !entry.starts_with('!'))
        .peekable();
    let has_inclusions = included.peek().is_some();
    !has_inclusions || included.any(|entry| entry == value)
}

/// Registries serve these as arrays, but a single string turns up in older manifests.
fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(single)) => vec![single.clone()],
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(os: &str, cpu: &str, libc: Option<&str>) -> HostPlatform {
        HostPlatform {
            os: os.to_string(),
            cpu: cpu.to_string(),
            libc: libc.map(str::to_string),
        }
    }

    fn constraints(value: serde_json::Value) -> PlatformConstraints {
        PlatformConstraints::from_manifest(&serde_json::from_value(value).unwrap())
    }

    #[test]
    fn test_matches_listed_platforms() {
        let darwin_arm = constraints(serde_json::json!({ "os": ["darwin"], "cpu": ["arm64"] }));
        assert!(darwin_arm.supports(&host("darwin", "arm64", None)));
        assert!(!darwin_arm.supports(&host("darwin", "x64", None)));
        assert!(!darwin_arm.supports(&host("linux", "arm64", Some("glibc"))));
        assert!(PlatformConstraints::default().supports(&host("aix", "ppc64", None)));
    }

    #[test]
    fn test_negations_exclude_platforms() {
        let not_windows = constraints(serde_json::json!({ "os": ["!win32"] }));
        assert!(not_windows.supports(&host("linux", "x64", Some("glibc"))));
        assert!(!not_windows.supports(&host("win32", "x64", None)));
    }

    #[test]
    fn test_libc_only_applies_on_linux() {
        let musl =
            constraints(serde_json::json!({ "os": "linux", "cpu": ["x64"], "libc": ["musl"] }));
        assert!(musl.supports(&host("linux", "x64", Some("musl"))));
        assert!(!musl.supports(&host("linux", "x64", Some("glibc"))));

        let any_os_glibc = constraints(serde_json::json!({ "libc": ["glibc"] }));
        assert!(any_os_glibc.supports(&host("darwin", "arm64", None)));
    }
}
