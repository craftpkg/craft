use contract::{NetworkError, Result};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Network {
    client: Client,
}

impl Network {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch JSON data from a URL and deserialize it into type [T]
    pub async fn fetch<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = successful(url, self.client.get(url).send().await?)?;
        let data = response.json::<T>().await?;
        Ok(data)
    }

    /// Downloads `from_url` to `store_path`, which only ever holds a complete download.
    pub async fn download(&self, from_url: &str, store_path: PathBuf) -> Result<()> {
        let response = successful(from_url, self.client.get(from_url).send().await?)?;
        let bytes = response.bytes().await?;

        tokio::task::spawn_blocking(move || write_atomically(&store_path, &bytes)).await?
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

fn successful(url: &str, response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    Err(NetworkError::Status {
        url: url.to_string(),
        status: status.as_u16(),
    }
    .into())
}

/// Writes to a temp file beside `path`, then renames it into place.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_new() {
        let network = Network::new();
        assert!(std::mem::size_of_val(&network.client) > 0);
    }

    #[test]
    fn test_write_atomically_leaves_only_the_complete_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pkg-1.0.0.tgz");
        std::fs::write(&path, "stale").unwrap();

        write_atomically(&path, b"complete").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"complete");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_network_default() {
        let network = Network::default();
        assert!(std::mem::size_of_val(&network.client) > 0);
    }
}
