use std::fmt::Display;
use std::path::PathBuf;

use contract::Context;
use network::Integrity;
use package::PackageJson;
use sha2::{Digest, Sha256};

use crate::NodeId;

/// Hex characters of the source hash used in cache directory names.
const SOURCE_HASH_LENGTH: usize = 16;

#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub name: String,
    pub version: String,
    pub source: ArtifactSource,
    pub integrity: Option<Integrity>,
    pub package: Option<PackageJson>,
}

/// Where a package's files come from.
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactSource {
    Registry {
        tarball_url: String,
    },
    /// A tarball URL dependency, extracted into the cache by its content hash.
    Tarball {
        url: String,
    },
    /// A `file:` tarball, extracted into the cache by its content hash.
    LocalTarball {
        path: PathBuf,
    },
    /// A `file:` directory, copied into the cache on every install.
    Directory {
        path: PathBuf,
    },
    /// A `link:` directory, symlinked in place with none of its dependencies installed.
    Link {
        path: PathBuf,
    },
}

impl ResolvedArtifact {
    pub fn to_cache_key(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }

    /// Registry versions are identified by version; every other source by where it came from,
    /// since two local copies or URLs can share a version and still differ.
    pub fn node_id(&self) -> NodeId {
        match &self.source {
            ArtifactSource::Registry { .. } => NodeId::new(&self.name, &self.version),
            ArtifactSource::Tarball { url } => NodeId::new(&self.name, url),
            ArtifactSource::LocalTarball { path } | ArtifactSource::Directory { path } => {
                NodeId::new(&self.name, &format!("file:{}", path.display()))
            }
            ArtifactSource::Link { path } => {
                NodeId::new(&self.name, &format!("link:{}", path.display()))
            }
        }
    }

    /// The directory holding the package's files, which dependents link to.
    pub fn package_dir(&self, context: &Context) -> PathBuf {
        match &self.source {
            ArtifactSource::Registry { .. } => context.package_dir(&self.name, &self.version),
            ArtifactSource::Link { path } => path.clone(),
            ArtifactSource::Tarball { .. } | ArtifactSource::LocalTarball { .. } => {
                let key = self
                    .integrity
                    .as_ref()
                    .map_or_else(|| self.node_id().to_string(), Integrity::to_string);
                source_cache_dir(context, "tarball", &key)
            }
            ArtifactSource::Directory { path } => {
                source_cache_dir(context, "file", &path.display().to_string())
            }
        }
    }
}

impl Display for ResolvedArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

pub(crate) fn source_hash(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex[..SOURCE_HASH_LENGTH].to_string()
}

/// Named by source rather than package, since a tarball's name is only known once extracted.
pub(crate) fn source_cache_dir(context: &Context, kind: &str, key: &str) -> PathBuf {
    context
        .package_cache_dir()
        .join(format!("{kind}-{}", source_hash(key)))
        .join("package")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(source: ArtifactSource) -> ResolvedArtifact {
        ResolvedArtifact {
            name: "local-lib".to_string(),
            version: "0.1.0".to_string(),
            source,
            integrity: None,
            package: None,
        }
    }

    #[test]
    fn test_node_ids_name_the_source() {
        let registry = artifact(ArtifactSource::Registry {
            tarball_url: String::new(),
        });
        let directory = artifact(ArtifactSource::Directory {
            path: "/app/libs/local-lib".into(),
        });
        let link = artifact(ArtifactSource::Link {
            path: "/app/libs/local-lib".into(),
        });

        assert_eq!(registry.node_id().to_string(), "local-lib@0.1.0");
        assert_eq!(
            directory.node_id().to_string(),
            "local-lib@file:/app/libs/local-lib"
        );
        assert_eq!(
            link.node_id().to_string(),
            "local-lib@link:/app/libs/local-lib"
        );
    }

    #[test]
    fn test_package_dirs() {
        let context = Context::new("/app".into(), "/home/me/.craft".into(), false);
        let link = artifact(ArtifactSource::Link {
            path: "/app/libs/local-lib".into(),
        });
        let directory = artifact(ArtifactSource::Directory {
            path: "/app/libs/local-lib".into(),
        });
        let other_directory = artifact(ArtifactSource::Directory {
            path: "/other/libs/local-lib".into(),
        });

        assert_eq!(
            link.package_dir(&context),
            PathBuf::from("/app/libs/local-lib")
        );
        let directory_dir = directory.package_dir(&context);
        assert!(directory_dir.starts_with("/home/me/.craft/packages"));
        assert!(directory_dir.ends_with("package"));
        assert_ne!(directory_dir, other_directory.package_dir(&context));
    }
}
