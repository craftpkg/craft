use std::collections::HashMap;

use contract::{Actor, Context, LockfileError, Pipeline, Result};
use lockfile::Lockfile;
use package::{InstallPackage, PackageJson};
use pipeline::{InstallPipe, LinkerPipe};

use crate::lock_state::{
    is_up_to_date, lock_differences, mark_installed, save_lockfile, with_lock_preference,
};
use crate::scripts::{run_install_scripts, script_policy};

pub struct InstallActor {
    context: Context,
    options: InstallOptions,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InstallOptions {
    /// Install exactly what craft.lock records, failing if it is missing or out of date.
    pub frozen_lockfile: bool,
    /// Start from an empty `node_modules`, as `craft ci` does.
    pub clean: bool,
    /// Run no lifecycle scripts at all, the project's own included.
    pub ignore_scripts: bool,
}

impl Actor<InstallOptions> for InstallActor {
    fn with(context: Context, options: InstallOptions) -> Self {
        Self { context, options }
    }

    async fn run(&self) -> Result<()> {
        if self.options.clean {
            fsutil::remove_entry(&self.context.node_modules_dir()).await?;
        }
        let package_json = PackageJson::from_file(&self.context.package_json_path()).await?;
        let lockfile = Lockfile::load(&self.context.cwd).await?;
        let policy = script_policy(&package_json, self.options.ignore_scripts);
        let is_frozen = self.options.frozen_lockfile || self.context.ci;
        if is_frozen {
            ensure_lockfile_matches(lockfile.as_ref(), &package_json)?;
        }
        if is_up_to_date(&self.context, lockfile.as_ref(), &package_json, &policy).await {
            println!("Already up to date");
            return Ok(());
        }

        let packages: Vec<InstallPackage> = manifest_packages(&package_json)
            .into_iter()
            .map(|package| with_lock_preference(lockfile.as_ref(), package))
            .collect();
        debug::trace!("Installing packages from package.json: {packages:?}");

        let mut pipe = InstallPipe::new(&self.context, packages).with_lockfile(lockfile.as_ref());
        if is_frozen {
            pipe = pipe.frozen();
        }
        let graph = pipe.run().await?;
        LinkerPipe::new(&self.context, graph.clone()).run().await?;
        run_install_scripts(&self.context, graph.clone(), &package_json, &policy, true).await?;

        match lockfile.filter(|_| is_frozen) {
            Some(lockfile) => mark_installed(&self.context, &lockfile, &policy).await,
            None => {
                let specifiers = manifest_specifiers(&package_json);
                save_lockfile(&self.context, &graph, &specifiers, &policy).await
            }
        }
    }
}

/// A frozen install needs a lockfile that already records every dependency as package.json
/// declares it.
fn ensure_lockfile_matches(lockfile: Option<&Lockfile>, package_json: &PackageJson) -> Result<()> {
    let lockfile = lockfile.ok_or(LockfileError::Missing)?;
    let differences = lock_differences(lockfile, package_json);
    if differences.is_empty() {
        return Ok(());
    }
    Err(LockfileError::OutOfDate { differences }.into())
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

        let actor = InstallActor::with(context, InstallOptions::default());
        let result = actor.run().await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "no package.json found; run `craft init` to create one"
        );
    }

    #[test]
    fn test_install_actor_creation() {
        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = InstallActor::with(context, InstallOptions::default());
        // Just verify it can be created
        drop(actor);
    }
}
