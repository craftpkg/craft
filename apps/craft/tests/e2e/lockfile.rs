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
