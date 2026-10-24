use std::path::{Path, PathBuf};

use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn lays_packages_out_in_the_virtual_store() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project.craft().arg("install").assert().success();

    let node_modules = project.path().join("node_modules");
    assert_eq!(
        read_link(&node_modules.join("debug")),
        PathBuf::from(".craft/debug@4.4.0/node_modules/debug")
    );
    assert_eq!(
        read_link(&node_modules.join(".craft/debug@4.4.0/node_modules/ms")),
        PathBuf::from("../../ms@2.1.3/node_modules/ms")
    );
    assert!(
        node_modules
            .join(".craft/ms@2.1.3/node_modules/ms/package.json")
            .is_file()
    );
    assert!(std::fs::symlink_metadata(node_modules.join("ms")).is_err());
    let store_entry_has_links = project.store_entries().iter().any(|entry| {
        project
            .store_dir()
            .join(entry)
            .join("node_modules")
            .exists()
    });
    assert!(!store_entry_has_links);
}

#[test]
fn keeps_projects_sharing_a_store_independent() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.0.0"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.0.0"));
    let older = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    older.craft().arg("install").assert().success();

    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let newer = older.sibling(json!({ "dependencies": { "debug": "^4.0.0" } }));
    newer.craft().arg("install").assert().success();

    let ms_seen_by_debug = |project: &Project| {
        project.require("debug").expect("debug loads")["dependencies"]["ms"].clone()
    };
    assert_eq!(ms_seen_by_debug(&newer), "2.1.3");
    assert_eq!(ms_seen_by_debug(&older), "2.0.0");
}

#[test]
fn keeps_working_after_the_store_is_deleted() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();

    std::fs::remove_dir_all(project.craft_home().join("store")).expect("delete the store");

    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn resolves_undeclared_requires_through_hidden_hoisting() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"))
        .publish(PackageFixture::new("sloppy", "1.0.0").file(
            "index.js",
            r#"module.exports = { ms: require("ms/package.json").version };"#,
        ));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "sloppy": "^1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.require("sloppy").expect("sloppy loads")["ms"],
        "2.1.3"
    );
    assert!(project.require("ms").is_err());
}

#[test]
fn links_dependency_bins_inside_each_virtual_store_entry() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"))
        .publish(PackageFixture::new("greeter", "1.0.0").dependency("cowsay", "^1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "greeter": "^1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    let node_modules = project.path().join("node_modules");
    assert!(
        node_modules
            .join(".craft/greeter@1.0.0/node_modules/.bin/cowsay")
            .exists()
    );
    assert!(std::fs::symlink_metadata(node_modules.join(".bin/cowsay")).is_err());
}

fn read_link(path: &Path) -> PathBuf {
    std::fs::read_link(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
