use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
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
    address: SocketAddr,
    stopping: Arc<AtomicBool>,
    state: Arc<Mutex<RegistryState>>,
    acceptor: Option<JoinHandle<()>>,
}

struct MockResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    retry_after: Option<u64>,
}

#[derive(Default)]
struct RegistryState {
    packuments: HashMap<String, Value>,
    tarballs: HashMap<String, Vec<u8>>,
    requested_paths: Vec<String>,
    /// When set, every request must carry `Authorization: Bearer <token>`.
    required_token: Option<String>,
    authorizations: Vec<Option<String>>,
    user_agents: Vec<Option<String>>,
    /// Requests left to fail, with the status and `Retry-After` to fail them with.
    pending_failures: Option<(usize, u16, Option<u64>)>,
}

impl MockRegistry {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock registry binds");
        let address = listener.local_addr().expect("mock registry has an address");
        let stopping = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(RegistryState::default()));

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
                    std::thread::spawn(move || serve(&stream, &state));
                }
            })
        };

        Self {
            url: format!("http://{address}"),
            address,
            stopping,
            state,
            acceptor: Some(acceptor),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
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

    /// The `User-Agent` header of every request so far, in order.
    pub fn user_agents(&self) -> Vec<Option<String>> {
        lock(&self.state).user_agents.clone()
    }

    /// The `Authorization` header of every request so far, in order.
    pub fn authorizations(&self) -> Vec<Option<String>> {
        lock(&self.state).authorizations.clone()
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
                retry_after: None,
            };
        }
        self.respond(url)
    }

    fn respond(&mut self, url: &str) -> MockResponse {
        let path = decode_path(url);
        self.requested_paths.push(path.clone());

        if let Some(tarball) = self.tarballs.get(&path) {
            return MockResponse {
                status: 200,
                content_type: "application/octet-stream",
                body: tarball.clone(),
                retry_after: None,
            };
        }

        match self.packuments.get(path.trim_start_matches('/')) {
            Some(packument) => MockResponse {
                status: 200,
                content_type: "application/json",
                body: packument.to_string().into_bytes(),
                retry_after: None,
            },
            None => MockResponse {
                status: 404,
                content_type: "application/json",
                body: br#"{"error":"Not found"}"#.to_vec(),
                retry_after: None,
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
fn serve(stream: &TcpStream, state: &Mutex<RegistryState>) {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut header_line = String::new();
    let mut authorization = None;
    let mut user_agent = None;
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
            }
        }
    }

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let response = lock(state).respond_authorized(path, authorization, user_agent);
    let reason = match response.status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Service Unavailable",
    };
    let retry_after = response
        .retry_after
        .map(|seconds| format!("Retry-After: {seconds}\r\n"))
        .unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{retry_after}Connection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );

    let mut writer = stream;
    let _ = writer.write_all(head.as_bytes());
    let _ = writer.write_all(&response.body);
    let _ = writer.flush();
}
