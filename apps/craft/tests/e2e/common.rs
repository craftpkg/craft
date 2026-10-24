use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::rc::Rc;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::TempDir;
use testkit::MockRegistry;

const PROXY_VARS: [&str; 6] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

/// A throwaway project directory with its own craft home, wired to a [`MockRegistry`].
pub struct Project {
    dir: TempDir,
    craft_home: Rc<TempDir>,
    registry_url: String,
}

impl Project {
    pub fn new(registry: &MockRegistry, package_json: Value) -> Self {
        let craft_home = Rc::new(tempfile::tempdir().expect("craft home tempdir"));
        Self::create(registry.url(), craft_home, package_json)
    }

    /// Another project using the same craft home (and therefore the same global cache).
    pub fn sibling(&self, package_json: Value) -> Self {
        Self::create(
            &self.registry_url,
            Rc::clone(&self.craft_home),
            package_json,
        )
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn craft_home(&self) -> &Path {
        self.craft_home.path()
    }

    pub fn store_dir(&self) -> PathBuf {
        self.craft_home().join("store/v1")
    }

    /// Names in the global store, sorted; empty when the store doesn't exist yet.
    pub fn store_entries(&self) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(self.store_dir())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        entries.sort();
        entries
    }

    pub fn craft(&self) -> Command {
        Command::from_std(self.craft_process())
    }

    /// The same invocation as [`Project::craft`], for spawning without waiting.
    pub fn craft_process(&self) -> StdCommand {
        let mut command = StdCommand::new(env!("CARGO_BIN_EXE_craft"));
        command
            .current_dir(self.path())
            .env("CRAFT_HOME", self.craft_home())
            .env("CRAFT_REGISTRY", &self.registry_url)
            .env("HOME", self.craft_home())
            .env_remove("NODE_PATH")
            .env_remove("CI");

        for proxy_var in PROXY_VARS {
            command.env_remove(proxy_var);
        }
        command
    }

    pub fn package_json(&self) -> Value {
        let content = self.read_file("package.json");
        serde_json::from_str(&content).expect("package.json is valid JSON")
    }

    pub fn read_file(&self, relative_path: &str) -> String {
        std::fs::read_to_string(self.path().join(relative_path))
            .unwrap_or_else(|error| panic!("read {relative_path}: {error}"))
    }

    pub fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("write {relative_path}: {error}"));
    }

    /// `require(module)` from the project root under Node, returning the module's exports
    /// as JSON, or Node's error output if it can't be loaded.
    pub fn require(&self, module: &str) -> Result<Value, String> {
        let script = format!("JSON.stringify(require({}))", json!(module));
        let output = StdCommand::new("node")
            .arg("-p")
            .arg(script)
            .current_dir(self.path())
            .env_remove("NODE_PATH")
            .output()
            .expect("node is required to run the e2e tests");

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    /// Version exported by a fixture package, as seen by Node from the project root.
    pub fn required_version(&self, module: &str) -> Result<String, String> {
        let exports = self.require(module)?;
        exports["version"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{module} exports no version: {exports}"))
    }

    fn create(registry_url: &str, craft_home: Rc<TempDir>, package_json: Value) -> Self {
        let project = Self {
            dir: tempfile::tempdir().expect("project tempdir"),
            craft_home,
            registry_url: registry_url.to_string(),
        };
        let content = serde_json::to_string_pretty(&package_json).expect("package.json serializes");
        project.write_file("package.json", &content);
        project
    }
}
