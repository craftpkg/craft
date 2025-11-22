use std::path::PathBuf;

pub const CRAFT_VERBOSE: &str = "CRAFT_VERBOSE";

/// Get the package cache directory path
/// - macOS/Linux: ~/.craft/packages
/// - Windows: %USERPROFILE%\.craft\packages
pub fn get_package_cache_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("Could not determine home directory");

    PathBuf::from(home).join(".craft").join("packages")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_package_cache_dir() {
        let cache_dir = get_package_cache_dir();
        assert!(cache_dir.to_string_lossy().contains(".craft"));
        assert!(cache_dir.to_string_lossy().contains("packages"));
    }
}
