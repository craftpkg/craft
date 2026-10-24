use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha512};
use tiny_http::{Header, Response, Server};

use crate::package_fixture::{PackageFixture, unscoped_name};

/// An npm-compatible registry on `127.0.0.1` serving packuments and tarballs built from
/// [`PackageFixture`]s. Stops when dropped.
pub struct MockRegistry {
    url: String,
    server: Arc<Server>,
    state: Arc<Mutex<RegistryState>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct RegistryState {
    packuments: HashMap<String, Value>,
    tarballs: HashMap<String, Vec<u8>>,
    requested_paths: Vec<String>,
}

impl MockRegistry {
    pub fn start() -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").expect("mock registry binds"));
        let port = server
            .server_addr()
            .to_ip()
            .map(|address| address.port())
            .expect("mock registry listens on TCP");
        let state = Arc::new(Mutex::new(RegistryState::default()));

        let worker = {
            let server = Arc::clone(&server);
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                for request in server.incoming_requests() {
                    let response = lock(&state).respond(request.url());
                    let _ = request.respond(response);
                }
            })
        };

        Self {
            url: format!("http://127.0.0.1:{port}"),
            server,
            state,
            worker: Some(worker),
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
        let tarball_path = format!(
            "/{}/-/{}-{}.tgz",
            package.name,
            unscoped_name(&package.name),
            package.version
        );

        let mut manifest = package.manifest();
        manifest["dist"] = json!({
            "tarball": format!("{}{tarball_path}", self.url),
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

    /// Every request path received so far, in order, with `%2f`-encoded scopes decoded.
    pub fn requested_paths(&self) -> Vec<String> {
        lock(&self.state).requested_paths.clone()
    }
}

impl Drop for MockRegistry {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl RegistryState {
    fn respond(&mut self, url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let path = decode_path(url);
        self.requested_paths.push(path.clone());

        if let Some(tarball) = self.tarballs.get(&path) {
            return Response::from_data(tarball.clone())
                .with_header(content_type("application/octet-stream"));
        }

        match self.packuments.get(path.trim_start_matches('/')) {
            Some(packument) => Response::from_data(packument.to_string().into_bytes())
                .with_header(content_type("application/json")),
            None => Response::from_data(br#"{"error":"Not found"}"#.to_vec())
                .with_status_code(404)
                .with_header(content_type("application/json")),
        }
    }
}

fn lock(state: &Mutex<RegistryState>) -> MutexGuard<'_, RegistryState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

fn content_type(value: &str) -> Header {
    Header::from_bytes("Content-Type", value).expect("static content type is a valid header")
}
