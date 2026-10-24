use contract::{Actor, Context, PackageError, Result};
use serde_json::json;

pub struct InitActor {
    context: Context,
}

impl Actor<()> for InitActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    /// Writes a minimal package.json named after the directory, never overwriting one.
    async fn run(&self) -> Result<()> {
        let path = self.context.package_json_path();
        if path.exists() {
            return Err(PackageError::AlreadyExists.into());
        }

        let directory_name = self
            .context
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let manifest = json!({
            "name": package_name(&directory_name),
            "version": "1.0.0",
        });
        let mut content = serde_json::to_string_pretty(&manifest)?;
        content.push('\n');
        tokio::fs::write(&path, content).await?;

        println!("Created package.json");
        Ok(())
    }
}

/// npm names are lowercase, URL-safe and may not start with `.` or `_`.
fn package_name(directory_name: &str) -> String {
    let name: String = directory_name
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' => character,
            _ => '-',
        })
        .collect();
    let name = name
        .trim_start_matches(['.', '_', '-'])
        .trim_end_matches('-');
    if name.is_empty() {
        "app".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_name_from_directory() {
        assert_eq!(package_name("my-app"), "my-app");
        assert_eq!(package_name("My App"), "my-app");
        assert_eq!(package_name(".hidden_project"), "hidden_project");
        assert_eq!(package_name("café"), "caf");
        assert_eq!(package_name(""), "app");
    }

    #[tokio::test]
    async fn test_creates_package_json_once() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Shop Front");
        std::fs::create_dir(&project).unwrap();
        let context = Context::new(project.clone(), dir.path().join(".craft"), false);

        InitActor::with(context.clone(), ()).run().await.unwrap();
        let error = InitActor::with(context, ()).run().await.unwrap_err();

        let content = std::fs::read_to_string(project.join("package.json")).unwrap();
        assert_eq!(
            content,
            "{\n  \"name\": \"shop-front\",\n  \"version\": \"1.0.0\"\n}\n"
        );
        assert_eq!(error.to_string(), "package.json already exists");
    }
}
