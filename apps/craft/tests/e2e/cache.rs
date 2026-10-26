use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

fn metadata_requests(registry: &MockRegistry) -> usize {
    registry
        .requested_paths()
        .iter()
        .filter(|path| !path.ends_with(".tgz"))
        .count()
}

fn publish_debug(registry: &MockRegistry) {
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
}

fn installed_twice(registry: &MockRegistry, second_args: &[&str]) -> Project {
    let package_json = json!({ "dependencies": { "debug": "^4.0.0" } });
    let first = Project::new(registry, package_json.clone());
    first.craft().arg("install").assert().success();
    let second = first.sibling(package_json);
    second.craft().args(second_args).assert().success();
    second
}

#[test]
fn requests_abbreviated_metadata() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(registry.abbreviated_responses(), 2);
}

#[test]
fn revalidates_cached_metadata_with_etags() {
    let registry = MockRegistry::start();
    publish_debug(&registry);

    let second = installed_twice(&registry, &["install"]);

    assert_eq!(registry.not_modified_responses(), 2);
    assert_eq!(
        second.require("debug").expect("debug loads")["dependencies"]["ms"],
        "2.1.3"
    );
}

#[test]
fn sees_new_versions_after_revalidating() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let first = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    first.craft().arg("install").assert().success();
    registry.publish(PackageFixture::new("ms", "2.1.4"));

    let second = first.sibling(json!({ "dependencies": { "ms": "^2.0.0" } }));
    second.craft().arg("install").assert().success();

    assert_eq!(second.required_version("ms"), Ok("2.1.4".to_string()));
}

#[test]
fn trusts_fresh_metadata_without_asking() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    registry.set_max_age(300);
    let package_json = json!({ "dependencies": { "debug": "^4.0.0" } });
    let first = Project::new(&registry, package_json.clone());
    first.craft().arg("install").assert().success();
    let metadata_before = metadata_requests(&registry);

    first
        .sibling(package_json)
        .craft()
        .arg("install")
        .assert()
        .success();

    assert_eq!(metadata_requests(&registry), metadata_before);
}

#[test]
fn prefer_offline_uses_the_cache_as_is() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let package_json = json!({ "dependencies": { "debug": "^4.0.0" } });
    let first = Project::new(&registry, package_json.clone());
    first.craft().arg("install").assert().success();
    let metadata_before = metadata_requests(&registry);

    first
        .sibling(package_json)
        .craft()
        .args(["--prefer-offline", "install"])
        .assert()
        .success();

    assert_eq!(metadata_requests(&registry), metadata_before);
}

#[test]
fn offline_installs_from_cache_and_store_alone() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let package_json = json!({ "dependencies": { "debug": "^4.0.0" } });
    let first = Project::new(&registry, package_json.clone());
    first.craft().arg("install").assert().success();
    let requests_before = registry.requested_paths().len();

    let second = first.sibling(package_json);
    second
        .craft()
        .args(["--offline", "install"])
        .assert()
        .success();

    assert_eq!(registry.requested_paths().len(), requests_before);
    assert!(second.require("debug").is_ok());
}

#[test]
fn offline_reports_what_is_missing() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project
        .craft()
        .args(["--offline", "install"])
        .assert()
        .failure()
        .stderr(contains("is not cached and craft is offline"));

    assert!(registry.requested_paths().is_empty());
}

#[test]
fn cache_clean_removes_cached_metadata() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    let metadata_dir = project.craft_home().join("cache/metadata");
    assert!(metadata_dir.exists());

    project
        .craft()
        .args(["cache", "clean", "--force"])
        .assert()
        .success();

    assert!(!metadata_dir.exists());
}

#[test]
fn cache_clean_keeps_everything_without_force() {
    let registry = MockRegistry::start();
    publish_debug(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["cache", "clean"])
        .assert()
        .success()
        .stdout(contains("--force"));

    assert!(project.craft_home().join("cache/metadata").exists());
    assert!(!project.store_entries().is_empty());
}
