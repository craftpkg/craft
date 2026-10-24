use std::path::{Path, PathBuf};

use resolver::{ArtifactSource, NodeId, ResolvedArtifact, short_hash};

pub const VIRTUAL_STORE_DIR: &str = ".craft";
/// The hidden hoisting directory inside the virtual store; never a package entry itself.
pub(crate) const HOIST_DIR: &str = "node_modules";
/// The lockfile `node_modules` was last laid out from, kept inside the virtual store.
pub const INSTALLED_LOCKFILE: &str = "lock.json";
/// Longer entry names are truncated and suffixed with a hash, as pnpm does, to stay well under
/// filesystem name limits.
const MAX_ENTRY_NAME_LENGTH: usize = 120;

/// The project's `node_modules/.craft`: one directory per resolved package, holding the package
/// itself plus links to its dependencies, all inside the project.
pub(crate) struct VirtualStore {
    root: PathBuf,
}

impl VirtualStore {
    pub fn new(node_modules: &Path) -> Self {
        Self {
            root: node_modules.join(VIRTUAL_STORE_DIR),
        }
    }

    /// `node_modules/.craft/node_modules`, where every package name is hoisted once. Node's
    /// lookup from inside the virtual store reaches it, so undeclared requires still resolve.
    pub fn hoist_dir(&self) -> PathBuf {
        self.root.join(HOIST_DIR)
    }

    /// `node_modules/.craft/<entry>/node_modules`: the package and the links to its
    /// dependencies, side by side.
    pub fn modules_dir(&self, node_id: &NodeId, artifact: &ResolvedArtifact) -> PathBuf {
        self.root
            .join(entry_name(node_id, artifact))
            .join("node_modules")
    }

    /// Where dependents link to: the package inside its entry, or a `link:` target in place.
    pub fn package_dir(&self, node_id: &NodeId, artifact: &ResolvedArtifact) -> PathBuf {
        match &artifact.source {
            ArtifactSource::Link { path } => path.clone(),
            _ => self.modules_dir(node_id, artifact).join(&artifact.name),
        }
    }
}

/// The instance id for registry packages — `<name>@<version>`, plus `_<peer>@<version>` for
/// each resolved peer — with `/` escaped (`@scope/x` → `@scope+x`); other sources are told apart
/// by a hash of their id.
fn entry_name(node_id: &NodeId, artifact: &ResolvedArtifact) -> String {
    let name = artifact.name.replace('/', "+");
    let source_hash = || short_hash(&node_id.to_string());
    let entry = match &artifact.source {
        ArtifactSource::Registry { .. } => node_id.to_string().replace('/', "+"),
        ArtifactSource::Tarball { .. } => format!("{name}@tarball+{}", source_hash()),
        ArtifactSource::LocalTarball { .. } | ArtifactSource::Directory { .. } => {
            format!("{name}@file+{}", source_hash())
        }
        ArtifactSource::Git { .. } => format!("{name}@git+{}", source_hash()),
        ArtifactSource::Link { .. } => format!("{name}@link+{}", source_hash()),
    };
    truncate(entry)
}

fn truncate(entry: String) -> String {
    if entry.len() <= MAX_ENTRY_NAME_LENGTH {
        return entry;
    }
    let hash = short_hash(&entry);
    let kept: String = entry
        .chars()
        .take(MAX_ENTRY_NAME_LENGTH - hash.len() - 1)
        .collect();
    format!("{kept}_{hash}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str, source: ArtifactSource) -> ResolvedArtifact {
        ResolvedArtifact {
            name: name.to_string(),
            version: "5.90.10".to_string(),
            source,
            integrity: None,
            platform: Default::default(),
            package: None,
        }
    }

    fn registry() -> ArtifactSource {
        ArtifactSource::Registry {
            tarball_url: String::new(),
        }
    }

    #[test]
    fn test_registry_entries_are_readable() {
        let store = VirtualStore::new(Path::new("/app/node_modules"));
        let query = artifact("@tanstack/query-core", registry());
        let id = NodeId::new("@tanstack/query-core", "5.90.10");

        assert_eq!(
            store.package_dir(&id, &query),
            PathBuf::from(
                "/app/node_modules/.craft/@tanstack+query-core@5.90.10/node_modules/@tanstack/query-core"
            )
        );
    }

    #[test]
    fn test_peer_variants_get_their_own_entry() {
        let store = VirtualStore::new(Path::new("/app/node_modules"));
        let query = artifact("@tanstack/react-query", registry());
        let id = NodeId::new("@tanstack/react-query", "5.90.10")
            .with_peers([&NodeId::new("react", "19.2.0")]);

        assert_eq!(
            store.modules_dir(&id, &query),
            PathBuf::from(
                "/app/node_modules/.craft/@tanstack+react-query@5.90.10_react@19.2.0/node_modules"
            )
        );
    }

    #[test]
    fn test_links_point_at_their_target() {
        let store = VirtualStore::new(Path::new("/app/node_modules"));
        let link = artifact(
            "local",
            ArtifactSource::Link {
                path: "/app/libs/local".into(),
            },
        );
        let id = NodeId::new("local", "link:/app/libs/local");

        assert_eq!(
            store.package_dir(&id, &link),
            PathBuf::from("/app/libs/local")
        );
    }

    #[test]
    fn test_local_sources_are_hashed_and_long_names_truncated() {
        let directory = artifact(
            "local-lib",
            ArtifactSource::Directory {
                path: "/app/libs/local-lib".into(),
            },
        );
        let id = NodeId::new("local-lib", "file:/app/libs/local-lib");
        let entry = entry_name(&id, &directory);
        assert!(entry.starts_with("local-lib@file+"), "{entry}");

        let long_name = "x".repeat(200);
        let long = artifact(&long_name, registry());
        let entry = entry_name(&NodeId::new(&long_name, "1.0.0"), &long);
        assert_eq!(entry.len(), MAX_ENTRY_NAME_LENGTH);
    }
}
