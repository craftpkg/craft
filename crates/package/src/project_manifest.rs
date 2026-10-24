use std::path::{Path, PathBuf};

use contract::{PackageError, Result};
use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use serde_json::{Map, Serializer, Value};

const DEFAULT_INDENT: &str = "  ";
const BYTE_ORDER_MARK: char = '\u{feff}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencySection {
    Dependencies,
    DevDependencies,
}

impl DependencySection {
    pub const ALL: [Self; 2] = [Self::Dependencies, Self::DevDependencies];

    pub fn key(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "devDependencies",
        }
    }
}

/// A project's `package.json` opened for editing. Only dependency sections change; key order,
/// indentation, line endings and the trailing newline are written back as they were found.
#[derive(Debug)]
pub struct ProjectManifest {
    path: PathBuf,
    root: Map<String, Value>,
    style: Style,
}

#[derive(Debug, PartialEq)]
struct Style {
    indent: String,
    is_crlf: bool,
    has_trailing_newline: bool,
}

impl ProjectManifest {
    pub async fn open(path: &Path) -> Result<Self> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(PackageError::NoPackageJson.into());
            }
            Err(error) => return Err(error.into()),
        };
        Self::parse(path, &content)
    }

    pub fn parse(path: &Path, content: &str) -> Result<Self> {
        let content = content.trim_start_matches(BYTE_ORDER_MARK);
        let Value::Object(root) = serde_json::from_str(content)? else {
            return Err(PackageError::NotAnObject.into());
        };

        Ok(Self {
            path: path.to_path_buf(),
            root,
            style: Style::detect(content),
        })
    }

    /// Updates an existing entry in place, or inserts a new one at its alphabetical position.
    pub fn set_dependency(&mut self, section: DependencySection, name: &str, spec: &str) {
        let section_value = self
            .root
            .entry(section.key())
            .or_insert_with(|| Value::Object(Map::new()));
        if !section_value.is_object() {
            *section_value = Value::Object(Map::new());
        }
        let Value::Object(dependencies) = section_value else {
            return;
        };

        if let Some(existing) = dependencies.get_mut(name) {
            *existing = Value::String(spec.to_string());
            return;
        }
        *dependencies = insert_sorted(std::mem::take(dependencies), name, spec);
    }

    /// Removes the entry, and the section with it once empty. Returns whether it was listed.
    pub fn remove_dependency(&mut self, section: DependencySection, name: &str) -> bool {
        let Some(Value::Object(dependencies)) = self.root.get_mut(section.key()) else {
            return false;
        };
        let was_listed = dependencies.shift_remove(name).is_some();

        if dependencies.is_empty() {
            self.root.shift_remove(section.key());
        }
        was_listed
    }

    pub fn render(&self) -> Result<String> {
        let mut bytes = Vec::new();
        let formatter = PrettyFormatter::with_indent(self.style.indent.as_bytes());
        self.root
            .serialize(&mut Serializer::with_formatter(&mut bytes, formatter))?;

        let mut rendered = String::from_utf8(bytes)?;
        if self.style.has_trailing_newline {
            rendered.push('\n');
        }
        if self.style.is_crlf {
            rendered = rendered.replace('\n', "\r\n");
        }
        Ok(rendered)
    }

    pub async fn save(&self) -> Result<()> {
        tokio::fs::write(&self.path, self.render()?).await?;
        Ok(())
    }
}

impl Style {
    fn detect(content: &str) -> Self {
        let indent = content
            .lines()
            .skip(1)
            .map(|line| {
                let content_start = line.len() - line.trim_start_matches([' ', '\t']).len();
                &line[..content_start]
            })
            .find(|indent| !indent.is_empty())
            .unwrap_or(DEFAULT_INDENT);

        Self {
            indent: indent.to_string(),
            is_crlf: content.contains("\r\n"),
            has_trailing_newline: content.ends_with('\n'),
        }
    }
}

fn insert_sorted(dependencies: Map<String, Value>, name: &str, spec: &str) -> Map<String, Value> {
    let mut sorted = Map::new();
    let mut pending = Some((name.to_string(), Value::String(spec.to_string())));

    for (existing_name, existing_spec) in dependencies {
        let belongs_before = pending
            .as_ref()
            .is_some_and(|(new_name, _)| new_name.as_str() < existing_name.as_str());
        if belongs_before && let Some((new_name, new_spec)) = pending.take() {
            sorted.insert(new_name, new_spec);
        }
        sorted.insert(existing_name, existing_spec);
    }
    if let Some((new_name, new_spec)) = pending {
        sorted.insert(new_name, new_spec);
    }
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL: &str = "{\n    \"name\": \"app\",\n    \"private\": true,\n    \"type\": \"module\",\n    \"dependencies\": {\n        \"react\": \"^19.0.0\",\n        \"zod\": \"^3.0.0\"\n    }\n}\n";

    fn manifest(content: &str) -> ProjectManifest {
        ProjectManifest::parse(Path::new("package.json"), content).unwrap()
    }

    #[test]
    fn test_unchanged_manifest_renders_byte_identical() {
        for content in [
            ORIGINAL,
            "{\n\t\"name\": \"app\",\n\t\"b\": 1,\n\t\"a\": [\n\t\t1\n\t]\n}",
            "{\r\n  \"type\": \"module\",\r\n  \"private\": true\r\n}\r\n",
        ] {
            assert_eq!(manifest(content).render().unwrap(), content);
        }
    }

    #[test]
    fn test_set_then_remove_restores_the_original() {
        let mut manifest = manifest(ORIGINAL);

        manifest.set_dependency(DependencySection::Dependencies, "ms", "^2.1.3");
        manifest.set_dependency(DependencySection::DevDependencies, "typescript", "^5.9.3");
        assert!(manifest.remove_dependency(DependencySection::Dependencies, "ms"));
        assert!(manifest.remove_dependency(DependencySection::DevDependencies, "typescript"));

        assert_eq!(manifest.render().unwrap(), ORIGINAL);
    }

    #[test]
    fn test_new_entries_are_inserted_alphabetically() {
        let mut manifest = manifest(ORIGINAL);

        manifest.set_dependency(DependencySection::Dependencies, "ms", "^2.1.3");
        manifest.set_dependency(DependencySection::Dependencies, "axios", "^1.0.0");
        manifest.set_dependency(DependencySection::Dependencies, "zzz", "^1.0.0");

        let dependencies: Vec<&String> = manifest.root["dependencies"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(dependencies, ["axios", "ms", "react", "zod", "zzz"]);
    }

    #[test]
    fn test_updating_an_entry_keeps_its_position() {
        let mut manifest = manifest(ORIGINAL);

        manifest.set_dependency(DependencySection::Dependencies, "react", "^18.3.1");

        assert_eq!(
            manifest.render().unwrap(),
            ORIGINAL.replace("^19.0.0", "^18.3.1")
        );
    }

    #[test]
    fn test_new_section_goes_last_with_detected_indent() {
        let mut manifest = manifest("{\n    \"name\": \"app\"\n}");

        manifest.set_dependency(DependencySection::DevDependencies, "ms", "^2.1.3");

        assert_eq!(
            manifest.render().unwrap(),
            "{\n    \"name\": \"app\",\n    \"devDependencies\": {\n        \"ms\": \"^2.1.3\"\n    }\n}"
        );
    }

    #[test]
    fn test_minified_manifest_gets_default_indent() {
        let mut manifest = manifest("{\"name\":\"app\"}");

        manifest.set_dependency(DependencySection::Dependencies, "ms", "^2.1.3");

        assert_eq!(
            manifest.render().unwrap(),
            "{\n  \"name\": \"app\",\n  \"dependencies\": {\n    \"ms\": \"^2.1.3\"\n  }\n}"
        );
    }

    #[test]
    fn test_removing_an_unlisted_dependency_changes_nothing() {
        let mut manifest = manifest(ORIGINAL);

        assert!(!manifest.remove_dependency(DependencySection::Dependencies, "ms"));
        assert!(!manifest.remove_dependency(DependencySection::DevDependencies, "ms"));
        assert_eq!(manifest.render().unwrap(), ORIGINAL);
    }

    #[test]
    fn test_byte_order_mark_is_dropped() {
        let manifest = manifest("\u{feff}{\n  \"name\": \"app\"\n}\n");
        assert_eq!(manifest.render().unwrap(), "{\n  \"name\": \"app\"\n}\n");
    }

    #[test]
    fn test_non_object_manifest_is_an_error() {
        let error = ProjectManifest::parse(Path::new("package.json"), "[]").unwrap_err();
        assert_eq!(error.to_string(), "package.json must contain a JSON object");
    }
}
