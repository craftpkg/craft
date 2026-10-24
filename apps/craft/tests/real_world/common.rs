use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::OnceLock;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

/// A project on disk installed from the real registry. Projects share one craft home per test
/// run, as projects on a developer's machine do, unless created with [`World::isolated`].
pub struct World {
    dir: TempDir,
    home: Home,
}

enum Home {
    Shared,
    Own(TempDir),
}

impl World {
    pub fn new(package_json: &Value) -> Self {
        Self::create(package_json, Home::Shared)
    }

    /// A project with a craft home of its own: an empty store and metadata cache.
    pub fn isolated(package_json: &Value) -> Self {
        let home = tempfile::tempdir_in(scratch_dir()).expect("craft home tempdir");
        Self::create(package_json, Home::Own(home))
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn craft_home(&self) -> &Path {
        match &self.home {
            Home::Shared => shared_home(),
            Home::Own(dir) => dir.path(),
        }
    }

    pub fn craft(&self) -> Command {
        self.craft_in(".")
    }

    pub fn craft_in(&self, relative_dir: &str) -> Command {
        let mut command = StdCommand::new(env!("CARGO_BIN_EXE_craft"));
        command
            .current_dir(self.path().join(relative_dir))
            .env("CRAFT_HOME", self.craft_home())
            .env("HOME", self.craft_home())
            .env_remove("CRAFT_REGISTRY")
            .env_remove("NODE_PATH")
            .env_remove("CI");
        let npm_config_vars = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.to_ascii_lowercase().starts_with("npm_config_"));
        for npm_config_var in npm_config_vars {
            command.env_remove(npm_config_var);
        }
        Command::from_std(command)
    }

    /// Runs `script` with Node from the project root and returns what it printed.
    pub fn node(&self, script: &str) -> String {
        self.run_node(&["-e", script])
    }

    /// Like [`World::node`], for a script using ES module `import`s.
    pub fn node_module(&self, script: &str) -> String {
        self.run_node(&["--input-type=module", "-e", script])
    }

    pub fn read(&self, relative_path: &str) -> String {
        std::fs::read_to_string(self.path().join(relative_path))
            .unwrap_or_else(|error| panic!("read {relative_path}: {error}"))
    }

    pub fn write(&self, relative_path: &str, contents: &str) {
        let path = self.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("write {relative_path}: {error}"));
    }

    pub fn package_json(&self) -> Value {
        serde_json::from_str(&self.read("package.json")).expect("package.json is JSON")
    }

    pub fn lockfile(&self) -> Value {
        serde_json::from_str(&self.read("craft.lock")).expect("craft.lock is JSON")
    }

    /// Package entries in the project's virtual store, `node_modules/.craft`, without its
    /// hoisting directory and state file.
    pub fn virtual_store(&self) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(self.path().join("node_modules/.craft"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.contains('@'))
                    .collect()
            })
            .unwrap_or_default();
        entries.sort();
        entries
    }

    pub fn remove(&self, relative_path: &str) {
        let path = self.path().join(relative_path);
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.unwrap_or_else(|error| panic!("remove {relative_path}: {error}"));
    }

    fn create(package_json: &Value, home: Home) -> Self {
        let world = Self {
            dir: tempfile::tempdir_in(scratch_dir()).expect("project tempdir"),
            home,
        };
        let content = serde_json::to_string_pretty(package_json).expect("package.json serializes");
        world.write("package.json", &content);
        world
    }

    fn run_node(&self, args: &[&str]) -> String {
        let output = StdCommand::new("node")
            .args(args)
            .current_dir(self.path())
            .env_remove("NODE_PATH")
            .output()
            .expect("node is required to run the real-world tests");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "node failed:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
    }
}

/// Starts empty on every run, so the first project to need a package downloads it.
fn shared_home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let home = scratch_dir().join("shared-craft-home");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).expect("create the shared craft home");
        home
    })
}

/// Outside the repository, whose own `node_modules` Node and craft would otherwise find by
/// walking up from a project.
fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("craft-real-world");
    std::fs::create_dir_all(&dir).expect("create the real-world scratch directory");
    dir
}
