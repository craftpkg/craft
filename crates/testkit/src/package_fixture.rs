use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Map, Value, json};

const DEFAULT_ROOT_DIR: &str = "package";
const EXECUTABLE_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

/// A package version to publish to a [`crate::MockRegistry`].
///
/// Unless overridden with [`PackageFixture::file`], `index.js` requires every dependency and
/// peer dependency at load time and exports their versions, so `require()`-ing the package
/// proves its whole subtree resolves under Node.
#[derive(Debug, Clone)]
pub struct PackageFixture {
    pub(crate) name: String,
    pub(crate) version: String,
    manifest: Map<String, Value>,
    files: Vec<FixtureFile>,
    root_dir: String,
    pub(crate) integrity_override: Option<String>,
}

#[derive(Debug, Clone)]
struct FixtureFile {
    path: String,
    contents: String,
    mode: u32,
}

impl PackageFixture {
    pub fn new(name: &str, version: &str) -> Self {
        let mut manifest = Map::new();
        manifest.insert("name".to_string(), json!(name));
        manifest.insert("version".to_string(), json!(version));

        Self {
            name: name.to_string(),
            version: version.to_string(),
            manifest,
            files: Vec::new(),
            root_dir: DEFAULT_ROOT_DIR.to_string(),
            integrity_override: None,
        }
    }

    pub fn dependency(self, name: &str, spec: &str) -> Self {
        self.manifest_entry("dependencies", name, spec)
    }

    pub fn peer_dependency(self, name: &str, spec: &str) -> Self {
        self.manifest_entry("peerDependencies", name, spec)
    }

    pub fn optional_dependency(self, name: &str, spec: &str) -> Self {
        self.manifest_entry("optionalDependencies", name, spec)
    }

    pub fn manifest_field(mut self, key: &str, value: Value) -> Self {
        self.manifest.insert(key.to_string(), value);
        self
    }

    pub fn file(mut self, path: &str, contents: &str) -> Self {
        self.files.retain(|file| file.path != path);
        self.files.push(FixtureFile {
            path: path.to_string(),
            contents: contents.to_string(),
            mode: FILE_MODE,
        });
        self
    }

    /// Adds `"bin": { "<bin_name>": "<path>" }` and a script printing `<bin_name> <name>@<version>`.
    pub fn bin(self, bin_name: &str, path: &str) -> Self {
        let script = self.bin_script(bin_name);
        self.manifest_entry("bin", bin_name, path)
            .executable(path, &script)
    }

    /// Adds the string form `"bin": "<path>"`, whose bin name is the unscoped package name.
    pub fn string_bin(self, path: &str) -> Self {
        let bin_name = unscoped_name(&self.name).to_string();
        let script = self.bin_script(&bin_name);
        self.manifest_field("bin", json!(path))
            .executable(path, &script)
    }

    /// Top-level directory inside the tarball. npm uses `package/`; `@types/*` and some
    /// older packages use other names.
    pub fn root_dir(mut self, root_dir: &str) -> Self {
        self.root_dir = root_dir.to_string();
        self
    }

    /// Publishes a well-formed `sha512` integrity that doesn't match the tarball.
    pub fn wrong_integrity(mut self) -> Self {
        self.integrity_override = Some(format!("sha512-{}", BASE64.encode([0u8; 64])));
        self
    }

    pub(crate) fn manifest(&self) -> Value {
        Value::Object(self.manifest.clone())
    }

    /// The package as an npm tarball, for tests that install it from a file or URL.
    pub fn tarball(&self) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(encoder);

        let package_json =
            serde_json::to_string_pretty(&self.manifest()).expect("fixture manifest serializes");
        append_file(
            &mut archive,
            &self.root_dir,
            "package.json",
            &package_json,
            FILE_MODE,
        );

        let has_index = self.files.iter().any(|file| file.path == "index.js");
        if !has_index {
            append_file(
                &mut archive,
                &self.root_dir,
                "index.js",
                &self.default_index(),
                FILE_MODE,
            );
        }

        for file in &self.files {
            append_file(
                &mut archive,
                &self.root_dir,
                &file.path,
                &file.contents,
                file.mode,
            );
        }

        archive
            .into_inner()
            .and_then(|encoder| encoder.finish())
            .expect("fixture tarball builds in memory")
    }

    fn manifest_entry(mut self, section: &str, key: &str, value: &str) -> Self {
        let entries = self
            .manifest
            .entry(section.to_string())
            .or_insert_with(|| Value::Object(Map::new()));

        if let Value::Object(entries) = entries {
            entries.insert(key.to_string(), json!(value));
        }
        self
    }

    fn executable(mut self, path: &str, contents: &str) -> Self {
        self.files.retain(|file| file.path != path);
        self.files.push(FixtureFile {
            path: path.to_string(),
            contents: contents.to_string(),
            mode: EXECUTABLE_MODE,
        });
        self
    }

    fn bin_script(&self, bin_name: &str) -> String {
        let output = format!("{bin_name} {}@{}", self.name, self.version);
        format!("#!/usr/bin/env node\nconsole.log({});\n", json!(output))
    }

    fn default_index(&self) -> String {
        let required: Vec<String> = ["dependencies", "peerDependencies"]
            .iter()
            .filter_map(|section| self.manifest.get(*section).and_then(Value::as_object))
            .flat_map(|entries| entries.keys())
            .map(|name| format!("    {}: require({}).version,", json!(name), json!(name)))
            .collect();

        format!(
            "module.exports = {{\n  name: {},\n  version: {},\n  dependencies: {{\n{}\n  }},\n}};\n",
            json!(self.name),
            json!(self.version),
            required.join("\n"),
        )
    }
}

pub(crate) fn unscoped_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn append_file<W: std::io::Write>(
    archive: &mut tar::Builder<W>,
    root_dir: &str,
    path: &str,
    contents: &str,
    mode: u32,
) {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(mode);
    header.set_mtime(0);

    archive
        .append_data(
            &mut header,
            format!("{root_dir}/{path}"),
            contents.as_bytes(),
        )
        .expect("fixture file appends to tarball");
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;

    #[test]
    fn test_tarball_places_files_under_root_dir() {
        let package = PackageFixture::new("@types/node", "22.0.0")
            .root_dir("node")
            .file("index.d.ts", "export {};");

        let paths = tarball_paths(&package);

        assert!(paths.contains(&"node/package.json".to_string()));
        assert!(paths.contains(&"node/index.d.ts".to_string()));
        assert!(paths.contains(&"node/index.js".to_string()));
    }

    #[test]
    fn test_default_index_requires_dependencies_and_peers() {
        let package = PackageFixture::new("app", "1.0.0")
            .dependency("ms", "^2.0.0")
            .peer_dependency("react", "^19.0.0")
            .optional_dependency("fsevents", "^2.0.0");

        let index = package.default_index();

        assert!(index.contains(r#""ms": require("ms").version"#));
        assert!(index.contains(r#""react": require("react").version"#));
        assert!(!index.contains("fsevents"));
    }

    #[test]
    fn test_string_bin_uses_unscoped_name() {
        let package = PackageFixture::new("@scope/tool", "1.0.0").string_bin("cli.js");

        assert_eq!(package.manifest()["bin"], json!("cli.js"));
        assert!(tarball_paths(&package).contains(&"package/cli.js".to_string()));
    }

    fn tarball_paths(package: &PackageFixture) -> Vec<String> {
        let mut bytes = Vec::new();
        GzDecoder::new(package.tarball().as_slice())
            .read_to_end(&mut bytes)
            .expect("fixture tarball decompresses");

        tar::Archive::new(bytes.as_slice())
            .entries()
            .expect("fixture tarball is readable")
            .filter_map(|entry| {
                entry
                    .ok()?
                    .path()
                    .ok()
                    .map(|path| path.display().to_string())
            })
            .collect()
    }
}
