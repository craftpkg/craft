use contract::{Actor, Context, LockfileError, Result};
use lockfile::Lockfile;
use package::{DependencySpec, InstallPackage, PackageJson};
use resolver::Resolver;

use crate::install_actor::manifest_packages;

/// Lists direct registry dependencies whose locked version is behind what their specifier
/// allows (wanted) or the `latest` tag.
pub struct OutdatedActor {
    context: Context,
}

struct Row {
    name: String,
    current: String,
    wanted: String,
    latest: String,
    section: &'static str,
}

impl Actor<()> for OutdatedActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    async fn run(&self) -> Result<()> {
        let package_json = PackageJson::from_file(&self.context.package_json_path()).await?;
        let lockfile = Lockfile::load(&self.context.cwd)
            .await?
            .ok_or(LockfileError::Missing)?;
        let resolver = Resolver::new(&self.context);

        let mut rows = Vec::new();
        for package in manifest_packages(&package_json) {
            let Some(spec) = package.version.clone() else {
                continue;
            };
            let is_registry_dependency = DependencySpec::parse(&package.name, Some(&spec))
                .is_ok_and(|parsed| parsed.registry_target().is_some());
            if !is_registry_dependency {
                continue;
            }
            let Some(current) = locked_version(&lockfile, &package) else {
                continue;
            };

            let request = |spec: String| {
                InstallPackage::new(package.name.clone(), Some(spec), false).without_lock()
            };
            let wanted = resolver.resolve(&request(spec.clone())).await?.version;
            let latest = resolver
                .resolve(&request(with_requirement(&spec, "latest")))
                .await?
                .version;
            let is_outdated = current != wanted || current != latest;
            if is_outdated {
                rows.push(Row {
                    name: package.name.clone(),
                    current,
                    wanted,
                    latest,
                    section: package.section().key(),
                });
            }
        }

        if rows.is_empty() {
            println!("All dependencies are up to date");
            return Ok(());
        }
        for line in table(&rows) {
            println!("{line}");
        }
        Ok(())
    }
}

/// `spec` asking for `requirement` instead, keeping an `npm:` alias's target.
pub(crate) fn with_requirement(spec: &str, requirement: &str) -> String {
    match spec.strip_prefix("npm:") {
        Some(target) => {
            let name = target
                .char_indices()
                .skip(1)
                .find(|(_, character)| *character == '@')
                .map_or(target, |(index, _)| &target[..index]);
            format!("npm:{name}@{requirement}")
        }
        None => requirement.to_string(),
    }
}

fn locked_version(lockfile: &Lockfile, package: &InstallPackage) -> Option<String> {
    let root = lockfile.root()?;
    let section = match package.section() {
        package::DependencySection::DevDependencies => &root.dev_dependencies,
        package::DependencySection::OptionalDependencies => &root.optional_dependencies,
        _ => &root.dependencies,
    };
    let key = &section.get(&package.name)?.version;
    Some(lockfile.packages.get(key)?.version.clone())
}

fn table(rows: &[Row]) -> Vec<String> {
    let header = ["Package", "Current", "Wanted", "Latest", "Type"];
    let cells: Vec<[&str; 5]> = rows
        .iter()
        .map(|row| {
            [
                &row.name[..],
                &row.current,
                &row.wanted,
                &row.latest,
                row.section,
            ]
        })
        .collect();
    let mut widths = header.map(str::len);
    for row in &cells {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }

    std::iter::once(header)
        .chain(cells)
        .map(|row| {
            let padded: Vec<String> = row
                .iter()
                .zip(widths)
                .map(|(cell, width)| format!("{cell:width$}"))
                .collect();
            padded.join("  ").trim_end().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirement_keeps_alias_targets() {
        assert_eq!(with_requirement("^18.0.0", "latest"), "latest");
        assert_eq!(
            with_requirement("npm:string-width@^4.2.0", "latest"),
            "npm:string-width@latest"
        );
        assert_eq!(
            with_requirement("npm:@scope/pkg@^1.0.0", "latest"),
            "npm:@scope/pkg@latest"
        );
    }

    #[test]
    fn test_table_aligns_columns() {
        let rows = [Row {
            name: "react".to_string(),
            current: "18.3.1".to_string(),
            wanted: "18.3.1".to_string(),
            latest: "19.2.0".to_string(),
            section: "dependencies",
        }];
        assert_eq!(
            table(&rows),
            [
                "Package  Current  Wanted  Latest  Type",
                "react    18.3.1   18.3.1  19.2.0  dependencies",
            ]
        );
    }
}
