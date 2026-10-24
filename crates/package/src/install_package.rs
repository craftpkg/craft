use std::fmt::Display;

use crate::dependency_spec::{is_location, split_name_and_spec};
pub use crate::{NpmPackage, PackageJson};

#[derive(Debug, Clone)]
pub struct InstallPackage {
    pub name: String,
    pub version: Option<String>,
    pub is_dev: bool,
}

impl InstallPackage {
    pub fn new(name: String, version: Option<String>, is_dev: bool) -> Self {
        Self {
            name,
            version,
            is_dev,
        }
    }

    pub fn to_cache_key(&self) -> String {
        format!(
            "{}-{}",
            self.name,
            self.version.clone().unwrap_or("latest".to_string())
        )
    }

    /// Parses a `craft add` argument: `name`, `name@spec`, `@scope/name@spec`, or a bare git
    /// URL, tarball URL or path, which is kept whole as the name.
    pub fn from_literal(literal: &str, is_dev: bool) -> Self {
        if is_location(literal) {
            return Self::new(literal.to_string(), None, is_dev);
        }

        let (name, spec) = split_name_and_spec(literal);
        Self::new(name.to_string(), spec.map(str::to_string), is_dev)
    }
}

impl Display for InstallPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.version {
            Some(version) => write!(f, "{}@{}", self.name, version),
            None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::InstallPackage;

    #[test]
    fn test_new_package() {
        let pkg = InstallPackage::new("react".to_string(), Some("1.0.0".to_string()), false);
        assert_eq!(pkg.name, "react");
        assert_eq!(pkg.version, Some("1.0.0".to_string()));
        assert!(!pkg.is_dev);
    }

    #[test]
    fn test_from_literal_simple() {
        let pkg = InstallPackage::from_literal("react", false);
        assert_eq!(pkg.name, "react");
        assert_eq!(pkg.version, None);
        assert!(!pkg.is_dev);
    }

    #[test]
    fn test_from_literal_with_version() {
        let pkg = InstallPackage::from_literal("react@17.0.2", false);
        assert_eq!(pkg.name, "react");
        assert_eq!(pkg.version, Some("17.0.2".to_string()));
        assert!(!pkg.is_dev);
    }

    #[test]
    fn test_from_literal_scoped() {
        let pkg = InstallPackage::from_literal("@types/node", false);
        assert_eq!(pkg.name, "@types/node");
        assert_eq!(pkg.version, None);
        assert!(!pkg.is_dev);
    }

    #[test]
    fn test_from_literal_scoped_with_version() {
        let pkg = InstallPackage::from_literal("@types/node@18.0.0", false);
        assert_eq!(pkg.name, "@types/node");
        assert_eq!(pkg.version, Some("18.0.0".to_string()));
        assert!(!pkg.is_dev);
    }

    #[test]
    fn test_from_literal_alias() {
        let pkg = InstallPackage::from_literal("sw@npm:string-width@^4.2.0", false);
        assert_eq!(pkg.name, "sw");
        assert_eq!(pkg.version, Some("npm:string-width@^4.2.0".to_string()));
    }

    #[test]
    fn test_from_literal_keeps_locations_whole() {
        for literal in [
            "git+ssh://git@github.com:user/repo.git",
            "github:user/repo",
            "https://example.com/pkg-1.0.0.tgz",
            "./vendor/lib",
        ] {
            let pkg = InstallPackage::from_literal(literal, false);
            assert_eq!(pkg.name, literal);
            assert_eq!(pkg.version, None);
        }
    }

    #[test]
    fn test_from_literal_dev() {
        let pkg = InstallPackage::from_literal("typescript@5.0.0", true);
        assert_eq!(pkg.name, "typescript");
        assert_eq!(pkg.version, Some("5.0.0".to_string()));
        assert!(pkg.is_dev);
    }
}
