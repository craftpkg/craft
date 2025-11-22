pub use crate::{NpmPackage, PackageJson};

pub struct InstallPackage {
    pub name: String,
    pub version: Option<String>,
}

impl InstallPackage {
    pub fn new(name: String, version: Option<String>) -> Self {
        Self { name, version }
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
        let pkg = InstallPackage::new("react".to_string(), Some("1.0.0".to_string()));
        assert_eq!(pkg.name, "react");
        assert_eq!(pkg.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_is_git() {
        let pkg = InstallPackage::new("git:gitlab.com/package/psc".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("git+ssh:git@github.com:user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("git+http://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("git+https://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("ssh:git@github.com:user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("https://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = InstallPackage::new("react".to_string(), None);
        assert!(!pkg.is_git());

        let pkg = InstallPackage::new("express".to_string(), None);
        assert!(!pkg.is_git());

        let pkg = InstallPackage::new("@scope/package".to_string(), None);
        assert!(!pkg.is_git());
    }
}
