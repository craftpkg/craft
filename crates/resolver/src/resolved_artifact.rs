use std::fmt::Display;

use network::Integrity;
use package::PackageJson;

#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub name: String,
    pub version: String,
    pub download_url: String,
    pub integrity: Option<Integrity>,
    pub package: Option<PackageJson>,
}

impl ResolvedArtifact {
    pub fn to_cache_key(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}

impl Display for ResolvedArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}
