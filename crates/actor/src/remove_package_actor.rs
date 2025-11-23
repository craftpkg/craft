use contract::{Actor, Result};
use package::PackageJson;
use tokio::fs;

#[derive(Debug)]
pub struct RemoveActorPayload {
    pub packages: Vec<String>,
}

pub struct RemovePackageActor {
    payload: RemoveActorPayload,
}

impl Actor<RemoveActorPayload> for RemovePackageActor {
    fn with(payload: RemoveActorPayload) -> Self {
        Self { payload }
    }

    async fn run(&self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let node_modules = cwd.join("node_modules");

        // Remove packages from node_modules
        for package_name in &self.payload.packages {
            let package_path = node_modules.join(package_name);

            if package_path.exists() {
                if package_path.is_symlink() {
                    fs::remove_file(&package_path).await?;
                } else {
                    fs::remove_dir_all(&package_path).await?;
                }
                debug::info!("Removed {} from node_modules", package_name);
            } else {
                debug::warning!("Package {} not found in node_modules", package_name);
            }
        }

        // Update package.json if it exists
        let package_json_path = cwd.join("package.json");

        if package_json_path.exists() {
            let content = fs::read_to_string(&package_json_path).await?;
            let mut package_json: PackageJson = serde_json::from_str(&content)?;

            // Remove from dependencies
            if let Some(deps) = &mut package_json.dependencies {
                for package_name in &self.payload.packages {
                    if deps.remove(package_name).is_some() {
                        debug::info!("Removed {} from dependencies", package_name);
                    }
                }
            }

            // Remove from devDependencies
            if let Some(dev_deps) = &mut package_json.dev_dependencies {
                for package_name in &self.payload.packages {
                    if dev_deps.remove(package_name).is_some() {
                        debug::info!("Removed {} from devDependencies", package_name);
                    }
                }
            }

            // Write updated package.json
            let updated_content = serde_json::to_string_pretty(&package_json)?;
            fs::write(&package_json_path, updated_content).await?;
            debug::info!("Updated package.json");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_remove_package_actor_removes_from_node_modules() {
        let temp_dir = tempfile::tempdir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        // Create node_modules/test-package as a directory
        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir_all(&node_modules).await.unwrap();

        let test_package = node_modules.join("test-package");
        fs::create_dir(&test_package).await.unwrap();

        let payload = RemoveActorPayload {
            packages: vec!["test-package".to_string()],
        };

        let actor = RemovePackageActor::with(payload);
        let result = actor.run().await;

        assert!(result.is_ok());
        assert!(!test_package.exists());
    }

    #[tokio::test]
    async fn test_remove_package_actor_updates_package_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

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

        let actor = RemovePackageActor::with(payload);
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

        // Verify typescript was removed from devDependencies
        let dev_deps = package_json["devDependencies"].as_object().unwrap();
        assert!(!dev_deps.contains_key("typescript"));
    }

    #[tokio::test]
    async fn test_remove_package_actor_handles_missing_package() {
        let temp_dir = tempfile::tempdir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir_all(&node_modules).await.unwrap();

        let payload = RemoveActorPayload {
            packages: vec!["nonexistent-package".to_string()],
        };

        let actor = RemovePackageActor::with(payload);
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
}
