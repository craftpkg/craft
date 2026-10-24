use crate::constants::CRAFT_VERSION;

/// The platform packages are installed for, named the way Node and package manifests name it
/// (`darwin` / `linux` / `win32`, `x64` / `arm64`, `glibc` / `musl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPlatform {
    pub os: String,
    pub cpu: String,
    /// Only meaningful on Linux; `None` elsewhere.
    pub libc: Option<String>,
}

impl HostPlatform {
    pub fn detect() -> Self {
        let os = node_os(std::env::consts::OS).to_string();
        let libc = (os == "linux").then(detect_libc);
        Self {
            os,
            cpu: node_cpu(std::env::consts::ARCH).to_string(),
            libc,
        }
    }

    /// The detected platform with any of `os` / `cpu` / `libc` overridden, for installing
    /// another platform's packages (e.g. when building a container image).
    pub fn with_overrides(
        mut self,
        os: Option<String>,
        cpu: Option<String>,
        libc: Option<String>,
    ) -> Self {
        if let Some(os) = os.filter(|value| !value.is_empty()) {
            self.os = os;
        }
        if let Some(cpu) = cpu.filter(|value| !value.is_empty()) {
            self.cpu = cpu;
        }
        if let Some(libc) = libc.filter(|value| !value.is_empty()) {
            self.libc = Some(libc);
        }
        self
    }
}

impl std::fmt::Display for HostPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.os, self.cpu)?;
        if let Some(libc) = &self.libc {
            write!(f, "-{libc}")?;
        }
        Ok(())
    }
}

/// npm's format (`<manager>/<version> <platform> <arch>`), sent to registries and given to
/// scripts, which read it to detect the package manager that started them.
pub fn user_agent() -> String {
    let platform = node_os(std::env::consts::OS);
    let arch = node_cpu(std::env::consts::ARCH);
    format!("craft/{CRAFT_VERSION} {platform} {arch}")
}

/// Node's `process.platform` for a Rust `std::env::consts::OS`.
pub fn node_os(os: &str) -> &str {
    match os {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// Node's `process.arch` for a Rust `std::env::consts::ARCH`.
pub fn node_cpu(arch: &str) -> &str {
    match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "ia32",
        "powerpc64" => "ppc64",
        "loongarch64" => "loong64",
        other => other,
    }
}

/// musl systems ship their dynamic loader as `/lib/ld-musl-<arch>.so.1`.
fn detect_libc() -> String {
    let is_musl = std::fs::read_dir("/lib").is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("ld-musl-"))
    });
    if is_musl { "musl" } else { "glibc" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_names_craft() {
        assert!(user_agent().starts_with(&format!("craft/{CRAFT_VERSION} ")));
    }

    #[test]
    fn test_maps_rust_names_to_node_names() {
        assert_eq!(node_os("macos"), "darwin");
        assert_eq!(node_os("linux"), "linux");
        assert_eq!(node_cpu("aarch64"), "arm64");
        assert_eq!(node_cpu("x86_64"), "x64");
    }

    #[test]
    fn test_overrides_replace_detected_values() {
        let host = HostPlatform {
            os: "darwin".to_string(),
            cpu: "arm64".to_string(),
            libc: None,
        }
        .with_overrides(
            Some("linux".to_string()),
            Some(String::new()),
            Some("musl".to_string()),
        );

        assert_eq!(host.to_string(), "linux-arm64-musl");
    }
}
