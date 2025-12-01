use contract::{PackageError, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
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

impl PackageJson {
    pub fn file_path() -> Result<PathBuf> {
        let package_json_path = std::env::current_dir()?.join("package.json");
        Ok(package_json_path)
    }
    pub async fn from_file() -> Result<Self> {
        let package_json_path = Self::file_path()?;

        if !package_json_path.exists() {
            return Err(PackageError::NoPackageJson.into());
        }

        let content = fs::read_to_string(&package_json_path).await?;
        let package_json: PackageJson = serde_json::from_str(&content)?;
        Ok(package_json)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageDist {
    pub tarball: String,
}

#[derive(Debug, Deserialize)]
pub struct NpmPackage {
    pub name: String,
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, PackageJson>,
}
