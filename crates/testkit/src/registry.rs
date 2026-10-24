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
}

#[derive(Default)]
struct RegistryState {
    packuments: HashMap<String, Value>,
    tarballs: HashMap<String, Vec<u8>>,
    requested_paths: Vec<String>,
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
    fn respond(&mut self, url: &str) -> MockResponse {
        let path = decode_path(url);
        self.requested_paths.push(path.clone());

        if let Some(tarball) = self.tarballs.get(&path) {
            return MockResponse {
                status: 200,
                content_type: "application/octet-stream",
                body: tarball.clone(),
            };
        }

        match self.packuments.get(path.trim_start_matches('/')) {
            Some(packument) => MockResponse {
                status: 200,
                content_type: "application/json",
                body: packument.to_string().into_bytes(),
            },
            None => MockResponse {
                status: 404,
                content_type: "application/json",
                body: br#"{"error":"Not found"}"#.to_vec(),
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
    loop {
        header_line.clear();
        let is_end_of_headers = matches!(reader.read_line(&mut header_line), Ok(0) | Err(_))
            || header_line.trim_end().is_empty();
        if is_end_of_headers {
            break;
        }
    }

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let response = lock(state).respond(path);
    let reason = if response.status == 200 {
        "OK"
    } else {
        "Not Found"
    };
    let head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );

    let mut writer = stream;
    let _ = writer.write_all(head.as_bytes());
    let _ = writer.write_all(&response.body);
    let _ = writer.flush();
}
