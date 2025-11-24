use std::{fmt::Display, path::PathBuf};

#[derive(Debug)]
pub struct DownloadArtifact {
    pub key: String,
    pub path: PathBuf,
}

impl Display for DownloadArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}
