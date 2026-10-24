use std::collections::HashMap;

use contract::{Context, Result};
use lockfile::{LockedDependency, LockedSource, Lockfile};
use package::{InstallPackage, PackageJson};
use pipeline::{INSTALLED_LOCKFILE, VIRTUAL_STORE_DIR, lockfile_from_graph};
use resolver::ResolutionGraph;

use crate::install_actor::manifest_packages;

/// Records `graph` in craft.lock and marks `node_modules` as matching it.
pub(crate) async fn save_lockfile(
    context: &Context,
    graph: &ResolutionGraph,
    specifiers: &HashMap<String, String>,
) -> Result<()> {
    let lockfile = lockfile_from_graph(graph, specifiers, &context.cwd);
    lockfile.save(&context.cwd).await?;
    mark_installed(context, &lockfile).await
}

pub(crate) async fn mark_installed(context: &Context, lockfile: &Lockfile) -> Result<()> {
    let marker = installed_lockfile_path(context);
    fsutil::write_if_changed(&marker, lockfile.render()?.as_bytes()).await?;
    Ok(())
}

/// Whether the lockfile may answer `package`: only while package.json still lists it with
/// the specifier that was locked. A changed specifier is resolved afresh, though its own
/// dependencies may still come from the lockfile.
pub(crate) fn with_lock_preference(
    lockfile: Option<&Lockfile>,
    package: InstallPackage,
) -> InstallPackage {
    if locked_entry(lockfile, &package).is_some() {
        package
    } else {
        package.without_lock()
    }
}

/// True when package.json, craft.lock and `node_modules` all agree, so there is nothing to do.
/// `file:` directories always reinstall, since their source may have changed.
pub(crate) async fn is_up_to_date(
    context: &Context,
    lockfile: Option<&Lockfile>,
    package_json: &PackageJson,
) -> bool {
    let Some(lockfile) = lockfile else {
        return false;
    };
    let packages = manifest_packages(package_json);
    let locked_count = lockfile.root().map_or(0, |root| {
        root.dependencies.len() + root.dev_dependencies.len()
    });
    let matches_manifest = packages.len() == locked_count
        && packages
            .iter()
            .all(|package| locked_entry(Some(lockfile), package).is_some());
    let has_directories = lockfile
        .packages
        .values()
        .any(|package| matches!(package.resolution, LockedSource::Directory { .. }));
    let may_skip_install = matches_manifest && !has_directories;
    if !may_skip_install {
        return false;
    }

    let marker = installed_lockfile_path(context);
    let installed = tokio::fs::read_to_string(&marker).await.ok();
    let current = lockfile.render().ok();
    installed.is_some() && installed == current
}

fn locked_entry<'a>(
    lockfile: Option<&'a Lockfile>,
    package: &InstallPackage,
) -> Option<&'a LockedDependency> {
    let root = lockfile?.root()?;
    let section = if package.is_dev {
        &root.dev_dependencies
    } else {
        &root.dependencies
    };
    section
        .get(&package.name)
        .filter(|locked| Some(&locked.specifier) == package.version.as_ref())
}

/// A copy of the lockfile `node_modules` was last laid out from, so an unchanged project can
/// skip the install entirely.
fn installed_lockfile_path(context: &Context) -> std::path::PathBuf {
    context
        .node_modules_dir()
        .join(VIRTUAL_STORE_DIR)
        .join(INSTALLED_LOCKFILE)
}
