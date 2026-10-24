pub const CRAFT_HOME: &str = "CRAFT_HOME";
pub const CRAFT_REGISTRY: &str = "CRAFT_REGISTRY";
/// Install platform-specific packages for another OS, CPU or libc than the host's.
pub const CRAFT_OS: &str = "CRAFT_OS";
pub const CRAFT_CPU: &str = "CRAFT_CPU";
pub const CRAFT_LIBC: &str = "CRAFT_LIBC";
pub const CRAFT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Bumped whenever the store's layout changes, so old entries are never misread.
pub const STORE_VERSION: &str = "v1";
pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";
