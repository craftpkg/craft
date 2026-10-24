use std::collections::{BTreeMap, HashMap};

use contract::{Context, Result};
use lockfile::{LockedDependency, LockedSource, Lockfile};
use package::{DependencySection, InstallPackage, PackageJson};
use pipeline::{INSTALLED_STATE, ScriptPolicy, VIRTUAL_STORE_DIR, lockfile_from_graph};
use resolver::ResolutionGraph;

use crate::install_actor::manifest_packages;

/// Besides the lockfile, what decides whether `node_modules` is current: which install scripts
/// may run, and whether dev dependencies were left out.
pub(crate) struct InstallMode<'a> {
    pub policy: &'a ScriptPolicy,
    pub production: bool,
}

/// Records `graph` in craft.lock and marks `node_modules` as matching it.
pub(crate) async fn save_lockfile(
    context: &Context,
    graph: &ResolutionGraph,
    specifiers: &HashMap<String, String>,
    mode: &InstallMode<'_>,
) -> Result<Lockfile> {
    let lockfile = lockfile_from_graph(graph, specifiers, &context.cwd);
    lockfile.save(&context.cwd).await?;
    mark_installed(context, &lockfile, mode).await?;
    Ok(lockfile)
}

pub(crate) async fn mark_installed(
    context: &Context,
    lockfile: &Lockfile,
    mode: &InstallMode<'_>,
) -> Result<()> {
    let marker = installed_state_path(context);
    fsutil::write_if_changed(&marker, installed_state(lockfile, mode)?.as_bytes()).await?;
    Ok(())
}

/// The lockfile plus the install mode: changing any of them means the next install has work.
fn installed_state(lockfile: &Lockfile, mode: &InstallMode<'_>) -> Result<String> {
    let omitted = if mode.production { "omit: dev\n" } else { "" };
    Ok(format!(
        "{}{}\n{omitted}",
        lockfile.render()?,
        mode.policy.describe()
    ))
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
    mode: &InstallMode<'_>,
) -> bool {
    let Some(lockfile) = lockfile else {
        return false;
    };
    let matches_manifest = lock_differences(lockfile, package_json).is_empty();
    let has_directories = lockfile
        .packages
        .values()
        .any(|package| matches!(package.resolution, LockedSource::Directory { .. }));
    let may_skip_install = matches_manifest && !has_directories;
    if !may_skip_install {
        return false;
    }

    let marker = installed_state_path(context);
    let installed = tokio::fs::read_to_string(&marker).await.ok();
    let current = installed_state(lockfile, mode).ok();
    installed.is_some() && installed == current
}

/// How package.json and craft.lock's project entry disagree, one phrase per dependency.
pub(crate) fn lock_differences(lockfile: &Lockfile, package_json: &PackageJson) -> Vec<String> {
    let packages = manifest_packages(package_json);
    let mut differences: Vec<String> = packages
        .iter()
        .filter(|package| locked_entry(Some(lockfile), package).is_none())
        .map(|package| {
            let specifier = package.version.as_deref().unwrap_or_default();
            let locked = locked_section(lockfile, package.section())
                .and_then(|section| section.get(&package.name));
            match locked {
                Some(locked) => format!(
                    "{} ({specifier} in package.json, {} in craft.lock)",
                    package.name, locked.specifier
                ),
                None => format!("{} (not in craft.lock)", package.name),
            }
        })
        .collect();

    for section in DependencySection::INSTALLED {
        let Some(locked) = locked_section(lockfile, section) else {
            continue;
        };
        let unlisted = locked.keys().filter(|name| {
            !packages
                .iter()
                .any(|package| is_declared_in(package, name, section))
        });
        differences.extend(unlisted.map(|name| format!("{name} (not in package.json)")));
    }
    differences.sort();
    differences
}

fn is_declared_in(package: &InstallPackage, name: &str, section: DependencySection) -> bool {
    package.section() == section && package.name == name
}

fn locked_section(
    lockfile: &Lockfile,
    section: DependencySection,
) -> Option<&BTreeMap<String, LockedDependency>> {
    let root = lockfile.root()?;
    match section {
        DependencySection::Dependencies => Some(&root.dependencies),
        DependencySection::DevDependencies => Some(&root.dev_dependencies),
        DependencySection::OptionalDependencies => Some(&root.optional_dependencies),
        DependencySection::PeerDependencies => None,
    }
}

fn locked_entry<'a>(
    lockfile: Option<&'a Lockfile>,
    package: &InstallPackage,
) -> Option<&'a LockedDependency> {
    locked_section(lockfile?, package.section())?
        .get(&package.name)
        .filter(|locked| Some(&locked.specifier) == package.version.as_ref())
}

/// What `node_modules` was last laid out from, so an unchanged project can skip the install
/// entirely.
fn installed_state_path(context: &Context) -> std::path::PathBuf {
    context
        .node_modules_dir()
        .join(VIRTUAL_STORE_DIR)
        .join(INSTALLED_STATE)
}
