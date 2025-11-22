pub use crate::{NpmPackage, PackageJson};

#[derive(Debug)]
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

    pub fn from_literal(package: &str, is_dev: bool) -> Self {
        // Handle scoped packages like @scope/package@version
        // Find the last @ symbol to split name and version
        if let Some(last_at) = package.rfind('@') {
            // Check if this @ is part of a scoped package (at the start)
            if last_at == 0 {
                // This is a scoped package without version like @scope/package
                Self {
                    name: package.to_string(),
                    version: None,
                    is_dev,
                }
            } else {
                // Split into name and version
                let (name, version) = package.split_at(last_at);
                Self {
                    name: name.to_string(),
                    version: Some(version[1..].to_string()), // Skip the @ symbol
                    is_dev,
                }
            }
        } else {
            // No @ symbol, just a package name
            Self {
                name: package.to_string(),
                version: None,
                is_dev,
            }
        }
    }

    pub fn is_git(&self) -> bool {
        self.name.starts_with("git:")
            || self.name.starts_with("git+ssh:")
            || self.name.starts_with("git+http:")
            || self.name.starts_with("git+https:")
            || self.name.starts_with("ssh:")
            || self.name.ends_with(".git")
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
    fn test_is_git() {
        let pkg = InstallPackage::new("git:gitlab.com/package/psc".to_string(), None, false);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new(
            "git+ssh:git@github.com:user/repo.git".to_string(),
            None,
            false,
        );
        assert!(pkg.is_git());

        let pkg = InstallPackage::new(
            "git+http://github.com/user/repo.git".to_string(),
            None,
            false,
        );
        assert!(pkg.is_git());

        let pkg = InstallPackage::new(
            "git+https://github.com/user/repo.git".to_string(),
            None,
            false,
        );
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("ssh:git@github.com:user/repo.git".to_string(), None, false);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("https://github.com/user/repo.git".to_string(), None, false);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("react".to_string(), None, false);
        assert!(!pkg.is_git());

        let pkg = InstallPackage::new("express".to_string(), None, false);
        assert!(!pkg.is_git());

        let pkg = InstallPackage::new("@scope/package".to_string(), None, false);
        assert!(!pkg.is_git());
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
    fn test_from_literal_dev() {
        let pkg = InstallPackage::from_literal("typescript@5.0.0", true);
        assert_eq!(pkg.name, "typescript");
        assert_eq!(pkg.version, Some("5.0.0".to_string()));
        assert!(pkg.is_dev);
    }
}
