use std::path::PathBuf;

#[derive(Debug)]
pub struct DownloadArtifact {
    pub key: String,
    pub path: PathBuf,
}
