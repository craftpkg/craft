use std::collections::{BTreeMap, HashSet};

use contract::{Actor, Context, LockfileError, Result};
use lockfile::{LockedDependency, LockedPackage, Lockfile};

/// How many dependency chains `craft why` prints per version before summarizing the rest.
const MAX_PATHS_PER_VERSION: usize = 10;

#[derive(Debug)]
pub enum InspectPayload {
    /// `craft list`: the project's dependencies, `depth` levels deep.
    List { depth: usize },
    /// `craft why <name>`: every chain of dependencies that installs `name`.
    Why { name: String },
}

/// Answers questions about the installed tree from craft.lock alone, without the network.
pub struct InspectActor {
    context: Context,
    payload: InspectPayload,
}

impl Actor<InspectPayload> for InspectActor {
    fn with(context: Context, payload: InspectPayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> Result<()> {
        let lockfile = Lockfile::load(&self.context.cwd)
            .await?
            .ok_or(LockfileError::Missing)?;
        let lines = match &self.payload {
            InspectPayload::List { depth } => list(&lockfile, *depth),
            InspectPayload::Why { name } => why(&lockfile, name),
        };
        for line in lines {
            println!("{line}");
        }
        Ok(())
    }
}

/// The project's dependencies by section, each followed by its own dependencies down to
/// `depth`, drawn as a tree.
fn list(lockfile: &Lockfile, depth: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (section, roots) in sections(lockfile) {
        if roots.is_empty() {
            continue;
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("{section}:"));
        for (name, key) in roots {
            lines.push(describe(lockfile, name, key));
            let mut ancestors = vec![key.as_str()];
            tree(lockfile, key, depth, "", &mut ancestors, &mut lines);
        }
    }
    if lines.is_empty() {
        lines.push("No dependencies".to_string());
    }
    lines
}

fn tree<'a>(
    lockfile: &'a Lockfile,
    key: &str,
    depth: usize,
    indent: &str,
    ancestors: &mut Vec<&'a str>,
    lines: &mut Vec<String>,
) {
    let Some(package) = lockfile.packages.get(key) else {
        return;
    };
    if depth == 0 {
        return;
    }
    let children = children(package);
    for (index, (name, child_key, label)) in children.iter().enumerate() {
        let is_last = index + 1 == children.len();
        let (branch, next_indent) = if is_last {
            ("└── ", format!("{indent}    "))
        } else {
            ("├── ", format!("{indent}│   "))
        };
        let is_cycle = ancestors.contains(&child_key.as_str());
        let suffix = match (label, is_cycle) {
            (_, true) => " (cycle)".to_string(),
            (Some(label), false) => format!(" ({label})"),
            (None, false) => String::new(),
        };
        lines.push(format!(
            "{indent}{branch}{}{suffix}",
            describe(lockfile, name, child_key)
        ));
        if !is_cycle {
            ancestors.push(child_key.as_str());
            tree(
                lockfile,
                child_key,
                depth - 1,
                &next_indent,
                ancestors,
                lines,
            );
            ancestors.pop();
        }
    }
}

/// Every chain from the project to a package called `name`, grouped by installed version.
fn why(lockfile: &Lockfile, name: &str) -> Vec<String> {
    let mut search = PathSearch {
        lockfile,
        target: name,
        paths: BTreeMap::new(),
    };
    for (section, roots) in sections(lockfile) {
        for (root_name, key) in roots {
            let mut chain = vec![describe(lockfile, root_name, key)];
            let mut visited = HashSet::from([key.as_str()]);
            search.visit(section, root_name, key, &mut chain, &mut visited);
        }
    }
    let paths = search.paths;

    if paths.is_empty() {
        return vec![format!("{name} is not installed")];
    }
    let mut lines = Vec::new();
    for (version, chains) in paths {
        lines.push(format!("{name} {version}"));
        for chain in chains.iter().take(MAX_PATHS_PER_VERSION) {
            lines.push(format!("  {chain}"));
        }
        let hidden = chains.len().saturating_sub(MAX_PATHS_PER_VERSION);
        if hidden > 0 {
            lines.push(format!("  … and {hidden} more"));
        }
    }
    lines
}

/// Depth-first search for chains ending at `target`, collected by the version reached.
struct PathSearch<'a> {
    lockfile: &'a Lockfile,
    target: &'a str,
    paths: BTreeMap<String, Vec<String>>,
}

impl<'a> PathSearch<'a> {
    fn visit(
        &mut self,
        section: &str,
        edge_name: &str,
        key: &'a str,
        chain: &mut Vec<String>,
        visited: &mut HashSet<&'a str>,
    ) {
        let Some(package) = self.lockfile.packages.get(key) else {
            return;
        };
        let is_target = edge_name == self.target || package.name == self.target;
        if is_target {
            let rendered = format!("{section}: {}", chain.join(" → "));
            self.paths
                .entry(package.version.clone())
                .or_default()
                .push(rendered);
            return;
        }

        for (name, child_key, _) in children(package) {
            if !visited.insert(child_key.as_str()) {
                continue;
            }
            chain.push(describe(self.lockfile, name, child_key));
            self.visit(section, name, child_key, chain, visited);
            chain.pop();
            visited.remove(child_key.as_str());
        }
    }
}

/// A package's dependencies, peers and optional dependencies, by name, with a label for the
/// non-regular kinds.
fn children(package: &LockedPackage) -> Vec<(&String, &String, Option<&'static str>)> {
    let mut children: Vec<_> = package
        .dependencies
        .iter()
        .map(|(name, key)| (name, key, None))
        .chain(
            package
                .peer_dependencies
                .iter()
                .map(|(name, key)| (name, key, Some("peer"))),
        )
        .chain(
            package
                .optional_dependencies
                .iter()
                .map(|(name, key)| (name, key, Some("optional"))),
        )
        .collect();
    children.sort_by(|a, b| a.0.cmp(b.0));
    children
}

fn sections(lockfile: &Lockfile) -> Vec<(&'static str, Vec<(&String, &String)>)> {
    let Some(root) = lockfile.root() else {
        return Vec::new();
    };
    vec![
        ("dependencies", entries(&root.dependencies)),
        ("devDependencies", entries(&root.dev_dependencies)),
        ("optionalDependencies", entries(&root.optional_dependencies)),
    ]
}

fn entries(dependencies: &BTreeMap<String, LockedDependency>) -> Vec<(&String, &String)> {
    dependencies
        .iter()
        .map(|(name, locked)| (name, &locked.version))
        .collect()
}

/// `name version`, with the package name when it is installed under an alias.
fn describe(lockfile: &Lockfile, name: &str, key: &str) -> String {
    match lockfile.packages.get(key) {
        Some(package) if package.name != name => {
            format!("{name} (npm:{}@{})", package.name, package.version)
        }
        Some(package) => format!("{name} {}", package.version),
        None => format!("{name} (missing from craft.lock)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lockfile::{Importer, LockedSource, ROOT_IMPORTER};

    fn lockfile() -> Lockfile {
        let registry = |name: &str| LockedSource::Registry {
            tarball: format!("https://registry.test/{name}.tgz"),
            integrity: None,
        };
        let mut lockfile = Lockfile::default();
        let mut debug = LockedPackage::new("debug", "4.4.0", registry("debug"));
        debug
            .dependencies
            .insert("ms".to_string(), "ms@2.1.3".to_string());
        let mut ui = LockedPackage::new("ui", "1.0.0", registry("ui"));
        ui.peer_dependencies
            .insert("react".to_string(), "react@19.2.0".to_string());
        ui.dependencies
            .insert("debug".to_string(), "debug@4.4.0".to_string());
        for (key, package) in [
            ("debug@4.4.0", debug),
            (
                "ms@2.1.3",
                LockedPackage::new("ms", "2.1.3", registry("ms")),
            ),
            (
                "ms@2.0.0",
                LockedPackage::new("ms", "2.0.0", registry("ms")),
            ),
            (
                "react@19.2.0",
                LockedPackage::new("react", "19.2.0", registry("react")),
            ),
            ("ui@1.0.0_react@19.2.0", ui),
        ] {
            lockfile.packages.insert(key.to_string(), package);
        }
        let locked = |specifier: &str, version: &str| LockedDependency {
            specifier: specifier.to_string(),
            version: version.to_string(),
        };
        lockfile.importers.insert(
            ROOT_IMPORTER.to_string(),
            Importer {
                dependencies: BTreeMap::from([
                    ("debug".to_string(), locked("^4.0.0", "debug@4.4.0")),
                    ("react".to_string(), locked("^19.0.0", "react@19.2.0")),
                    ("ui".to_string(), locked("^1.0.0", "ui@1.0.0_react@19.2.0")),
                ]),
                dev_dependencies: BTreeMap::from([("ms".to_string(), locked("2.0.0", "ms@2.0.0"))]),
                optional_dependencies: BTreeMap::new(),
            },
        );
        lockfile
    }

    #[test]
    fn test_lists_direct_dependencies_by_section() {
        assert_eq!(
            list(&lockfile(), 0),
            [
                "dependencies:",
                "debug 4.4.0",
                "react 19.2.0",
                "ui 1.0.0",
                "",
                "devDependencies:",
                "ms 2.0.0"
            ]
        );
    }

    #[test]
    fn test_lists_a_tree_to_the_requested_depth() {
        let lines = list(&lockfile(), 2);
        let ui = lines.iter().position(|line| line == "ui 1.0.0").unwrap();
        assert_eq!(
            lines[ui + 1..ui + 4],
            [
                "├── debug 4.4.0",
                "│   └── ms 2.1.3",
                "└── react 19.2.0 (peer)"
            ]
        );
    }

    #[test]
    fn test_explains_every_chain_to_a_package() {
        assert_eq!(
            why(&lockfile(), "ms"),
            [
                "ms 2.0.0",
                "  devDependencies: ms 2.0.0",
                "ms 2.1.3",
                "  dependencies: debug 4.4.0 → ms 2.1.3",
                "  dependencies: ui 1.0.0 → debug 4.4.0 → ms 2.1.3",
            ]
        );
        assert_eq!(why(&lockfile(), "lodash"), ["lodash is not installed"]);
    }
}
