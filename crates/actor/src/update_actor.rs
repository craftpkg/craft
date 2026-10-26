use std::time::Instant;

use contract::{Actor, Context, Pipeline, Result};
use lockfile::Lockfile;
use package::{DependencySpec, InstallPackage, PackageJson, ProjectManifest, VersionRequirement};
use pipeline::{InstallPipe, LinkerPipe};
use resolver::{ArtifactSource, ResolvedArtifact};

use crate::install_actor::{manifest_packages, manifest_specifiers};
use crate::lock_state::{InstallMode, save_lockfile, with_lock_preference};
use crate::outdated_actor::with_requirement;
use crate::scripts::{run_install_scripts, script_policy};
use crate::summary::{report_changes, report_installed};

#[derive(Debug)]
pub struct UpdatePayload {
    /// The direct dependencies to update; all of them when empty.
    pub packages: Vec<String>,
    /// Move to the `latest` tag and rewrite the specifier, instead of staying within it.
    pub latest: bool,
}

/// Re-resolves dependencies without their locked versions: within their specifiers, or to
/// `latest` with the specifier rewritten in the same style.
pub struct UpdateActor {
    context: Context,
    payload: UpdatePayload,
}

impl Actor<UpdatePayload> for UpdateActor {
    fn with(context: Context, payload: UpdatePayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> Result<()> {
        let started = Instant::now();
        let package_json_path = self.context.package_json_path();
        let package_json = PackageJson::from_file(&package_json_path).await?;
        let mut manifest = ProjectManifest::open(&package_json_path).await?;
        let lockfile = Lockfile::load(&self.context.cwd).await?;
        let declared = manifest_packages(&package_json);

        let unknown: Vec<&String> = self
            .payload
            .packages
            .iter()
            .filter(|name| !declared.iter().any(|package| &package.name == *name))
            .collect();
        if let Some(name) = unknown.first() {
            anyhow::bail!("{name} is not a dependency of this project");
        }

        let packages: Vec<InstallPackage> = declared
            .iter()
            .cloned()
            .map(|package| self.request(package, lockfile.as_ref()))
            .collect();
        let updates_everything = self.payload.packages.is_empty();
        let preferred = lockfile.as_ref().filter(|_| !updates_everything);
        let graph = InstallPipe::new(&self.context, packages)
            .with_lockfile(preferred)
            .into_graph()
            .await?;
        LinkerPipe::new(&self.context, graph.clone()).run().await?;
        let policy = script_policy(&package_json, false);
        run_install_scripts(&self.context, graph.clone(), &package_json, &policy, false).await?;

        let mut specifiers = manifest_specifiers(&package_json);
        if self.payload.latest {
            for package in declared
                .iter()
                .filter(|package| self.is_target(&package.name))
            {
                let Some(root) = graph.roots.iter().find(|root| root.name == package.name) else {
                    continue;
                };
                let Some(old_spec) = &package.version else {
                    continue;
                };
                let artifact = &graph.target(root)?.artifact;
                let Some(spec) = rewritten_spec(&package.name, old_spec, artifact) else {
                    continue;
                };
                manifest.set_dependency(package.section(), &package.name, &spec);
                specifiers.insert(package.name.clone(), spec);
            }
            manifest.save().await?;
        }

        let mode = InstallMode {
            policy: &policy,
            production: false,
        };
        let installed_lockfile = save_lockfile(&self.context, &graph, &specifiers, &mode).await?;
        report_changes(&self.context, lockfile.as_ref(), &installed_lockfile);
        report_installed(&self.context, &graph, started);
        Ok(())
    }
}

impl UpdateActor {
    fn is_target(&self, name: &str) -> bool {
        self.payload.packages.is_empty()
            || self.payload.packages.iter().any(|target| target == name)
    }

    /// Targets resolve without the lockfile (to `latest` with `--latest`); the rest keep their
    /// locked versions.
    fn request(&self, package: InstallPackage, lockfile: Option<&Lockfile>) -> InstallPackage {
        if !self.is_target(&package.name) {
            return with_lock_preference(lockfile, package);
        }
        let is_registry_dependency =
            DependencySpec::parse(&package.name, package.version.as_deref())
                .is_ok_and(|spec| spec.registry_target().is_some());
        let moves_to_latest = self.payload.latest && is_registry_dependency;
        let spec = match (&package.version, moves_to_latest) {
            (Some(spec), true) => Some(with_requirement(spec, "latest")),
            (spec, _) => spec.clone(),
        };
        InstallPackage {
            version: spec,
            ..package
        }
        .without_lock()
    }
}

/// The new specifier for a dependency moved to `artifact`, in the old one's style: exact stays
/// exact, `~` stays `~`, anything else becomes `^`; aliases keep their `npm:` target.
fn rewritten_spec(name: &str, old_spec: &str, artifact: &ResolvedArtifact) -> Option<String> {
    let is_registry_package = matches!(artifact.source, ArtifactSource::Registry { .. });
    if !is_registry_package {
        return None;
    }
    let requirement = DependencySpec::parse(name, Some(old_spec))
        .ok()?
        .registry_target()
        .map(|(_, requirement)| requirement.clone())?;
    let range_text = old_spec.rsplit('@').next().unwrap_or(old_spec);
    let version = &artifact.version;
    let range = match requirement {
        VersionRequirement::Exact(_) => version.clone(),
        _ if range_text.trim_start().starts_with('~') => format!("~{version}"),
        _ => format!("^{version}"),
    };
    Some(with_requirement(old_spec, &range))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str, version: &str) -> ResolvedArtifact {
        ResolvedArtifact {
            name: name.to_string(),
            version: version.to_string(),
            source: ArtifactSource::Registry {
                tarball_url: String::new(),
            },
            integrity: None,
            package: None,
            platform: Default::default(),
        }
    }

    #[test]
    fn test_rewritten_specs_keep_their_style() {
        let react = artifact("react", "19.2.0");
        assert_eq!(
            rewritten_spec("react", "^18.0.0", &react).as_deref(),
            Some("^19.2.0")
        );
        assert_eq!(
            rewritten_spec("react", "~18.3.0", &react).as_deref(),
            Some("~19.2.0")
        );
        assert_eq!(
            rewritten_spec("react", "18.3.1", &react).as_deref(),
            Some("19.2.0")
        );
        assert_eq!(
            rewritten_spec("react", ">=18 <19", &react).as_deref(),
            Some("^19.2.0")
        );

        let width = artifact("string-width", "7.2.0");
        assert_eq!(
            rewritten_spec("sw", "npm:string-width@^4.2.0", &width).as_deref(),
            Some("npm:string-width@^7.2.0")
        );
    }
}
