use std::collections::HashMap;

use lockfile::{LockedSource, Lockfile};
use network::Integrity;
use node_semver::Version;
use package::VersionRequirement;

use crate::git::GitSpec;
use crate::{ArtifactSource, ResolvedArtifact};

/// What `craft.lock` already decided, consulted before the registry or a git remote so an
/// unchanged project resolves without metadata requests.
#[derive(Debug, Default)]
pub struct LockPreferences {
    registry: HashMap<String, Vec<LockedVersion>>,
    git_commits: HashMap<String, String>,
}

#[derive(Debug)]
struct LockedVersion {
    version: Version,
    tarball: String,
    integrity: Option<String>,
}

impl LockPreferences {
    pub fn from_lockfile(lockfile: &Lockfile) -> Self {
        let mut preferences = Self::default();
        for package in lockfile.packages.values() {
            match &package.resolution {
                LockedSource::Registry { tarball, integrity } => {
                    let Ok(version) = Version::parse(&package.version) else {
                        continue;
                    };
                    let versions = preferences
                        .registry
                        .entry(package.name.clone())
                        .or_default();
                    let is_listed = versions.iter().any(|locked| locked.version == version);
                    if !is_listed {
                        versions.push(LockedVersion {
                            version,
                            tarball: tarball.clone(),
                            integrity: integrity.clone(),
                        });
                    }
                }
                LockedSource::Git { url, commit } => {
                    preferences.git_commits.insert(url.clone(), commit.clone());
                }
                _ => {}
            }
        }
        preferences
    }

    /// The highest locked version of `name` that satisfies `requirement`; for a dist-tag, the
    /// highest locked version at all, since the lockfile pins what the tag meant when locked.
    pub fn registry(
        &self,
        name: &str,
        requirement: &VersionRequirement,
    ) -> Option<ResolvedArtifact> {
        let locked = self
            .registry
            .get(name)?
            .iter()
            .filter(|locked| match requirement {
                VersionRequirement::Exact(version) => locked.version == *version,
                VersionRequirement::Range(range) => range.satisfies(&locked.version),
                VersionRequirement::Tag(_) => true,
            })
            .max_by(|a, b| a.version.cmp(&b.version))?;

        Some(ResolvedArtifact {
            name: name.to_string(),
            version: locked.version.to_string(),
            source: ArtifactSource::Registry {
                tarball_url: locked.tarball.clone(),
            },
            integrity: locked
                .integrity
                .as_deref()
                .and_then(|integrity| Integrity::parse(integrity).ok()),
            package: None,
        })
    }

    pub fn git_commit(&self, url: &str) -> Option<&str> {
        self.git_commits.get(url).map(String::as_str)
    }

    /// The locked commit for a git dependency spec, matched by the URL it clones.
    pub fn git_commit_for_spec(&self, spec: &str) -> Option<&str> {
        let git_spec = GitSpec::parse(spec)?;
        self.git_commit(&git_spec.clone_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lockfile::LockedPackage;
    use std::collections::BTreeMap;

    fn registry_package(name: &str, version: &str) -> LockedPackage {
        LockedPackage {
            name: name.to_string(),
            version: version.to_string(),
            resolution: LockedSource::Registry {
                tarball: format!("https://registry.test/{name}-{version}.tgz"),
                integrity: None,
            },
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
        }
    }

    fn preferences() -> LockPreferences {
        let mut lockfile = Lockfile::default();
        for (key, package) in [
            ("react@18.3.1", registry_package("react", "18.3.1")),
            ("react@19.2.0", registry_package("react", "19.2.0")),
            ("react@19.2.0_x@1.0.0", registry_package("react", "19.2.0")),
        ] {
            lockfile.packages.insert(key.to_string(), package);
        }
        lockfile.packages.insert(
            "lib@git".to_string(),
            LockedPackage {
                resolution: LockedSource::Git {
                    url: "https://github.com/o/lib.git".to_string(),
                    commit: "abc123".to_string(),
                },
                ..registry_package("lib", "1.0.0")
            },
        );
        LockPreferences::from_lockfile(&lockfile)
    }

    fn requirement(spec: &str) -> VersionRequirement {
        package::DependencySpec::parse("react", Some(spec))
            .unwrap()
            .registry_target()
            .unwrap()
            .1
            .clone()
    }

    fn locked_version(spec: &str) -> Option<String> {
        preferences()
            .registry("react", &requirement(spec))
            .map(|artifact| artifact.version)
    }

    #[test]
    fn test_prefers_the_highest_locked_version_in_range() {
        assert_eq!(locked_version("^18.0.0"), Some("18.3.1".to_string()));
        assert_eq!(locked_version(">=18"), Some("19.2.0".to_string()));
        assert_eq!(locked_version("19.2.0"), Some("19.2.0".to_string()));
        assert_eq!(locked_version("latest"), Some("19.2.0".to_string()));
        assert_eq!(locked_version("^20.0.0"), None);
        assert!(preferences().registry("vue", &requirement("*")).is_none());
    }

    #[test]
    fn test_locked_artifacts_point_at_the_locked_tarball() {
        let artifact = preferences()
            .registry("react", &requirement("^18"))
            .unwrap();
        assert_eq!(
            artifact.source,
            ArtifactSource::Registry {
                tarball_url: "https://registry.test/react-18.3.1.tgz".to_string()
            }
        );
        assert!(artifact.package.is_none());
    }

    #[test]
    fn test_remembers_git_commits_by_url() {
        assert_eq!(
            preferences().git_commit("https://github.com/o/lib.git"),
            Some("abc123")
        );
        assert_eq!(
            preferences().git_commit("https://github.com/o/other.git"),
            None
        );
    }
}
