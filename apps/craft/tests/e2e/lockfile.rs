use serde_json::{Value, json};
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

fn publish_react_stack(registry: &MockRegistry) {
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"))
        .publish(PackageFixture::new("react", "19.2.0"))
        .publish(PackageFixture::new("ui", "1.0.0").peer_dependency("react", "^19.0.0"));
}

fn lockfile(project: &Project) -> Value {
    serde_json::from_str(&project.read_file("craft.lock")).expect("craft.lock is valid JSON")
}

#[test]
fn records_what_every_dependency_resolved_to() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(
        &registry,
        json!({
            "dependencies": { "debug": "^4.0.0", "ui": "^1.0.0", "react": "^19.0.0" },
            "devDependencies": { "ms": "^2.0.0" },
        }),
    );

    project.craft().arg("install").assert().success();

    let lock = lockfile(&project);
    assert_eq!(lock["lockfileVersion"], 1);
    let root = &lock["importers"]["."];
    assert_eq!(
        root["dependencies"]["debug"],
        json!({ "specifier": "^4.0.0", "version": "debug@4.4.0" })
    );
    assert_eq!(
        root["dependencies"]["ui"]["version"],
        "ui@1.0.0_react@19.2.0"
    );
    assert_eq!(root["devDependencies"]["ms"]["version"], "ms@2.1.3");

    let debug = &lock["packages"]["debug@4.4.0"];
    assert_eq!(debug["dependencies"], json!({ "ms": "ms@2.1.3" }));
    assert_eq!(debug["resolution"]["type"], "registry");
    assert_eq!(
        debug["resolution"]["tarball"],
        registry.tarball_url("debug", "4.4.0").as_str()
    );
    let integrity = debug["resolution"]["integrity"]
        .as_str()
        .expect("integrity");
    assert!(integrity.starts_with("sha512-"), "{integrity}");
    assert_eq!(
        lock["packages"]["ui@1.0.0_react@19.2.0"]["peerDependencies"],
        json!({ "react": "react@19.2.0" })
    );
    assert!(project.read_file("craft.lock").ends_with("}\n"));
}

#[test]
fn writes_the_same_lockfile_for_the_same_project_anywhere() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let package_json = json!({
        "dependencies": { "debug": "^4.0.0", "ui": "^1.0.0", "local": "file:libs/local" },
    });
    let first = Project::new(&registry, package_json.clone());
    let second = first.sibling(package_json);
    for project in [&first, &second] {
        project.write_file(
            "libs/local/package.json",
            r#"{ "name": "local", "version": "0.1.0" }"#,
        );
        project.craft().arg("install").assert().success();
    }

    assert_eq!(
        first.read_file("craft.lock"),
        second.read_file("craft.lock")
    );
    let lock = lockfile(&first);
    assert_eq!(
        lock["packages"]["local@file:libs/local"]["resolution"],
        json!({ "type": "directory", "path": "libs/local" })
    );
}

#[test]
fn leaves_an_unchanged_lockfile_alone() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    let lock_path = project.path().join("craft.lock");
    let written_at = std::fs::metadata(&lock_path)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    std::thread::sleep(std::time::Duration::from_millis(20));

    project.craft().arg("install").assert().success();

    let rewritten_at = std::fs::metadata(&lock_path)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    assert_eq!(written_at, rewritten_at);
}

#[test]
fn records_the_saved_specifier_on_add() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project.craft().args(["add", "-D", "ms"]).assert().success();

    let root = &lockfile(&project)["importers"]["."];
    assert_eq!(
        root["devDependencies"]["ms"],
        json!({ "specifier": "^2.1.3", "version": "ms@2.1.3" })
    );
    assert_eq!(root["dependencies"]["debug"]["specifier"], "^4.0.0");
}

#[test]
fn drops_removed_dependencies_and_what_only_they_needed() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "react": "^19.0.0" } }),
    );
    project.craft().arg("install").assert().success();
    let requests_before = registry.requested_paths().len();

    project.craft().args(["remove", "debug"]).assert().success();

    let lock = lockfile(&project);
    assert_eq!(
        lock["importers"]["."]["dependencies"],
        json!({ "react": { "specifier": "^19.0.0", "version": "react@19.2.0" } })
    );
    let packages: Vec<&String> = lock["packages"]
        .as_object()
        .expect("packages")
        .keys()
        .collect();
    assert_eq!(packages, ["react@19.2.0"]);
    assert_eq!(registry.requested_paths().len(), requests_before);
}

fn metadata_requests(registry: &MockRegistry) -> Vec<String> {
    registry
        .requested_paths()
        .into_iter()
        .filter(|path| !path.ends_with(".tgz"))
        .collect()
}

#[test]
fn skips_an_install_that_is_already_up_to_date() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    let requests_before = registry.requested_paths().len();

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout("Already up to date\n");

    assert_eq!(registry.requested_paths().len(), requests_before);
}

#[test]
fn installs_a_fresh_checkout_without_registry_metadata() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let package_json =
        json!({ "dependencies": { "debug": "^4.0.0", "ui": "^1.0.0", "react": "^19.0.0" } });
    let original = Project::new(&registry, package_json.clone());
    original.craft().arg("install").assert().success();
    let lock = original.read_file("craft.lock");

    let checkout = Project::new(&registry, package_json);
    checkout.write_file("craft.lock", &lock);
    let metadata_before = metadata_requests(&registry).len();
    checkout.craft().arg("install").assert().success();

    assert_eq!(metadata_requests(&registry).len(), metadata_before);
    assert_eq!(checkout.read_file("craft.lock"), lock);
    let debug = checkout.require("debug").expect("debug loads");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn keeps_locked_versions_when_newer_ones_are_published() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();

    registry
        .publish(PackageFixture::new("ms", "2.1.4"))
        .publish(PackageFixture::new("debug", "4.4.1").dependency("ms", "^2.1.3"));
    std::fs::remove_dir_all(project.path().join("node_modules")).expect("remove node_modules");
    project.craft().arg("install").assert().success();

    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["version"], "4.4.0");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn re_resolves_only_dependencies_whose_specifier_changed() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    registry.publish(PackageFixture::new("react", "18.3.1"));
    registry.set_dist_tag("react", "latest", "19.2.0");
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "react": "^19.0.0" } }),
    );
    project.craft().arg("install").assert().success();
    let metadata_before = metadata_requests(&registry).len();

    project.write_file(
        "package.json",
        &json!({ "dependencies": { "debug": "^4.0.0", "react": "^18.0.0" } }).to_string(),
    );
    project.craft().arg("install").assert().success();

    let new_requests: Vec<String> = metadata_requests(&registry)[metadata_before..].to_vec();
    assert_eq!(new_requests, ["/react"]);
    assert_eq!(project.required_version("react"), Ok("18.3.1".to_string()));
    assert_eq!(
        lockfile(&project)["importers"]["."]["dependencies"]["react"],
        json!({ "specifier": "^18.0.0", "version": "react@18.3.1" })
    );
}

#[test]
fn craft_add_resolves_the_added_package_afresh() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    registry.publish(PackageFixture::new("ms", "2.1.4"));

    project.craft().args(["add", "ms"]).assert().success();

    assert_eq!(project.package_json()["dependencies"]["ms"], "^2.1.4");
    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn frozen_install_needs_a_lockfile() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "craft.lock is missing; run `craft install` to create it",
        ));

    assert!(registry.requested_paths().is_empty());
}

#[test]
fn frozen_install_lists_what_is_out_of_date() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "react": "^19.0.0" } }),
    );
    project.craft().arg("install").assert().success();
    project.write_file(
        "package.json",
        &json!({ "dependencies": { "debug": "^4.4.0", "ms": "^2.0.0" } }).to_string(),
    );

    project
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "craft.lock is out of date with package.json: debug (^4.4.0 in package.json, ^4.0.0 in craft.lock), ms (not in craft.lock), react (not in package.json)",
        ));
}

#[test]
fn frozen_install_uses_only_the_lockfile() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let package_json =
        json!({ "dependencies": { "debug": "^4.0.0", "ui": "^1.0.0", "react": "^19.0.0" } });
    let original = Project::new(&registry, package_json.clone());
    original.craft().arg("install").assert().success();
    let lock = original.read_file("craft.lock");
    registry.publish(PackageFixture::new("ms", "2.1.4"));

    let checkout = Project::new(&registry, package_json);
    checkout.write_file("craft.lock", &lock);
    let metadata_before = metadata_requests(&registry).len();
    checkout
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .success();

    assert_eq!(metadata_requests(&registry).len(), metadata_before);
    assert_eq!(checkout.read_file("craft.lock"), lock);
    assert_eq!(
        checkout.require("debug").expect("debug loads")["dependencies"]["ms"],
        "2.1.3"
    );
}

#[test]
fn frozen_install_fails_on_an_incomplete_lockfile() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    let mut lock = lockfile(&project);
    lock["packages"]
        .as_object_mut()
        .expect("packages")
        .remove("ms@2.1.3");
    project.write_file(
        "craft.lock",
        &serde_json::to_string_pretty(&lock).expect("render lock"),
    );
    std::fs::remove_dir_all(project.path().join("node_modules")).expect("remove node_modules");
    let metadata_before = metadata_requests(&registry).len();

    project
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "ms@^2.1.3 is not in craft.lock; run `craft install` to update it",
        ));

    assert_eq!(metadata_requests(&registry).len(), metadata_before);
}

#[test]
fn installs_are_frozen_in_ci() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("CI", "true")
        .assert()
        .failure()
        .stderr(predicates::str::contains("craft.lock is missing"));

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command.arg("install").env("CI", "false").assert().success();
}

#[test]
fn craft_ci_reinstalls_from_scratch() {
    let registry = MockRegistry::start();
    publish_react_stack(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    project.write_file("node_modules/stray/index.js", "module.exports = 1;");

    project.craft().arg("ci").assert().success();

    assert!(!project.path().join("node_modules/stray").exists());
    assert_eq!(
        project.require("debug").expect("debug loads")["dependencies"]["ms"],
        "2.1.3"
    );
}
