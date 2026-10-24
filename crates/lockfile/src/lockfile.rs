use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Context as _;
use contract::{LockfileError, Result};
use serde::{Deserialize, Serialize};

pub const LOCKFILE_NAME: &str = "craft.lock";
pub const LOCKFILE_VERSION: u32 = 1;
/// The importer key of the project itself; workspaces (Phase 7) add one per package.
pub const ROOT_IMPORTER: &str = ".";

/// `craft.lock`: what every dependency resolved to, so installs are reproducible. Maps are
/// sorted and the file is pretty-printed so changes review well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockfile {
    pub lockfile_version: u32,
    pub importers: BTreeMap<String, Importer>,
    pub packages: BTreeMap<String, LockedPackage>,
}

/// A project's own dependencies, keyed by the name it requires them by.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Importer {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, LockedDependency>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dev_dependencies: BTreeMap<String, LockedDependency>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockedDependency {
    /// The spec as written in package.json; a changed specifier invalidates the entry.
    pub specifier: String,
    /// The key of the package it resolved to in [`Lockfile::packages`].
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub resolution: LockedSource,
    /// Dependency name → package key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
    /// Peer name → package key, as resolved for this instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peer_dependencies: BTreeMap<String, String>,
    /// Optional dependency name → package key, including packages for other platforms, so the
    /// lockfile installs anywhere.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub os: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cpu: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libc: Vec<String>,
}

/// Where a package came from. Paths are relative to the project directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LockedSource {
    Registry {
        tarball: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        integrity: Option<String>,
    },
    Tarball {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        integrity: Option<String>,
    },
    LocalTarball {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        integrity: Option<String>,
    },
    Directory {
        path: String,
    },
    Link {
        path: String,
    },
    Git {
        url: String,
        commit: String,
    },
}

impl LockedPackage {
    pub fn new(name: &str, version: &str, resolution: LockedSource) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            resolution,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            os: Vec::new(),
            cpu: Vec::new(),
            libc: Vec::new(),
        }
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            lockfile_version: LOCKFILE_VERSION,
            importers: BTreeMap::new(),
            packages: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    /// Reads `craft.lock` from `project_dir`; `None` when the project has none yet.
    pub async fn load(project_dir: &Path) -> Result<Option<Self>> {
        let path = project_dir.join(LOCKFILE_NAME);
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        Self::parse(&content)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(Some)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let lockfile: Self = serde_json::from_str(content)?;
        if lockfile.lockfile_version > LOCKFILE_VERSION {
            return Err(LockfileError::UnsupportedVersion {
                found: lockfile.lockfile_version,
                supported: LOCKFILE_VERSION,
            }
            .into());
        }
        Ok(lockfile)
    }

    pub fn render(&self) -> Result<String> {
        let mut rendered = serde_json::to_string_pretty(self)?;
        rendered.push('\n');
        Ok(rendered)
    }

    /// Writes `craft.lock` into `project_dir` when its contents changed; returns whether it did.
    pub async fn save(&self, project_dir: &Path) -> Result<bool> {
        let path = project_dir.join(LOCKFILE_NAME);
        let written = fsutil::write_if_changed(&path, self.render()?.as_bytes())
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(written)
    }

    pub fn root(&self) -> Option<&Importer> {
        self.importers.get(ROOT_IMPORTER)
    }

    /// Drops `name` from the project's dependencies, along with every package nothing else
    /// still reaches. Returns whether the project listed it.
    pub fn remove_dependency(&mut self, name: &str) -> bool {
        let Some(root) = self.importers.get_mut(ROOT_IMPORTER) else {
            return false;
        };
        let was_dependency = root.dependencies.remove(name).is_some();
        let was_dev_dependency = root.dev_dependencies.remove(name).is_some();
        self.prune_unreachable();
        was_dependency || was_dev_dependency
    }

    fn prune_unreachable(&mut self) {
        let mut reachable = BTreeSet::new();
        let mut pending: Vec<&String> = self
            .importers
            .values()
            .flat_map(|importer| {
                importer
                    .dependencies
                    .values()
                    .chain(importer.dev_dependencies.values())
            })
            .map(|dependency| &dependency.version)
            .collect();
        while let Some(key) = pending.pop() {
            if !reachable.insert(key.clone()) {
                continue;
            }
            if let Some(package) = self.packages.get(key) {
                pending.extend(package.dependencies.values());
                pending.extend(package.peer_dependencies.values());
                pending.extend(package.optional_dependencies.values());
            }
        }
        self.packages.retain(|key, _| reachable.contains(key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Lockfile {
        let mut lockfile = Lockfile::default();
        lockfile.importers.insert(
            ROOT_IMPORTER.to_string(),
            Importer {
                dependencies: BTreeMap::from([(
                    "ui".to_string(),
                    LockedDependency {
                        specifier: "^1.0.0".to_string(),
                        version: "ui@1.0.0_react@19.2.0".to_string(),
                    },
                )]),
                dev_dependencies: BTreeMap::new(),
            },
        );
        lockfile.packages.insert(
            "ui@1.0.0_react@19.2.0".to_string(),
            LockedPackage {
                name: "ui".to_string(),
                version: "1.0.0".to_string(),
                resolution: LockedSource::Registry {
                    tarball: "https://registry.npmjs.org/ui/-/ui-1.0.0.tgz".to_string(),
                    integrity: Some("sha512-abc".to_string()),
                },
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::from([(
                    "react".to_string(),
                    "react@19.2.0".to_string(),
                )]),
                ..LockedPackage::new(
                    "ui",
                    "1.0.0",
                    LockedSource::Directory {
                        path: String::new(),
                    },
                )
            },
        );
        lockfile
    }

    #[test]
    fn test_renders_reviewable_json() {
        let rendered = sample().render().unwrap();
        assert_eq!(
            rendered,
            r#"{
  "lockfileVersion": 1,
  "importers": {
    ".": {
      "dependencies": {
        "ui": {
          "specifier": "^1.0.0",
          "version": "ui@1.0.0_react@19.2.0"
        }
      }
    }
  },
  "packages": {
    "ui@1.0.0_react@19.2.0": {
      "name": "ui",
      "version": "1.0.0",
      "resolution": {
        "type": "registry",
        "tarball": "https://registry.npmjs.org/ui/-/ui-1.0.0.tgz",
        "integrity": "sha512-abc"
      },
      "peerDependencies": {
        "react": "react@19.2.0"
      }
    }
  }
}
"#
        );
    }

    #[test]
    fn test_round_trips() {
        let lockfile = sample();
        assert_eq!(
            Lockfile::parse(&lockfile.render().unwrap()).unwrap(),
            lockfile
        );
    }

    #[test]
    fn test_rejects_newer_versions() {
        let error = Lockfile::parse(r#"{ "lockfileVersion": 9, "importers": {}, "packages": {} }"#)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "craft.lock has lockfileVersion 9, but this craft reads up to 1; upgrade craft"
        );
    }

    #[test]
    fn test_removing_a_dependency_prunes_what_only_it_needed() {
        let mut lockfile = sample();
        let react = LockedPackage {
            name: "react".to_string(),
            version: "19.2.0".to_string(),
            resolution: LockedSource::Registry {
                tarball: String::new(),
                integrity: None,
            },
            ..LockedPackage::new(
                "react",
                "19.2.0",
                LockedSource::Directory {
                    path: String::new(),
                },
            )
        };
        lockfile.packages.insert("react@19.2.0".to_string(), react);

        assert!(!lockfile.remove_dependency("missing"));
        assert_eq!(lockfile.packages.len(), 2);
        assert!(lockfile.remove_dependency("ui"));
        assert!(lockfile.packages.is_empty());
    }

    #[tokio::test]
    async fn test_load_and_save() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Lockfile::load(dir.path()).await.unwrap(), None);

        assert!(sample().save(dir.path()).await.unwrap());
        assert!(!sample().save(dir.path()).await.unwrap());
        assert_eq!(Lockfile::load(dir.path()).await.unwrap(), Some(sample()));
    }
}
