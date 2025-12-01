/// Represents a parsed dependency specification
#[derive(Debug, Clone, PartialEq)]
pub struct DependencySpec {
    /// The actual package name to resolve (may differ from the alias name)
    pub package_name: String,
    /// The version requirement (None means latest)
    pub version: Option<String>,
}

impl DependencySpec {
    /// Parse a dependency specification from a package name and version string
    ///
    /// Handles various formats:
    /// - Regular: ("package", Some("^1.0.0")) -> DependencySpec { package_name: "package", version: Some("^1.0.0") }
    /// - NPM alias: ("alias", Some("npm:actual-package@^1.0.0")) -> DependencySpec { package_name: "actual-package", version: Some("^1.0.0") }
    /// - No version: ("package", None) -> DependencySpec { package_name: "package", version: None }
    pub fn parse(package_name: &str, version: Option<&str>) -> Self {
        if let Some(version_str) = version {
            if let Some(spec) = Self::parse_npm_alias(version_str) {
                return spec;
            }
            // Regular version specification
            Self {
                package_name: package_name.to_string(),
                version: Some(version_str.to_string()),
            }
        } else {
            // No version specified, use latest
            Self {
                package_name: package_name.to_string(),
                version: None,
            }
        }
    }

    /// Parse npm alias format: npm:package-name@version
    fn parse_npm_alias(version_str: &str) -> Option<Self> {
        if !version_str.starts_with("npm:") {
            return None;
        }

        let without_prefix = &version_str[4..]; // Remove "npm:"

        if let Some(at_pos) = without_prefix.find('@') {
            let pkg_name = &without_prefix[..at_pos];
            let ver = &without_prefix[at_pos + 1..];
            Some(Self {
                package_name: pkg_name.to_string(),
                version: Some(ver.to_string()),
            })
        } else {
            // npm:package-name without version
            Some(Self {
                package_name: without_prefix.to_string(),
                version: None,
            })
        }
    }
}

impl From<&package::InstallPackage> for DependencySpec {
    fn from(package: &package::InstallPackage) -> Self {
        Self::parse(&package.name, package.version.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use package::InstallPackage;

    #[test]
    fn test_parse_regular_version() {
        let spec = DependencySpec::parse("react", Some("^18.0.0"));
        assert_eq!(spec.package_name, "react");
        assert_eq!(spec.version, Some("^18.0.0".to_string()));
    }

    #[test]
    fn test_parse_no_version() {
        let spec = DependencySpec::parse("react", None);
        assert_eq!(spec.package_name, "react");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_npm_alias_with_version() {
        let spec = DependencySpec::parse("wrap-ansi-cjs", Some("npm:wrap-ansi@^7.0.0"));
        assert_eq!(spec.package_name, "wrap-ansi");
        assert_eq!(spec.version, Some("^7.0.0".to_string()));
    }

    #[test]
    fn test_parse_npm_alias_without_version() {
        let spec = DependencySpec::parse("my-alias", Some("npm:actual-package"));
        assert_eq!(spec.package_name, "actual-package");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_complex_npm_alias() {
        let spec = DependencySpec::parse("string-width-cjs", Some("npm:string-width@^4.2.0"));
        assert_eq!(spec.package_name, "string-width");
        assert_eq!(spec.version, Some("^4.2.0".to_string()));
    }

    #[test]
    fn test_from_install_package() {
        let package = InstallPackage::new("react".to_string(), Some("^18.0.0".to_string()), false);
        let spec: DependencySpec = (&package).into();
        assert_eq!(spec.package_name, "react");
        assert_eq!(spec.version, Some("^18.0.0".to_string()));
    }

    #[test]
    fn test_from_install_package_with_alias() {
        let package = InstallPackage::new(
            "wrap-ansi-cjs".to_string(),
            Some("npm:wrap-ansi@^7.0.0".to_string()),
            false,
        );
        let spec: DependencySpec = (&package).into();
        assert_eq!(spec.package_name, "wrap-ansi");
        assert_eq!(spec.version, Some("^7.0.0".to_string()));
    }
}
