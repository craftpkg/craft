use std::collections::BTreeMap;
use std::time::Instant;

use contract::Context;
use lockfile::Lockfile;
use resolver::ResolutionGraph;

/// Prints how the project's own dependencies changed between two lockfiles:
/// `+ react 19.2.0`, `- lodash 4.17.21`, `~ ms 2.1.3 → 2.1.4`.
pub(crate) fn report_changes(context: &Context, before: Option<&Lockfile>, after: &Lockfile) {
    context.reporter.finish();
    let before = before.map(direct_dependencies).unwrap_or_default();
    let after = direct_dependencies(after);
    let names: std::collections::BTreeSet<&String> = before.keys().chain(after.keys()).collect();

    for name in names {
        let line = match (before.get(name), after.get(name)) {
            (None, Some(added)) => format!("+ {name} {}", added.describe()),
            (Some(removed), None) => format!("- {name} {}", removed.version),
            (Some(old), Some(new)) if old.version != new.version => {
                format!("~ {name} {} → {}", old.version, new.version)
            }
            _ => continue,
        };
        context.reporter.line(line);
    }
}

/// Prints how many packages the install laid out and how long it took.
pub(crate) fn report_installed(context: &Context, graph: &ResolutionGraph, started: Instant) {
    let installed = graph
        .nodes
        .values()
        .filter(|node| node.artifact.platform.supports(&context.platform))
        .count();
    context.reporter.finish();
    context.reporter.line(format!(
        "{} installed in {:.1}s",
        packages(installed),
        started.elapsed().as_secs_f64()
    ));
}

/// `1 package`, `2 packages`.
pub(crate) fn packages(count: usize) -> String {
    if count == 1 {
        "1 package".to_string()
    } else {
        format!("{count} packages")
    }
}

struct DirectDependency {
    version: String,
    is_dev: bool,
}

impl DirectDependency {
    fn describe(&self) -> String {
        if self.is_dev {
            format!("{} (dev)", self.version)
        } else {
            self.version.clone()
        }
    }
}

fn direct_dependencies(lockfile: &Lockfile) -> BTreeMap<String, DirectDependency> {
    let Some(root) = lockfile.root() else {
        return BTreeMap::new();
    };
    let sections = [(&root.dependencies, false), (&root.dev_dependencies, true)];
    sections
        .into_iter()
        .flat_map(|(section, is_dev)| section.iter().map(move |entry| (entry, is_dev)))
        .map(|((name, locked), is_dev)| {
            let version = lockfile
                .packages
                .get(&locked.version)
                .map_or_else(|| locked.version.clone(), |package| package.version.clone());
            (name.clone(), DirectDependency { version, is_dev })
        })
        .collect()
}
