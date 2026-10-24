use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

fn publish_debug_and_cowsay(registry: &MockRegistry) {
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"))
        .publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
}

#[test]
fn prunes_dependencies_dropped_from_package_json() {
    let registry = MockRegistry::start();
    publish_debug_and_cowsay(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "cowsay": "^1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project.write_file(
        "package.json",
        &json!({ "dependencies": { "debug": "^4.0.0" } }).to_string(),
    );
    project.craft().arg("install").assert().success();

    assert_eq!(virtual_store_entries(&project), ["debug@4.4.0", "ms@2.1.3"]);
    assert!(missing(&project, "node_modules/cowsay"));
    assert!(missing(&project, "node_modules/.bin/cowsay"));
    assert!(project.require("debug").is_ok());
}

#[test]
fn craft_remove_prunes_orphaned_dependencies() {
    let registry = MockRegistry::start();
    publish_debug_and_cowsay(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "cowsay": "^1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project.craft().args(["remove", "debug"]).assert().success();

    assert_eq!(virtual_store_entries(&project), ["cowsay@1.6.0"]);
    assert!(missing(&project, "node_modules/.craft/node_modules/ms"));
    assert!(project.require("cowsay").is_ok());
}

#[test]
fn keeps_dependencies_still_needed_by_others() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"))
        .publish(PackageFixture::new("timer", "1.0.0").dependency("ms", "^2.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "timer": "^1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project.craft().args(["remove", "debug"]).assert().success();

    assert_eq!(virtual_store_entries(&project), ["ms@2.1.3", "timer@1.0.0"]);
    assert_eq!(
        project.require("timer").expect("timer loads")["dependencies"]["ms"],
        "2.1.3"
    );
}

#[test]
fn craft_add_keeps_existing_dependencies() {
    let registry = MockRegistry::start();
    publish_debug_and_cowsay(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();

    project.craft().args(["add", "cowsay"]).assert().success();

    assert!(project.require("debug").is_ok());
    assert!(project.require("cowsay").is_ok());
    assert_eq!(
        virtual_store_entries(&project),
        ["cowsay@1.6.0", "debug@4.4.0", "ms@2.1.3"]
    );
}

#[cfg(unix)]
#[test]
fn replaces_leftovers_of_the_old_layout() {
    let registry = MockRegistry::start();
    publish_debug_and_cowsay(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    let node_modules = project.path().join("node_modules");
    std::fs::create_dir_all(node_modules.join(".cache/vite")).expect("create tool cache");
    let old_cache = project.craft_home().join("packages");
    std::os::unix::fs::symlink(
        old_cache.join("debug-4.3.0/package"),
        node_modules.join("debug"),
    )
    .expect("link old debug");
    std::os::unix::fs::symlink(
        old_cache.join("lodash-4.17.21/package"),
        node_modules.join("lodash"),
    )
    .expect("link old lodash");

    project.craft().arg("install").assert().success();

    assert!(missing(&project, "node_modules/lodash"));
    assert!(node_modules.join(".cache/vite").exists());
    assert_eq!(
        std::fs::read_link(node_modules.join("debug")).expect("debug is a link"),
        std::path::PathBuf::from(".craft/debug@4.4.0/node_modules/debug")
    );
}

/// Package entries of `node_modules/.craft`, without the hoisting directory.
fn virtual_store_entries(project: &Project) -> Vec<String> {
    let mut entries: Vec<String> = std::fs::read_dir(project.path().join("node_modules/.craft"))
        .expect("virtual store exists")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "node_modules")
        .collect();
    entries.sort();
    entries
}

fn missing(project: &Project, relative_path: &str) -> bool {
    std::fs::symlink_metadata(project.path().join(relative_path)).is_err()
}
