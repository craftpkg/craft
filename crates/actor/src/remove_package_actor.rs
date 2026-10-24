use contract::{Actor, Context, Result};
use package::{DependencySection, ProjectManifest};

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

        // Remove packages from node_modules
        for package_name in &self.payload.packages {
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
            for section in DependencySection::ALL {
                if manifest.remove_dependency(section, package_name) {
                    debug::info!("Removed {} from {}", package_name, section.key());
                    is_changed = true;
                }
            }
        }

        if is_changed {
            manifest.save().await?;
            debug::info!("Updated package.json");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

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
