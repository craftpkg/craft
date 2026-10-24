use contract::{Actor, Context, Pipeline};
use package::{DependencySection, InstallPackage, ProjectManifest};
use pipeline::{InstallPipe, LinkerPipe};
use resolver::{Edge, ResolvedArtifact};

#[derive(Debug)]
pub struct AddActorPayload {
    pub packages: Vec<String>,
    pub is_dev: bool,
    pub is_exact: bool,
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

    async fn run(&self) -> contract::Result<()> {
        let mut manifest = ProjectManifest::open(&self.context.package_json_path()).await?;

        let packages = self
            .payload
            .packages
            .iter()
            .map(|literal| InstallPackage::from_literal(literal, self.payload.is_dev))
            .collect();
        let graph = InstallPipe::new(&self.context, packages).run().await?;
        LinkerPipe::new(&self.context, graph.clone()).run().await?;

        let section = if self.payload.is_dev {
            DependencySection::DevDependencies
        } else {
            DependencySection::Dependencies
        };
        for root in &graph.roots {
            let spec = saved_spec(root, &graph.target(root)?.artifact, self.payload.is_exact);
            debug::info!("Saving {}@{}", root.name, spec);
            save_dependency(&mut manifest, section, &root.name, &spec);
        }

        manifest.save().await?;
        debug::info!("Updated package.json");

        Ok(())
    }
}

/// The spec written to package.json, as npm saves it: a caret range on the resolved version (or
/// the exact version with `--exact`), kept behind the `npm:` prefix for an alias.
fn saved_spec(root: &Edge, artifact: &ResolvedArtifact, is_exact: bool) -> String {
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
    for other_section in DependencySection::ALL {
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
        };

        assert_eq!(format!("{}", payload), "react lodash");
    }

    #[test]
    fn test_add_actor_creation() {
        let payload = AddActorPayload {
            packages: vec!["test".to_string()],
            is_dev: true,
            is_exact: false,
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
            download_url: String::new(),
            integrity: None,
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
