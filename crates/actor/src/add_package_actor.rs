use contract::{Actor, Context, Pipeline};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};
use resolver::{Edge, ResolvedArtifact};

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

        let graph = InstallPipe::new(&self.context, pkgs).run().await?;

        LinkerPipe::new(&self.context, graph.clone()).run().await?;

        let package_json_path = self.context.package_json_path();
        let mut package_json = PackageJson::from_file(&package_json_path).await?;

        let section = if self.payload.is_dev {
            &mut package_json.dev_dependencies
        } else {
            &mut package_json.dependencies
        };
        let dependencies = section.get_or_insert_with(std::collections::HashMap::new);

        for root in &graph.roots {
            let saved_spec = saved_spec(root, &graph.target(root)?.artifact);
            debug::info!("Saving {}@{}", root.name, saved_spec);
            dependencies.insert(root.name.clone(), saved_spec);
        }

        // Write updated package.json
        let updated_content = serde_json::to_string_pretty(&package_json)?;
        tokio::fs::write(&package_json_path, updated_content).await?;
        debug::info!("Updated package.json");

        Ok(())
    }
}

/// The spec written to package.json: a caret range on the resolved version, kept behind the
/// `npm:` prefix when the package was added under an alias.
fn saved_spec(root: &Edge, artifact: &ResolvedArtifact) -> String {
    let is_alias = root.name != artifact.name;
    if is_alias {
        format!("npm:{}@^{}", artifact.name, artifact.version)
    } else {
        format!("^{}", artifact.version)
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
