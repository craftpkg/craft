use package::PackageJson;

#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub name: String,
    pub version: String,
    pub download_url: String,
    pub package: Option<PackageJson>,
}

impl ResolvedArtifact {
    pub fn to_cache_key(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}
