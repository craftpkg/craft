use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub name: String,
    pub version: String,
    pub resolved: String,
    pub integrity: Option<String>,
    pub dependencies: Option<HashMap<String, String>>,
}

impl PackageEntry {
    pub fn new(name: String, version: String, resolved: String) -> Self {
        Self {
            name,
            version,
            resolved,
            integrity: None,
            dependencies: None,
        }
    }

    pub fn with_integrity(mut self, integrity: String) -> Self {
        self.integrity = Some(integrity);
        self
    }

    pub fn with_dependencies(mut self, dependencies: HashMap<String, String>) -> Self {
        self.dependencies = Some(dependencies);
        self
    }

    /// Returns a unique key for this package entry
    pub fn key(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}
