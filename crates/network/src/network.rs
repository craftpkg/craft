use contract::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct Network {
    client: Client,
}

impl Network {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch JSON data from a URL and deserialize it into type T
    pub async fn fetch<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self.client.get(url).send().await?;
        let data = response.json::<T>().await?;
        Ok(data)
    }

    /// Download a file from a URL and save it to the specified path
    pub async fn download(&self, from_url: &str, store_path: PathBuf) -> Result<()> {
        let response = self.client.get(from_url).send().await?;
        let bytes = response.bytes().await?;

        let mut file = File::create(store_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        Ok(())
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_network_default() {
        let network = Network::default();
        assert!(std::mem::size_of_val(&network.client) > 0);
    }
}
