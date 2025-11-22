pub mod npm;
pub use npm::{NpmPackage, PackageJson};

pub struct Package {
    pub name: String,
    pub version: Option<String>,
}

impl Package {
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
    use crate::Package;

    #[test]
    fn test_new_package() {
        let pkg = Package::new("react".to_string(), Some("1.0.0".to_string()));
        assert_eq!(pkg.name, "react");
        assert_eq!(pkg.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_is_git() {
        let pkg = Package::new("git:gitlab.com/package/psc".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("git+ssh:git@github.com:user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("git+http://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("git+https://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("ssh:git@github.com:user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("https://github.com/user/repo.git".to_string(), None);
        assert!(pkg.is_git());

        let pkg = Package::new("react".to_string(), None);
        assert!(!pkg.is_git());

        let pkg = Package::new("express".to_string(), None);
        assert!(!pkg.is_git());

        let pkg = Package::new("@scope/package".to_string(), None);
        assert!(!pkg.is_git());
    }
}
