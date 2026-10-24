use crate::Integrity;
use contract::{NetworkError, RegistryConfig, Result};
use reqwest::header::AUTHORIZATION;
use reqwest::{Certificate, Client, ClientBuilder, NoProxy, Proxy, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Network {
    client: Client,
    registry: RegistryConfig,
}

impl Network {
    /// A client honouring the registry configuration's proxy, TLS and CA settings, which sends
    /// each URL the credentials configured for its host and nothing else.
    pub fn new(registry: &RegistryConfig) -> Self {
        let client = configured_client(registry).build().unwrap_or_else(|error| {
            eprintln!("warning: ignoring network settings from .npmrc: {error}");
            Client::new()
        });
        Self {
            client,
            registry: registry.clone(),
        }
    }

    fn get(&self, url: &str) -> RequestBuilder {
        let request = self.client.get(url);
        match self.registry.credentials_for(url) {
            Some(credentials) => request.header(AUTHORIZATION, credentials.header()),
            None => request,
        }
    }

    /// Fetch JSON data from a URL and deserialize it into type [T]
    pub async fn fetch<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = successful(url, self.get(url).send().await?)?;
        let data = response.json::<T>().await?;
        Ok(data)
    }

    /// Downloads `from_url` to `store_path`, which only ever holds a complete download that
    /// matched `integrity`.
    pub async fn download(
        &self,
        from_url: &str,
        store_path: PathBuf,
        integrity: Option<&Integrity>,
    ) -> Result<()> {
        let response = successful(from_url, self.get(from_url).send().await?)?;
        let bytes = response.bytes().await?;
        if let Some(integrity) = integrity {
            integrity.verify(&bytes)?;
        }

        tokio::task::spawn_blocking(move || write_atomically(&store_path, &bytes)).await?
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new(&RegistryConfig::default())
    }
}

fn configured_client(registry: &RegistryConfig) -> ClientBuilder {
    let mut builder = Client::builder().danger_accept_invalid_certs(!registry.strict_ssl);
    let no_proxy = registry.no_proxy.as_deref().and_then(NoProxy::from_string);
    if let Some(url) = &registry.https_proxy {
        builder = with_proxy(builder, Proxy::https(url.as_str()), url, &no_proxy);
    }
    if let Some(url) = &registry.http_proxy {
        builder = with_proxy(builder, Proxy::http(url.as_str()), url, &no_proxy);
    }

    if let Some(cafile) = &registry.cafile {
        let certificates = std::fs::read(cafile)
            .map_err(|error| error.to_string())
            .and_then(|pem| Certificate::from_pem(&pem).map_err(|error| error.to_string()));
        match certificates {
            Ok(certificate) => builder = builder.add_root_certificate(certificate),
            Err(error) => eprintln!("warning: ignoring cafile {}: {error}", cafile.display()),
        }
    }
    builder
}

fn with_proxy(
    builder: ClientBuilder,
    proxy: reqwest::Result<Proxy>,
    url: &str,
    no_proxy: &Option<NoProxy>,
) -> ClientBuilder {
    match proxy {
        Ok(proxy) => builder.proxy(proxy.no_proxy(no_proxy.clone())),
        Err(error) => {
            eprintln!("warning: ignoring proxy {url}: {error}");
            builder
        }
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
        let network = Network::new(&RegistryConfig::default());
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
