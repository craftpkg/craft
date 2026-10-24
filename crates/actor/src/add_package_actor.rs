use contract::{Actor, Context, Pipeline};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};

#[derive(Debug)]
pub struct AddActorPayload {
    pub packages: Vec<String>,
    pub is_dev: bool,
}

impl std::fmt::Display for AddActorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.packages.join(" "))
    }
}

pub struct AddPackageActor {
    context: Context,
    payload: AddActorPayload,
}

impl Actor<AddActorPayload> for AddPackageActor {
    fn with(context: Context, payload: AddActorPayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> contract::Result<()> {
        let mut pkgs = Vec::new();

        for pkg in &self.payload.packages {
            pkgs.push(InstallPackage::from_literal(pkg, self.payload.is_dev));
        }

        let artifacts = InstallPipe::new(&self.context, pkgs.clone()).run().await?;

        LinkerPipe::new(&self.context, artifacts.clone(), pkgs)
            .run()
            .await?;

        let package_json_path = self.context.package_json_path();
        let mut package_json = PackageJson::from_file(&package_json_path).await?;

        // Add only the explicitly requested packages to dependencies or devDependencies
        for pkg_name in &self.payload.packages {
            // Find the corresponding artifact for this package
            if let Some(artifact) = artifacts.iter().find(|a| &a.name == pkg_name) {
                if self.payload.is_dev {
                    // Add to devDependencies
                    let dev_deps = package_json
                        .dev_dependencies
                        .get_or_insert_with(std::collections::HashMap::new);
                    dev_deps.insert(artifact.name.clone(), format!("^{}", artifact.version));
                    debug::info!("Added {} to devDependencies", artifact.name);
                } else {
                    // Add to dependencies
                    let deps = package_json
                        .dependencies
                        .get_or_insert_with(std::collections::HashMap::new);
                    deps.insert(artifact.name.clone(), format!("^{}", artifact.version));
                    debug::info!("Added {} to dependencies", artifact.name);
                }
            }
        }

        // Write updated package.json
        let updated_content = serde_json::to_string_pretty(&package_json)?;
        tokio::fs::write(&package_json_path, updated_content).await?;
        debug::info!("Updated package.json");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_actor_payload_display() {
        let payload = AddActorPayload {
            packages: vec!["react".to_string(), "lodash".to_string()],
            is_dev: false,
        };

        assert_eq!(format!("{}", payload), "react lodash");
    }

    #[test]
    fn test_add_actor_creation() {
        let payload = AddActorPayload {
            packages: vec!["test".to_string()],
            is_dev: true,
        };

        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = AddPackageActor::with(context, payload);
        assert_eq!(actor.payload.packages.len(), 1);
        assert!(actor.payload.is_dev);
    }
}
