use std::path::Path;

use contract::{Actor, Context, Result};
use package::{DependencySection, ProjectManifest};
use tokio::fs;

#[derive(Debug)]
pub struct RemoveActorPayload {
    pub packages: Vec<String>,
}

pub struct RemovePackageActor {
    context: Context,
    payload: RemoveActorPayload,
}

impl Actor<RemoveActorPayload> for RemovePackageActor {
    fn with(context: Context, payload: RemoveActorPayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> Result<()> {
        let node_modules = self.context.node_modules_dir();

        for package_name in &self.payload.packages {
            for bin_name in remove_bins(&node_modules, package_name).await? {
                debug::info!("Removed bin {} of {}", bin_name, package_name);
            }

            let package_path = node_modules.join(package_name);
            if fsutil::remove_entry(&package_path).await? {
                debug::info!("Removed {} from node_modules", package_name);
            } else {
                debug::warning!("Package {} not found in node_modules", package_name);
            }
        }

        let package_json_path = self.context.package_json_path();
        if !package_json_path.exists() {
            return Ok(());
        }

        let mut manifest = ProjectManifest::open(&package_json_path).await?;
        let mut is_changed = false;
        for package_name in &self.payload.packages {
            let mut was_listed = false;
            for section in DependencySection::ALL {
                if manifest.remove_dependency(section, package_name) {
                    debug::info!("Removed {} from {}", package_name, section.key());
                    was_listed = true;
                }
            }

            if !was_listed {
                eprintln!("warning: {package_name} is not a dependency of this project");
            }
            is_changed |= was_listed;
        }

        if is_changed {
            manifest.save().await?;
            debug::info!("Updated package.json");
        }

        Ok(())
    }
}

/// Removes the `.bin` links that point into the package, returning their names.
async fn remove_bins(node_modules: &Path, package_name: &str) -> Result<Vec<String>> {
    let bin_dir = node_modules.join(".bin");
    let package_dir = node_modules.join(package_name);
    let mut entries = match fs::read_dir(&bin_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut removed = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let Ok(target) = fs::read_link(entry.path()).await else {
            continue;
        };
        let is_package_bin =
            fsutil::normalize_lexically(&bin_dir.join(target)).starts_with(&package_dir);
        if is_package_bin {
            fsutil::remove_entry(&entry.path()).await?;
            removed.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_remove_package_actor_removes_from_node_modules() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create node_modules/test-package as a directory
        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir_all(&node_modules).await.unwrap();

        let test_package = node_modules.join("test-package");
        fs::create_dir(&test_package).await.unwrap();

        let payload = RemoveActorPayload {
            packages: vec!["test-package".to_string()],
        };

        let actor = RemovePackageActor::with(temp_context(&temp_dir), payload);
        let result = actor.run().await;

        assert!(result.is_ok());
        assert!(!test_package.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_removes_only_the_bins_of_the_removed_package() {
        let temp_dir = tempfile::tempdir().unwrap();
        let node_modules = temp_dir.path().join("node_modules");
        let bin_dir = node_modules.join(".bin");
        fs::create_dir_all(&bin_dir).await.unwrap();
        fs::symlink("../cowsay/cli.js", bin_dir.join("cowsay"))
            .await
            .unwrap();
        fs::symlink(
            node_modules.join("@scope/cow/say.js"),
            bin_dir.join("cowthink"),
        )
        .await
        .unwrap();
        fs::symlink("../cowsay-extra/cli.js", bin_dir.join("extra"))
            .await
            .unwrap();
        fs::symlink("../@scope/cow/say.js", bin_dir.join("scoped"))
            .await
            .unwrap();

        let mut removed = remove_bins(&node_modules, "cowsay").await.unwrap();
        removed.extend(remove_bins(&node_modules, "@scope/cow").await.unwrap());
        removed.sort();

        assert_eq!(removed, ["cowsay", "cowthink", "scoped"]);
        assert!(fs::symlink_metadata(bin_dir.join("extra")).await.is_ok());
    }

    #[tokio::test]
    async fn test_remove_package_actor_updates_package_json() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create package.json with dependencies
        let package_json = r#"{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "react": "^19.0.0",
    "lodash": "^4.17.21"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  }
}"#;
        fs::write(temp_dir.path().join("package.json"), package_json)
            .await
            .unwrap();

        let payload = RemoveActorPayload {
            packages: vec!["react".to_string(), "typescript".to_string()],
        };

        let actor = RemovePackageActor::with(temp_context(&temp_dir), payload);
        let result = actor.run().await;

        assert!(result.is_ok());

        // Read updated package.json
        let content = fs::read_to_string(temp_dir.path().join("package.json"))
            .await
            .unwrap();
        let package_json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify react was removed from dependencies
        let deps = package_json["dependencies"].as_object().unwrap();
        assert!(!deps.contains_key("react"));
        assert!(deps.contains_key("lodash"));

        // Verify the emptied devDependencies section was removed
        assert!(package_json.get("devDependencies").is_none());
    }

    #[tokio::test]
    async fn test_remove_package_actor_handles_missing_package() {
        let temp_dir = tempfile::tempdir().unwrap();

        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir_all(&node_modules).await.unwrap();

        let payload = RemoveActorPayload {
            packages: vec!["nonexistent-package".to_string()],
        };

        let actor = RemovePackageActor::with(temp_context(&temp_dir), payload);
        let result = actor.run().await;

        // Should succeed even if package doesn't exist
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_actor_payload_creation() {
        let payload = RemoveActorPayload {
            packages: vec!["test".to_string()],
        };

        assert_eq!(payload.packages.len(), 1);
    }

    fn temp_context(temp_dir: &tempfile::TempDir) -> Context {
        Context::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().join(".craft"),
            false,
        )
    }
}
