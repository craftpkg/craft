use std::collections::HashMap;
use std::path::Path;

use contract::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::package_entry::PackageEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: String,
    pub packages: HashMap<String, PackageEntry>,
}

impl Lockfile {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            packages: HashMap::new(),
        }
    }

    pub fn add_package(&mut self, entry: PackageEntry) {
        let key = entry.key();
        self.packages.insert(key, entry);
    }

    pub fn get_package(&self, name: &str, version: &str) -> Option<&PackageEntry> {
        let key = format!("{}@{}", name, version);
        self.packages.get(&key)
    }

    /// Save the lockfile to disk as JSON (human-readable)
    pub async fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json).await?;
        Ok(())
    }

    /// Load a lockfile from disk (JSON format)
    pub async fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path).await?;
        let lockfile: Lockfile = serde_json::from_str(&contents)?;
        Ok(lockfile)
    }

    /// Save the lockfile to disk as binary (compact, efficient)
    pub async fn save_binary(&self, path: &Path) -> Result<()> {
        let bytes = bincode::serialize(self)?;
        fs::write(path, bytes).await?;
        Ok(())
    }

    /// Load a lockfile from disk (binary format)
    pub async fn load_binary(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).await?;
        let lockfile: Lockfile = bincode::deserialize(&bytes)?;
        Ok(lockfile)
    }

    pub fn has_package(&self, name: &str, version: &str) -> bool {
        let key = format!("{}@{}", name, version);
        self.packages.contains_key(&key)
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}
