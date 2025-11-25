use contract::{Actor, Pipeline, Result};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};

pub struct InstallActor;

impl Actor<()> for InstallActor {
    fn with(_: ()) -> Self {
        Self
    }

    async fn run(&self) -> Result<()> {
        let package_json = PackageJson::from_file().await?;

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
        let artifacts = InstallPipe::new(pkgs.clone()).run().await?;
        LinkerPipe::new(artifacts, pkgs).run().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_install_actor_no_package_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let actor = InstallActor::with(());
        let result = actor.run().await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "no package.json found");
    }

    #[test]
    fn test_install_actor_creation() {
        let actor = InstallActor::with(());
        // Just verify it can be created
        drop(actor);
    }
}
