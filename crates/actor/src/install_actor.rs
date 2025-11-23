use contract::{Actor, Pipeline, Result};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};
use tokio::fs;

pub struct InstallActor;

impl Actor<()> for InstallActor {
    fn with(_: ()) -> Self {
        Self
    }

    async fn run(&self) -> Result<()> {
        // Check if package.json exists
        let package_json_path = std::env::current_dir()?.join("package.json");

        if !package_json_path.exists() {
            return Err(anyhow::anyhow!("no package.json found"));
        }

        // Read and parse package.json
        let content = fs::read_to_string(&package_json_path).await?;
        let package_json: PackageJson = serde_json::from_str(&content)?;

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
