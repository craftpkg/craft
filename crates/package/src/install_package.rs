use std::fmt::Display;
use std::path::PathBuf;

use crate::DependencySection;
pub use crate::PackageJson;
use crate::dependency_spec::{is_location, split_name_and_spec};

#[derive(Debug, Clone)]
pub struct InstallPackage {
    pub name: String,
    pub version: Option<String>,
    pub is_dev: bool,
    /// The directory relative `file:` and `link:` specs resolve against; the project directory
    /// when `None`.
    pub base_dir: Option<PathBuf>,
    /// From the project's `optionalDependencies`: skipped instead of failing the install.
    pub is_optional: bool,
    /// Whether a version from craft.lock may answer this request. Off for packages the user
    /// just asked for and for project dependencies whose specifier changed.
    pub prefer_locked: bool,
}

impl InstallPackage {
    pub fn new(name: String, version: Option<String>, is_dev: bool) -> Self {
        Self {
            name,
            version,
            is_dev,
            base_dir: None,
            is_optional: false,
            prefer_locked: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    /// The package.json section a project dependency is declared in.
    pub fn section(&self) -> DependencySection {
        if self.is_optional {
            DependencySection::OptionalDependencies
        } else if self.is_dev {
            DependencySection::DevDependencies
        } else {
            DependencySection::Dependencies
        }
    }

    pub fn without_lock(mut self) -> Self {
        self.prefer_locked = false;
        self
    }

    pub fn with_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = Some(base_dir);
        self
    }

    pub fn to_cache_key(&self) -> String {
        let spec = self.version.as_deref().unwrap_or("latest");
        match &self.base_dir {
            Some(base_dir) => format!("{}-{spec}@{}", self.name, base_dir.display()),
            None => format!("{}-{spec}", self.name),
        }
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
