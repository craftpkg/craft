use std::collections::HashMap;

use contract::{Actor, Context, Pipeline, Result};
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe, lockfile_from_graph};
use resolver::ResolutionGraph;

pub struct InstallActor {
    context: Context,
}

impl Actor<()> for InstallActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    async fn run(&self) -> Result<()> {
        let package_json = PackageJson::from_file(&self.context.package_json_path()).await?;
        let packages = manifest_packages(&package_json);
        debug::trace!("Installing packages from package.json: {packages:?}");

        let graph = InstallPipe::new(&self.context, packages).run().await?;
        LinkerPipe::new(&self.context, graph.clone()).run().await?;
        save_lockfile(&self.context, &graph, &manifest_specifiers(&package_json)).await
    }
}

/// Records `graph` in `craft.lock`, rewriting it only when something changed.
pub(crate) async fn save_lockfile(
    context: &Context,
    graph: &ResolutionGraph,
    specifiers: &HashMap<String, String>,
) -> Result<()> {
    lockfile_from_graph(graph, specifiers, &context.cwd)
        .save(&context.cwd)
        .await?;
    Ok(())
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
