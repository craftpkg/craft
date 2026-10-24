use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha512};

use crate::package_fixture::{PackageFixture, unscoped_name};

/// An npm-compatible registry on `127.0.0.1` serving packuments and tarballs built from
/// [`PackageFixture`]s. Stops when dropped.
pub struct MockRegistry {
    url: String,
    certificate_pem: Option<String>,
    address: SocketAddr,
    stopping: Arc<AtomicBool>,
    state: Arc<Mutex<RegistryState>>,
    acceptor: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct MockResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    retry_after: Option<u64>,
    etag: Option<String>,
    max_age: Option<u64>,
}

#[derive(Default)]
struct RegistryState {
    packuments: HashMap<String, Value>,
    tarballs: HashMap<String, Vec<u8>>,
    requested_paths: Vec<String>,
    /// When set, every request must carry `Authorization: Bearer <token>`.
    required_token: Option<String>,
    authorizations: Vec<Option<String>>,
    /// The host of every request that arrived in proxy form (`GET http://host/path`).
    proxied_hosts: Vec<String>,
    user_agents: Vec<Option<String>>,
    /// Requests left to fail, with the status and `Retry-After` to fail them with.
    pending_failures: Option<(usize, u16, Option<u64>)>,
    in_flight: usize,
    max_in_flight: usize,
    /// `Cache-Control: max-age` to send with packuments.
    max_age: Option<u64>,
    not_modified: usize,
}

impl MockRegistry {
    pub fn start() -> Self {
        Self::listen(None)
    }

    /// The same registry over HTTPS, with a freshly generated self-signed certificate for
    /// `127.0.0.1` that clients have to be told to trust ([`MockRegistry::certificate_pem`]).
    pub fn start_tls() -> Self {
        let certified = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()])
            .expect("generate a self-signed certificate");
        let certificate_pem = certified.cert.pem();
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("TLS protocol versions")
            .with_no_client_auth()
            .with_single_cert(vec![certified.cert.der().clone()], key.into())
            .expect("TLS server config");
        let mut registry = Self::listen(Some(Arc::new(config)));
        registry.certificate_pem = Some(certificate_pem);
        registry
    }

    fn listen(tls: Option<Arc<rustls::ServerConfig>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock registry binds");
        let address = listener.local_addr().expect("mock registry has an address");
        let stopping = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(RegistryState::default()));

        let scheme = if tls.is_some() { "https" } else { "http" };
        let acceptor = {
            let stopping = Arc::clone(&stopping);
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(stream) = stream else {
                        continue;
                    };
                    let state = Arc::clone(&state);
                    let tls = tls.clone();
                    std::thread::spawn(move || serve(stream, tls, &state));
                }
            })
        };

        Self {
            url: format!("{scheme}://{address}"),
            certificate_pem: None,
            address,
            stopping,
            state,
            acceptor: Some(acceptor),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// The PEM certificate a [`MockRegistry::start_tls`] registry presents.
    pub fn certificate_pem(&self) -> Option<&str> {
        self.certificate_pem.as_deref()
    }

    /// Publishes a version and, like `npm publish`, points `latest` at it.
    pub fn publish(&self, package: PackageFixture) -> &Self {
        let tarball = package.tarball();
        let integrity = package
            .integrity_override
            .clone()
            .unwrap_or_else(|| sri_sha512(&tarball));
        let tarball_path = tarball_path(&package.name, &package.version);

        let mut manifest = package.manifest();
        manifest["dist"] = json!({
            "tarball": self.tarball_url(&package.name, &package.version),
            "integrity": integrity,
        });

        let mut state = lock(&self.state);
        state.tarballs.insert(tarball_path, tarball);

        let packument = state
            .packuments
            .entry(package.name.clone())
            .or_insert_with(|| empty_packument(&package.name));
        packument["versions"][&package.version] = manifest;
        packument["dist-tags"]["latest"] = json!(package.version);

        self
    }

    pub fn set_dist_tag(&self, name: &str, tag: &str, version: &str) -> &Self {
        let mut state = lock(&self.state);
        if let Some(packument) = state.packuments.get_mut(name) {
            packument["dist-tags"][tag] = json!(version);
        }
        self
    }

    pub fn tarball_url(&self, name: &str, version: &str) -> String {
        format!("{}{}", self.url, tarball_path(name, version))
    }

    /// Leaves the version in the packument but makes its tarball URL return 404.
    pub fn remove_tarball(&self, name: &str, version: &str) -> &Self {
        lock(&self.state)
            .tarballs
            .remove(&tarball_path(name, version));
        self
    }

    /// Answers 401 to any request without `Authorization: Bearer <token>`.
    pub fn require_token(&self, token: &str) -> &Self {
        lock(&self.state).required_token = Some(token.to_string());
        self
    }

    /// Fails the next `count` requests with `status`, sending `Retry-After: <seconds>` if given.
    pub fn fail_next(&self, count: usize, status: u16, retry_after: Option<u64>) -> &Self {
        lock(&self.state).pending_failures = Some((count, status, retry_after));
        self
    }

    /// Sends `Cache-Control: max-age=<seconds>` with every packument.
    pub fn set_max_age(&self, seconds: u64) -> &Self {
        lock(&self.state).max_age = Some(seconds);
        self
    }

    /// How many conditional requests were answered `304 Not Modified`.
    pub fn not_modified_responses(&self) -> usize {
        lock(&self.state).not_modified
    }

    /// The most requests that were being answered at the same time.
    pub fn max_concurrent_requests(&self) -> usize {
        lock(&self.state).max_in_flight
    }

    /// The `User-Agent` header of every request so far, in order.
    pub fn user_agents(&self) -> Vec<Option<String>> {
        lock(&self.state).user_agents.clone()
    }

    /// The `Authorization` header of every request so far, in order.
    pub fn authorizations(&self) -> Vec<Option<String>> {
        lock(&self.state).authorizations.clone()
    }

    /// The hosts requested through this registry acting as an HTTP proxy, in order.
    pub fn proxied_hosts(&self) -> Vec<String> {
        lock(&self.state).proxied_hosts.clone()
    }

    /// Every request path received so far, in order, with `%2f`-encoded scopes decoded.
    pub fn requested_paths(&self) -> Vec<String> {
        lock(&self.state).requested_paths.clone()
    }
}

impl Drop for MockRegistry {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(acceptor) = self.acceptor.take() {
            let _ = acceptor.join();
        }
    }
}

impl RegistryState {
    fn respond_authorized(
        &mut self,
        url: &str,
        authorization: Option<String>,
        user_agent: Option<String>,
        if_none_match: Option<String>,
    ) -> MockResponse {
        self.user_agents.push(user_agent);
        if let Some((remaining, status, retry_after)) = self.pending_failures.take() {
            if remaining > 1 {
                self.pending_failures = Some((remaining - 1, status, retry_after));
            }
            self.requested_paths.push(decode_path(url));
            return MockResponse {
                status,
                content_type: "application/json",
                body: br#"{"error":"Temporarily unavailable"}"#.to_vec(),
                retry_after,
                ..MockResponse::default()
            };
        }
        let expected = self
            .required_token
            .as_ref()
            .map(|token| format!("Bearer {token}"));
        let is_authorized = expected.is_none() || expected == authorization;
        self.authorizations.push(authorization);
        if !is_authorized {
            self.requested_paths.push(decode_path(url));
            return MockResponse {
                status: 401,
                content_type: "application/json",
                body: br#"{"error":"Unauthorized"}"#.to_vec(),
                ..MockResponse::default()
            };
        }
        let response = self.respond(url);
        let is_not_modified = response.etag.is_some() && response.etag == if_none_match;
        if is_not_modified {
            self.not_modified += 1;
            return MockResponse {
                status: 304,
                body: Vec::new(),
                ..response
            };
        }
        response
    }

    fn respond(&mut self, url: &str) -> MockResponse {
        let path = decode_path(url);
        self.requested_paths.push(path.clone());

        if let Some(tarball) = self.tarballs.get(&path) {
            return MockResponse {
                status: 200,
                content_type: "application/octet-stream",
                body: tarball.clone(),
                ..MockResponse::default()
            };
        }

        match self.packuments.get(path.trim_start_matches('/')) {
            Some(packument) => {
                let body = packument.to_string().into_bytes();
                MockResponse {
                    status: 200,
                    content_type: "application/json",
                    etag: Some(format!("\"{}\"", sri_sha512(&body))),
                    max_age: self.max_age,
                    body,
                    ..MockResponse::default()
                }
            }
            None => MockResponse {
                status: 404,
                content_type: "application/json",
                body: br#"{"error":"Not found"}"#.to_vec(),
                ..MockResponse::default()
            },
        }
    }
}

fn lock(state: &Mutex<RegistryState>) -> MutexGuard<'_, RegistryState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn tarball_path(name: &str, version: &str) -> String {
    format!("/{name}/-/{}-{version}.tgz", unscoped_name(name))
}

fn empty_packument(name: &str) -> Value {
    json!({
        "name": name,
        "dist-tags": Map::new(),
        "versions": Map::new(),
    })
}

fn sri_sha512(bytes: &[u8]) -> String {
    format!("sha512-{}", BASE64.encode(Sha512::digest(bytes)))
}

fn decode_path(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    path.replace("%2f", "/")
        .replace("%2F", "/")
        .replace("%40", "@")
}

/// Answers one GET per connection and closes it, so clients never hold idle keep-alive
/// connections open against the registry.
fn serve(stream: TcpStream, tls: Option<Arc<rustls::ServerConfig>>, state: &Mutex<RegistryState>) {
    {
        let mut state = lock(state);
        state.in_flight += 1;
        state.max_in_flight = state.max_in_flight.max(state.in_flight);
    }
    match tls {
        Some(config) => {
            if let Ok(connection) = rustls::ServerConnection::new(config) {
                let mut stream = rustls::StreamOwned::new(connection, stream);
                answer(&mut stream, state);
                stream.conn.send_close_notify();
                let _ = stream.flush();
            }
        }
        None => {
            let mut stream = stream;
            answer(&mut stream, state);
        }
    }
    lock(state).in_flight -= 1;
}

/// A short pause before answering, so requests that arrive together overlap and concurrency
/// limits become observable.
const RESPONSE_DELAY: std::time::Duration = std::time::Duration::from_millis(5);

fn answer<S: Read + Write>(stream: &mut S, state: &Mutex<RegistryState>) {
    std::thread::sleep(RESPONSE_DELAY);
    let mut reader = BufReader::new(&mut *stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut header_line = String::new();
    let mut authorization = None;
    let mut user_agent = None;
    let mut if_none_match = None;
    loop {
        header_line.clear();
        let is_end_of_headers = matches!(reader.read_line(&mut header_line), Ok(0) | Err(_))
            || header_line.trim_end().is_empty();
        if is_end_of_headers {
            break;
        }
        if let Some((name, value)) = header_line.split_once(':') {
            let name = name.trim();
            if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("user-agent") {
                user_agent = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("if-none-match") {
                if_none_match = Some(value.trim().to_string());
            }
        }
    }

    let target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let path = match target.split_once("://") {
        Some((_, rest)) => {
            let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            lock(state).proxied_hosts.push(host.to_string());
            if path.is_empty() { "/" } else { path }
        }
        None => target,
    }
    .to_string();
    drop(reader);
    let response = lock(state).respond_authorized(&path, authorization, user_agent, if_none_match);
    let reason = match response.status {
        200 => "OK",
        304 => "Not Modified",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Service Unavailable",
    };
    let mut extra_headers = response
        .retry_after
        .map(|seconds| format!("Retry-After: {seconds}\r\n"))
        .unwrap_or_default();
    if let Some(etag) = &response.etag {
        extra_headers.push_str(&format!("ETag: {etag}\r\n"));
    }
    if let Some(max_age) = response.max_age {
        extra_headers.push_str(&format!("Cache-Control: max-age={max_age}\r\n"));
    }
    let head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );

    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}
