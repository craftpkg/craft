use contract::{Actor, Context, Pipeline};
use lockfile::Lockfile;
use package::{DependencySection, DependencySpec, InstallPackage, PackageJson, ProjectManifest};
use pipeline::{InstallPipe, LinkerPipe};
use resolver::{ArtifactSource, Edge, ResolvedArtifact};

use crate::install_actor::{manifest_packages, manifest_specifiers};
use crate::lock_state::{InstallMode, save_lockfile, with_lock_preference};
use crate::scripts::{run_install_scripts, script_policy};
use crate::summary::{report_changes, report_installed};
use std::time::Instant;

#[derive(Debug)]
pub struct AddActorPayload {
    pub packages: Vec<String>,
    pub is_dev: bool,
    pub is_exact: bool,
    pub is_optional: bool,
    /// Saved to `peerDependencies`, and to `devDependencies` so it is installed for development.
    pub is_peer: bool,
    pub ignore_scripts: bool,
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

    /// Installs the added packages together with everything the project already depends on,
    /// so the result is a complete install that pruning can trust.
    async fn run(&self) -> contract::Result<()> {
        let started = Instant::now();
        let package_json_path = self.context.package_json_path();
        let mut manifest = ProjectManifest::open(&package_json_path).await?;
        let package_json = PackageJson::from_file(&package_json_path).await?;

        let lockfile = Lockfile::load(&self.context.cwd).await?;
        let added: Vec<InstallPackage> = self
            .payload
            .packages
            .iter()
            .map(|literal| self.added_package(literal))
            .collect();
        let existing: Vec<InstallPackage> = manifest_packages(&package_json)
            .into_iter()
            .filter(|package| !added.iter().any(|added| added.name == package.name))
            .map(|package| with_lock_preference(lockfile.as_ref(), package))
            .collect();

        let mut graph = InstallPipe::new(&self.context, [existing, added.clone()].concat())
            .with_lockfile(lockfile.as_ref())
            .into_graph()
            .await?;
        let added_roots: Vec<Edge> = added
            .iter()
            .filter_map(|package| root_for(&graph.roots, package).cloned())
            .collect();
        graph.roots.retain(|root| {
            let is_replaced = added_roots.iter().any(|added| added.name == root.name);
            !is_replaced
        });
        graph.roots.extend(added_roots.iter().cloned());
        LinkerPipe::new(&self.context, graph.clone()).run().await?;
        let policy = script_policy(&package_json, self.payload.ignore_scripts);
        run_install_scripts(&self.context, graph.clone(), &package_json, &policy, false).await?;

        let mut specifiers = manifest_specifiers(&package_json);
        for root in &added_roots {
            let spec = saved_spec(root, &graph.target(root)?.artifact, self.payload.is_exact);
            debug::info!("Saving {}@{}", root.name, spec);
            save_dependency(&mut manifest, self.section(), &root.name, &spec);
            if self.payload.is_peer {
                manifest.set_dependency(DependencySection::PeerDependencies, &root.name, &spec);
            }
            specifiers.insert(root.name.clone(), spec);
        }

        manifest.save().await?;
        debug::info!("Updated package.json");
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

impl AddPackageActor {
    fn added_package(&self, literal: &str) -> InstallPackage {
        let installs_for_development = self.payload.is_dev || self.payload.is_peer;
        let package =
            InstallPackage::from_literal(literal, installs_for_development).without_lock();
        if self.payload.is_optional {
            package.optional()
        } else {
            package
        }
    }

    fn section(&self) -> DependencySection {
        if self.payload.is_optional {
            DependencySection::OptionalDependencies
        } else if self.payload.is_dev || self.payload.is_peer {
            DependencySection::DevDependencies
        } else {
            DependencySection::Dependencies
        }
    }
}

/// The root edge an added package resolved to: matched by name and spec, or for a bare
/// location such as `./libs/x` by the location it is saved under, since its name comes from
/// the package found there.
fn root_for<'g>(roots: &'g [Edge], package: &InstallPackage) -> Option<&'g Edge> {
    let location = package
        .version
        .is_none()
        .then(|| DependencySpec::parse(&package.name, None).ok())
        .flatten()
        .and_then(|spec| spec.location_spec());
    roots.iter().find(|root| match &location {
        Some(location) => root.spec.as_ref() == Some(location),
        None => root.name == package.name && root.spec == package.version,
    })
}

/// The spec written to package.json, as npm saves it: a caret range on the resolved version (or
/// the exact version with `--exact`), kept behind the `npm:` prefix for an alias. Local and URL
/// dependencies keep their location.
fn saved_spec(root: &Edge, artifact: &ResolvedArtifact, is_exact: bool) -> String {
    let is_registry_package = matches!(artifact.source, ArtifactSource::Registry { .. });
    if !is_registry_package {
        return root
            .spec
            .clone()
            .unwrap_or_else(|| artifact.version.clone());
    }

    let range = if is_exact {
        artifact.version.clone()
    } else {
        format!("^{}", artifact.version)
    };

    let is_alias = root.name != artifact.name;
    if is_alias {
        format!("npm:{}@{range}", artifact.name)
    } else {
        range
    }
}

/// Saves into `section` and drops the entry from every other one, so adding with or without
/// `-D` moves an existing dependency rather than listing it twice.
fn save_dependency(
    manifest: &mut ProjectManifest,
    section: DependencySection,
    name: &str,
    spec: &str,
) {
    for other_section in DependencySection::INSTALLED {
        if other_section != section {
            manifest.remove_dependency(other_section, name);
        }
    }
    manifest.set_dependency(section, name, spec);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_actor_payload_display() {
        let payload = AddActorPayload {
            packages: vec!["react".to_string(), "lodash".to_string()],
            is_dev: false,
            is_exact: false,
            is_optional: false,
            is_peer: false,
            ignore_scripts: false,
        };

        assert_eq!(format!("{}", payload), "react lodash");
    }

    #[test]
    fn test_add_actor_creation() {
        let payload = AddActorPayload {
            packages: vec!["test".to_string()],
            is_dev: true,
            is_exact: false,
            is_optional: false,
            is_peer: false,
            ignore_scripts: false,
        };

        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = AddPackageActor::with(context, payload);
        assert_eq!(actor.payload.packages.len(), 1);
        assert!(actor.payload.is_dev);
    }

    #[test]
    fn test_saved_spec_follows_npm_save_rules() {
        let ms = artifact("ms", "2.1.3");
        assert_eq!(saved_spec(&root("ms", &ms), &ms, false), "^2.1.3");
        assert_eq!(saved_spec(&root("ms", &ms), &ms, true), "2.1.3");

        let string_width = artifact("string-width", "4.2.3");
        assert_eq!(
            saved_spec(&root("sw", &string_width), &string_width, false),
            "npm:string-width@^4.2.3"
        );
        assert_eq!(
            saved_spec(&root("sw", &string_width), &string_width, true),
            "npm:string-width@4.2.3"
        );
    }

    #[test]
    fn test_save_dependency_moves_between_sections() {
        let content =
            "{\n  \"dependencies\": {\n    \"ms\": \"^2.0.0\",\n    \"zod\": \"^3.0.0\"\n  }\n}\n";
        let mut manifest = ProjectManifest::parse("package.json".as_ref(), content).unwrap();

        save_dependency(
            &mut manifest,
            DependencySection::DevDependencies,
            "ms",
            "^2.1.3",
        );

        assert_eq!(
            manifest.render().unwrap(),
            "{\n  \"dependencies\": {\n    \"zod\": \"^3.0.0\"\n  },\n  \"devDependencies\": {\n    \"ms\": \"^2.1.3\"\n  }\n}\n"
        );
    }

    fn artifact(name: &str, version: &str) -> ResolvedArtifact {
        ResolvedArtifact {
            name: name.to_string(),
            version: version.to_string(),
            source: ArtifactSource::Registry {
                tarball_url: String::new(),
            },
            integrity: None,
            platform: Default::default(),
            package: None,
        }
    }

    fn root(name: &str, artifact: &ResolvedArtifact) -> Edge {
        Edge {
            name: name.to_string(),
            spec: None,
            target: resolver::NodeId::new(&artifact.name, &artifact.version),
            kind: resolver::DependencyKind::Production,
        }
    }
}
