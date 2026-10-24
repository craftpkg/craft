use anyhow::Context;
use contract::{PackageError, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<PackageDist>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<PackageBin>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum PackageBin {
    String(String),
    Map(HashMap<String, String>),
}

/// A `peerDependencies` entry, optional when `peerDependenciesMeta` marks it so.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerSpec {
    pub name: String,
    pub range: String,
    pub optional: bool,
}

impl PackageJson {
    /// The project's `trustedDependencies`: the dependencies allowed to run install scripts.
    pub fn trusted_dependencies(&self) -> Vec<String> {
        let mut trusted: Vec<String> = self
            .other
            .get("trustedDependencies")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|name| name.as_str().map(str::to_string))
            .collect();
        trusted.sort();
        trusted.dedup();
        trusted
    }

    /// The install-time scripts npm runs for a dependency, in order. A package with a
    /// `binding.gyp` and no `preinstall`/`install` script implicitly runs `node-gyp rebuild`.
    pub fn install_scripts(&self, package_dir: &Path) -> Vec<(&'static str, String)> {
        let script = |event: &str| {
            self.scripts
                .as_ref()
                .and_then(|scripts| scripts.get(event))
                .cloned()
        };
        let mut install = script("install");
        let has_explicit_build = script("preinstall").is_some() || install.is_some();
        if !has_explicit_build && package_dir.join("binding.gyp").is_file() {
            install = Some("node-gyp rebuild".to_string());
        }

        [
            ("preinstall", script("preinstall")),
            ("install", install),
            ("postinstall", script("postinstall")),
        ]
        .into_iter()
        .filter_map(|(event, script)| Some((event, script?)))
        .collect()
    }

    /// `optionalDependencies`, read loosely like peers. npm installs these in place of a
    /// `dependencies` entry of the same name.
    pub fn optional_dependencies(&self) -> Vec<(String, String)> {
        let Some(optional) = self
            .other
            .get("optionalDependencies")
            .and_then(serde_json::Value::as_object)
        else {
            return Vec::new();
        };
        let mut dependencies: Vec<(String, String)> = optional
            .iter()
            .filter_map(|(name, spec)| Some((name.clone(), spec.as_str()?.to_string())))
            .collect();
        dependencies.sort();
        dependencies
    }

    /// Declared peers, sorted by name. Read loosely from the raw manifest, since registries
    /// serve these fields as-is.
    pub fn peer_dependencies(&self) -> Vec<PeerSpec> {
        let Some(peers) = self
            .other
            .get("peerDependencies")
            .and_then(serde_json::Value::as_object)
        else {
            return Vec::new();
        };
        let meta = self.other.get("peerDependenciesMeta");

        let mut specs: Vec<PeerSpec> = peers
            .iter()
            .filter_map(|(name, range)| {
                let optional = meta
                    .and_then(|meta| meta.get(name))
                    .and_then(|entry| entry.get("optional"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                Some(PeerSpec {
                    name: name.clone(),
                    range: range.as_str()?.to_string(),
                    optional,
                })
            })
            .collect();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    pub async fn from_file(package_json_path: &Path) -> Result<Self> {
        if !package_json_path.exists() {
            return Err(PackageError::NoPackageJson.into());
        }

        let content = fs::read_to_string(package_json_path).await?;
        let package_json: PackageJson = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", package_json_path.display()))?;
        Ok(package_json)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageDist {
    pub tarball: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shasum: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NpmPackage {
    pub name: String,
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, PackageJson>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reads_peer_dependencies_and_optional_meta() {
        let manifest: PackageJson = serde_json::from_str(
            r#"{
                "peerDependencies": { "react": "^18 || ^19", "react-native": "*", "bad": 1 },
                "peerDependenciesMeta": { "react-native": { "optional": true } }
            }"#,
        )
        .unwrap();

        assert_eq!(
            manifest.peer_dependencies(),
            [
                PeerSpec {
                    name: "react".to_string(),
                    range: "^18 || ^19".to_string(),
                    optional: false,
                },
                PeerSpec {
                    name: "react-native".to_string(),
                    range: "*".to_string(),
                    optional: true,
                },
            ]
        );
    }

    #[test]
    fn test_reads_optional_dependencies() {
        let manifest: PackageJson = serde_json::from_str(
            r#"{ "optionalDependencies": { "fsevents": "~2.3.2", "@esbuild/darwin-arm64": "0.25.0", "bad": 1 } }"#,
        )
        .unwrap();

        assert_eq!(
            manifest.optional_dependencies(),
            [
                ("@esbuild/darwin-arm64".to_string(), "0.25.0".to_string()),
                ("fsevents".to_string(), "~2.3.2".to_string()),
            ]
        );
    }

    #[test]
    fn test_reads_trusted_dependencies() {
        let manifest: PackageJson = serde_json::from_str(
            r#"{ "trustedDependencies": ["esbuild", "bcrypt", "esbuild", 1] }"#,
        )
        .unwrap();
        assert_eq!(manifest.trusted_dependencies(), ["bcrypt", "esbuild"]);
    }

    #[test]
    fn test_install_scripts_in_order_with_implicit_node_gyp() {
        let dir = tempfile::tempdir().unwrap();
        let manifest: PackageJson = serde_json::from_str(
            r#"{ "scripts": { "postinstall": "node setup.js", "test": "jest" } }"#,
        )
        .unwrap();
        assert_eq!(
            manifest.install_scripts(dir.path()),
            [("postinstall", "node setup.js".to_string())]
        );

        std::fs::write(dir.path().join("binding.gyp"), "{}").unwrap();
        assert_eq!(
            manifest.install_scripts(dir.path()),
            [
                ("install", "node-gyp rebuild".to_string()),
                ("postinstall", "node setup.js".to_string()),
            ]
        );

        let explicit: PackageJson =
            serde_json::from_str(r#"{ "scripts": { "install": "prebuild-install" } }"#).unwrap();
        assert_eq!(
            explicit.install_scripts(dir.path()),
            [("install", "prebuild-install".to_string())]
        );
    }

    #[test]
    fn test_tolerates_loosely_typed_metadata_fields() {
        let manifest: PackageJson = serde_json::from_str(
            r#"{ "name": "lodash", "keywords": "modules, stdlib, util", "description": ["odd"] }"#,
        )
        .unwrap();
        assert_eq!(manifest.name.as_deref(), Some("lodash"));
    }
}
