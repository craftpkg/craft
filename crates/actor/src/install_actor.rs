use std::collections::HashMap;

use contract::{Actor, Context, Pipeline, Result};
use lockfile::Lockfile;
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};

use crate::lock_state::{is_up_to_date, save_lockfile, with_lock_preference};

pub struct InstallActor {
    context: Context,
}

impl Actor<()> for InstallActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    async fn run(&self) -> Result<()> {
        let package_json = PackageJson::from_file(&self.context.package_json_path()).await?;
        let lockfile = Lockfile::load(&self.context.cwd).await?;
        if is_up_to_date(&self.context, lockfile.as_ref(), &package_json).await {
            println!("Already up to date");
            return Ok(());
        }

        let packages: Vec<InstallPackage> = manifest_packages(&package_json)
            .into_iter()
            .map(|package| with_lock_preference(lockfile.as_ref(), package))
            .collect();
        debug::trace!("Installing packages from package.json: {packages:?}");

        let graph = InstallPipe::new(&self.context, packages)
            .with_lockfile(lockfile.as_ref())
            .run()
            .await?;
        LinkerPipe::new(&self.context, graph.clone()).run().await?;
        save_lockfile(&self.context, &graph, &manifest_specifiers(&package_json)).await
    }
}

/// Each declared dependency's spec as package.json lists it.
pub(crate) fn manifest_specifiers(package_json: &PackageJson) -> HashMap<String, String> {
    [&package_json.dependencies, &package_json.dev_dependencies]
        .into_iter()
        .flatten()
        .flat_map(|section| section.iter())
        .map(|(name, spec)| (name.clone(), spec.clone()))
        .collect()
}

/// Every dependency and dev dependency the manifest declares, sorted by name so installs are
/// deterministic.
pub(crate) fn manifest_packages(package_json: &PackageJson) -> Vec<InstallPackage> {
    let sections = [
        (&package_json.dependencies, false),
        (&package_json.dev_dependencies, true),
    ];
    let mut packages: Vec<InstallPackage> = sections
        .into_iter()
        .filter_map(|(section, is_dev)| Some((section.as_ref()?, is_dev)))
        .flat_map(|(section, is_dev)| {
            section.iter().map(move |(name, spec)| {
                InstallPackage::new(name.clone(), Some(spec.clone()), is_dev)
            })
        })
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
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
