use contract::{Actor, Context, Pipeline, Result};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};

pub struct InstallActor {
    context: Context,
}

impl Actor<()> for InstallActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    async fn run(&self) -> Result<()> {
        let package_json = PackageJson::from_file(&self.context.package_json_path()).await?;

        // Collect all dependencies
        let mut pkgs = Vec::new();

        if let Some(deps) = package_json.dependencies {
            for (name, version) in deps {
                pkgs.push(InstallPackage::new(name, Some(version), false));
            }
        }

        if let Some(dev_deps) = package_json.dev_dependencies {
            for (name, version) in dev_deps {
                pkgs.push(InstallPackage::new(name, Some(version), true));
            }
        }

        debug::trace!("Installing packages from package.json: {pkgs:?}");

        // Run install and link pipes
        let graph = InstallPipe::new(&self.context, pkgs).run().await?;
        LinkerPipe::new(&self.context, graph).run().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_install_actor_no_package_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let context = Context::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().join(".craft"),
            false,
        );

        let actor = InstallActor::with(context, ());
        let result = actor.run().await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "no package.json found");
    }

    #[test]
    fn test_install_actor_creation() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = InstallActor::with(context, ());
        // Just verify it can be created
        drop(actor);
    }
}
