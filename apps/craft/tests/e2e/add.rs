use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn adds_a_dependency_with_a_caret_range() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project.craft().args(["add", "ms"]).assert().success();

    assert_eq!(project.package_json()["dependencies"]["ms"], "^2.1.3");
    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn adds_a_dev_dependency() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("typescript", "5.9.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project
        .craft()
        .args(["add", "typescript", "-D"])
        .assert()
        .success();

    let package_json = project.package_json();
    assert_eq!(package_json["devDependencies"]["typescript"], "^5.9.3");
    assert!(package_json.get("dependencies").is_none());
}

#[test]
fn adds_several_packages_including_scoped_ones() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("@tanstack/query-core", "5.90.10"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project
        .craft()
        .args(["add", "ms", "@tanstack/query-core"])
        .assert()
        .success();

    let dependencies = &project.package_json()["dependencies"];
    assert_eq!(dependencies["ms"], "^2.1.3");
    assert_eq!(dependencies["@tanstack/query-core"], "^5.90.10");
}

#[test]
fn saves_a_dependency_added_with_a_version_spec() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react", "18.3.1"))
        .publish(PackageFixture::new("react", "19.2.0"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project.craft().args(["add", "react@18"]).assert().success();

    assert_eq!(project.package_json()["dependencies"]["react"], "^18.3.1");
    assert_eq!(project.required_version("react"), Ok("18.3.1".to_string()));
}

#[test]
fn adds_a_package_by_dist_tag() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project
        .craft()
        .args(["add", "ms@latest"])
        .assert()
        .success();

    assert_eq!(project.package_json()["dependencies"]["ms"], "^2.1.3");
}

#[test]
#[ignore = "fixed by 1.9: craft add installs before discovering package.json is missing"]
fn fails_before_installing_without_package_json() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({}));
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project.craft().args(["add", "ms"]).assert().failure();

    assert!(registry.requested_paths().is_empty());
    assert!(!project.path().join("node_modules").exists());
}

#[test]
#[ignore = "fixed by 1.10: package.json is re-serialized with new key order and formatting"]
fn add_then_remove_leaves_package_json_unchanged() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({}));
    let original = "{\n    \"name\": \"app\",\n    \"private\": true,\n    \"type\": \"module\",\n    \"dependencies\": {\n        \"zod\": \"^3.0.0\"\n    }\n}\n";
    project.write_file("package.json", original);

    project.craft().args(["add", "ms"]).assert().success();
    project.craft().args(["remove", "ms"]).assert().success();

    assert_eq!(project.read_file("package.json"), original);
}
