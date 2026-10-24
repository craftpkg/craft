use std::io;
use std::path::Path;

/// Names what was being done to which path in an IO error, keeping its kind so callers can
/// still tell e.g. a permission problem apart.
pub(crate) trait PathContext<T> {
    fn at(self, action: &str, path: &Path) -> io::Result<T>;
}

impl<T> PathContext<T> for io::Result<T> {
    fn at(self, action: &str, path: &Path) -> io::Result<T> {
        self.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not {action} {}: {error}", path.display()),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keeps_the_kind_and_names_the_path() {
        let failed: io::Result<()> = Err(io::Error::from(io::ErrorKind::PermissionDenied));
        let error = failed
            .at("create", Path::new("/app/node_modules"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .starts_with("could not create /app/node_modules: ")
        );
    }
}
