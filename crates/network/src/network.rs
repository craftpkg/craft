use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::Integrity;
use bytes::Bytes;
use contract::{NetworkError, RegistryConfig, Result, user_agent};
use reqwest::header::{AUTHORIZATION, RETRY_AFTER};
use reqwest::{Certificate, Client, ClientBuilder, NoProxy, Proxy, RequestBuilder};
use serde::de::DeserializeOwned;
use std::io::Write;
use std::path::{Path, PathBuf};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a response may stall between reads; large tarballs may take longer overall.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const BASE_RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const MAX_JITTER_MILLIS: u64 = 250;

#[derive(Debug)]
pub struct Network {
    client: Client,
    registry: RegistryConfig,
    /// Caps requests in flight; the nested parallelism of resolution would otherwise open
    /// connections without bound.
    permits: Arc<Semaphore>,
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
            permits: Arc::new(Semaphore::new(registry.network_concurrency)),
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
        let body = self.get_body(url).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// Downloads `from_url` to `store_path`, which only ever holds a complete download that
    /// matched `integrity`.
    pub async fn download(
        &self,
        from_url: &str,
        store_path: PathBuf,
        integrity: Option<&Integrity>,
    ) -> Result<()> {
        let bytes = self.get_body(from_url).await?;
        if let Some(integrity) = integrity {
            integrity.verify(&bytes)?;
        }

        tokio::task::spawn_blocking(move || write_atomically(&store_path, &bytes)).await?
    }
}

impl Network {
    /// GETs `url`, retrying connection failures, timeouts, interrupted bodies, 429 and 5xx
    /// with exponential backoff (or the server's `Retry-After`), `fetch-retries` times.
    async fn get_body(&self, url: &str) -> Result<Bytes> {
        let mut attempt = 0;
        loop {
            let failure = match self.attempt(url).await {
                Ok(body) => return Ok(body),
                Err(failure) => failure,
            };
            let may_retry = failure.is_retryable && attempt < self.registry.fetch_retries;
            if !may_retry {
                return Err(failure.error);
            }

            let delay = retry_delay(attempt, failure.retry_after);
            debug::warning!("Retrying {url} in {delay:?}: {:#}", failure.error);
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    async fn attempt(&self, url: &str) -> std::result::Result<Bytes, Failure> {
        let _permit = self.permits.acquire().await.map_err(|error| Failure {
            error: error.into(),
            is_retryable: false,
            retry_after: None,
        })?;
        let response = self
            .get(url)
            .send()
            .await
            .map_err(Failure::from_transport)?;
        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(Failure {
                error: NetworkError::Status {
                    url: url.to_string(),
                    status: status.as_u16(),
                }
                .into(),
                is_retryable: is_retryable_status(status.as_u16()),
                retry_after,
            });
        }
        response.bytes().await.map_err(Failure::from_transport)
    }
}

struct Failure {
    error: anyhow::Error,
    is_retryable: bool,
    retry_after: Option<Duration>,
}

impl Failure {
    fn from_transport(error: reqwest::Error) -> Self {
        let is_retryable =
            error.is_connect() || error.is_timeout() || error.is_body() || error.is_request();
        Self {
            error: error.into(),
            is_retryable,
            retry_after: None,
        }
    }
}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

/// `Retry-After` when the server sent one, otherwise doubling from half a second, plus up to
/// a quarter second of jitter so parallel requests don't retry in lockstep.
fn retry_delay(attempt: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(retry_after) = retry_after {
        return retry_after.min(MAX_RETRY_DELAY);
    }
    let backoff = BASE_RETRY_DELAY.saturating_mul(2u32.saturating_pow(attempt));
    let jitter_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::from(elapsed.subsec_nanos()) % MAX_JITTER_MILLIS
        });
    (backoff + Duration::from_millis(jitter_millis)).min(MAX_RETRY_DELAY)
}

impl Default for Network {
    fn default() -> Self {
        Self::new(&RegistryConfig::default())
    }
}

fn configured_client(registry: &RegistryConfig) -> ClientBuilder {
    let mut builder = Client::builder()
        .user_agent(user_agent())
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .danger_accept_invalid_certs(!registry.strict_ssl);
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
    fn test_retries_transient_statuses_only() {
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(429));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(401));
    }

    #[test]
    fn test_retry_delay_backs_off_and_honours_retry_after() {
        let first = retry_delay(0, None);
        let third = retry_delay(2, None);
        assert!(first >= BASE_RETRY_DELAY && first < BASE_RETRY_DELAY * 2);
        assert!(third >= BASE_RETRY_DELAY * 4);
        assert_eq!(
            retry_delay(0, Some(Duration::from_secs(3))),
            Duration::from_secs(3)
        );
        assert_eq!(
            retry_delay(9, Some(Duration::from_secs(600))),
            MAX_RETRY_DELAY
        );
        assert_eq!(retry_delay(30, None), MAX_RETRY_DELAY);
    }

    #[test]
    fn test_network_default() {
        let network = Network::default();
        assert!(std::mem::size_of_val(&network.client) > 0);
    }
}
