use anyhow::Context as _;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempPath;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::Integrity;
use bytes::Bytes;
use contract::{NetworkError, NetworkMode, RegistryConfig, Result, user_agent};
use reqwest::StatusCode;
use reqwest::header::{
    AUTHORIZATION, CACHE_CONTROL, ETAG, HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH,
    LAST_MODIFIED, RETRY_AFTER,
};
use reqwest::{Certificate, Client, ClientBuilder, NoProxy, Proxy, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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

    /// GETs `url` conditionally on the cached copy's validators: `NotModified` means the cached
    /// body is still current.
    pub async fn fetch_revalidated(
        &self,
        url: &str,
        cached: Option<&Validators>,
    ) -> Result<Revalidated> {
        self.with_retries(url, || async {
            let mut request = self.get(url);
            if let Some(etag) = cached.and_then(|cached| cached.etag.as_deref()) {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(modified) = cached.and_then(|cached| cached.last_modified.as_deref()) {
                request = request.header(IF_MODIFIED_SINCE, modified);
            }

            let response = self.send_request(url, request).await?;
            let validators = Validators::from_headers(response.response.headers());
            if response.response.status() == StatusCode::NOT_MODIFIED {
                return Ok(Revalidated::NotModified(validators));
            }
            let body = response.bytes().await.map_err(Failure::from_transport)?;
            Ok(Revalidated::Fresh { body, validators })
        })
        .await
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
        let dir = store_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let temp_path = self
            .with_retries(from_url, || {
                self.stream_to_temp_file(from_url, &dir, integrity)
            })
            .await?;
        temp_path
            .persist(&store_path)
            .with_context(|| format!("could not save {}", store_path.display()))?;
        Ok(())
    }

    /// Streams the body into a temp file beside the destination while hashing it, so a large
    /// tarball is never held in memory and nothing lands at the destination unverified.
    async fn stream_to_temp_file(
        &self,
        url: &str,
        dir: &Path,
        integrity: Option<&Integrity>,
    ) -> std::result::Result<TempPath, Failure> {
        let mut response = self.send(url).await?;
        let (file, temp_path) = tempfile::NamedTempFile::new_in(dir)
            .map_err(Failure::fatal)?
            .into_parts();
        let mut file = tokio::fs::File::from_std(file);
        let mut hasher = integrity.map(Integrity::hasher);

        while let Some(chunk) = response.chunk().await.map_err(Failure::from_transport)? {
            if let Some(hasher) = &mut hasher {
                hasher.update(&chunk);
            }
            file.write_all(&chunk).await.map_err(Failure::fatal)?;
        }
        file.flush().await.map_err(Failure::fatal)?;
        if let Some(hasher) = hasher {
            hasher.verify().map_err(Failure::fatal)?;
        }
        Ok(temp_path)
    }
}

impl Network {
    /// GETs `url` and reads the whole body, with retries.
    async fn get_body(&self, url: &str) -> Result<Bytes> {
        self.with_retries(url, || async {
            let response = self.send(url).await?;
            response.bytes().await.map_err(Failure::from_transport)
        })
        .await
    }

    /// Runs `operation` until it succeeds, retrying connection failures, timeouts, interrupted
    /// bodies, 429 and 5xx with exponential backoff (or the server's `Retry-After`),
    /// `fetch-retries` times.
    async fn with_retries<T, F, Fut>(&self, url: &str, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = std::result::Result<T, Failure>>,
    {
        let mut attempt = 0;
        loop {
            let failure = match operation().await {
                Ok(value) => return Ok(value),
                Err(failure) => failure,
            };
            let may_retry = failure.is_retryable && attempt < self.registry.fetch_retries;
            if !may_retry {
                return Err(failure.into_error(url));
            }

            let delay = retry_delay(attempt, failure.retry_after);
            debug::warning!("Retrying {url} in {delay:?}: {:#}", failure.error);
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    /// Sends the GET and checks its status, holding a request permit while the response is
    /// in flight.
    async fn send(&self, url: &str) -> std::result::Result<PermittedResponse, Failure> {
        self.send_request(url, self.get(url)).await
    }

    async fn send_request(
        &self,
        url: &str,
        request: RequestBuilder,
    ) -> std::result::Result<PermittedResponse, Failure> {
        if self.registry.network_mode == NetworkMode::Offline {
            return Err(Failure::fatal(NetworkError::Offline {
                url: url.to_string(),
            }));
        }
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(Failure::fatal)?;
        let response = request.send().await.map_err(Failure::from_transport)?;
        let status = response.status();
        let is_accepted = status.is_success() || status == StatusCode::NOT_MODIFIED;
        if !is_accepted {
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
                is_unreachable: false,
            });
        }
        Ok(PermittedResponse {
            response,
            _permit: permit,
        })
    }
}

/// What a cached response can be revalidated with, and how long it stays fresh.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// `Cache-Control: max-age`, in seconds.
    pub max_age: Option<u64>,
}

impl Validators {
    fn from_headers(headers: &HeaderMap) -> Self {
        let header = |name| {
            headers
                .get(name)
                .and_then(|value: &HeaderValue| value.to_str().ok())
                .map(str::to_string)
        };
        let max_age = header(CACHE_CONTROL).and_then(|cache_control| {
            cache_control
                .split(',')
                .find_map(|directive| directive.trim().strip_prefix("max-age="))
                .and_then(|seconds| seconds.parse().ok())
        });
        Self {
            etag: header(ETAG),
            last_modified: header(LAST_MODIFIED),
            max_age,
        }
    }
}

pub enum Revalidated {
    /// The cached body is current; the validators may carry a renewed `max_age`.
    NotModified(Validators),
    Fresh {
        body: Bytes,
        validators: Validators,
    },
}

/// A response that keeps its request permit until the body has been read.
struct PermittedResponse {
    response: reqwest::Response,
    _permit: OwnedSemaphorePermit,
}

impl PermittedResponse {
    async fn chunk(&mut self) -> reqwest::Result<Option<Bytes>> {
        self.response.chunk().await
    }

    async fn bytes(self) -> reqwest::Result<Bytes> {
        self.response.bytes().await
    }
}

struct Failure {
    error: anyhow::Error,
    is_retryable: bool,
    retry_after: Option<Duration>,
    /// The request never got a response: no connection, or it timed out.
    is_unreachable: bool,
}

impl Failure {
    fn fatal(error: impl Into<anyhow::Error>) -> Self {
        Self {
            error: error.into(),
            is_retryable: false,
            retry_after: None,
            is_unreachable: false,
        }
    }

    fn from_transport(error: reqwest::Error) -> Self {
        let is_unreachable = error.is_connect() || error.is_timeout();
        let is_retryable = is_unreachable || error.is_body() || error.is_request();
        Self {
            error: error.into(),
            is_retryable,
            retry_after: None,
            is_unreachable,
        }
    }

    /// The error to report once retries are exhausted, saying what to try when the registry
    /// could not be reached at all.
    fn into_error(self, url: &str) -> anyhow::Error {
        if !self.is_unreachable {
            return self.error;
        }
        let host = url
            .split_once("://")
            .map_or(url, |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or(url);
        let reason = self.error.root_cause();
        anyhow::anyhow!(
            "could not reach {host} ({reason}); check your network connection and proxy settings, or use --offline"
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_new() {
        let network = Network::new(&RegistryConfig::default());
        assert!(std::mem::size_of_val(&network.client) > 0);
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
