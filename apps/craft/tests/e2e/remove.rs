use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn removes_the_package_and_its_dependency_entry() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("lodash", "4.17.21"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "ms": "^2.0.0", "lodash": "^4.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project.craft().args(["remove", "ms"]).assert().success();

    let dependencies = &project.package_json()["dependencies"];
    assert!(dependencies.get("ms").is_none());
    assert_eq!(dependencies["lodash"], "^4.0.0");
    assert!(project.require("ms").is_err());
    assert_eq!(
        project.required_version("lodash"),
        Ok("4.17.21".to_string())
    );
}

#[test]
fn removes_dev_dependencies() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("typescript", "5.9.3"));
    let project = Project::new(
        &registry,
        json!({ "devDependencies": { "typescript": "^5.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["remove", "typescript"])
        .assert()
        .success();

    assert!(
        project.package_json()["devDependencies"]
            .get("typescript")
            .is_none()
    );
    assert!(!project.path().join("node_modules/typescript").exists());
}

#[test]
fn removes_a_dangling_symlink() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    let link = project.path().join("node_modules/ms");
    std::fs::create_dir_all(project.path().join("node_modules")).expect("create node_modules");
    symlink_dir("/nonexistent/ms", &link);

    project.craft().args(["remove", "ms"]).assert().success();

    assert!(std::fs::symlink_metadata(&link).is_err());
}

#[test]
fn removes_the_bins_of_a_removed_package() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"))
        .publish(PackageFixture::new("typescript", "5.9.3").bin("tsc", "bin/tsc"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "cowsay": "^1.0.0", "typescript": "^5.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["remove", "cowsay"])
        .assert()
        .success();

    let bin = project.path().join("node_modules/.bin/cowsay");
    assert!(std::fs::symlink_metadata(bin).is_err());
    assert!(project.path().join("node_modules/.bin/tsc").exists());
}

#[test]
fn warns_about_packages_that_are_not_dependencies() {
    let registry = MockRegistry::start();
    let original = "{\n  \"dependencies\": {\n    \"zod\": \"^3.0.0\"\n  }\n}\n";
    let project = Project::new(&registry, json!({}));
    project.write_file("package.json", original);

    project
        .craft()
        .args(["remove", "ms"])
        .assert()
        .success()
        .stderr(contains("warning: ms is not a dependency of this project"));

    assert_eq!(project.read_file("package.json"), original);
}

#[cfg(unix)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("create dangling symlink");
}

#[cfg(windows)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("create dangling symlink");
}

#[test]
fn removes_optional_and_peer_dependencies() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("react", "19.2.0"));
    let project = Project::new(
        &registry,
        json!({
            "name": "lib",
            "optionalDependencies": { "ms": "^2.0.0" },
            "peerDependencies": { "react": "^19.0.0" },
            "devDependencies": { "react": "^19.0.0" },
        }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["remove", "ms", "react"])
        .assert()
        .success();

    assert_eq!(project.package_json(), json!({ "name": "lib" }));
    assert!(project.require("ms").is_err());
    assert!(project.require("react").is_err());
    let lock: serde_json::Value =
        serde_json::from_str(&project.read_file("craft.lock")).expect("craft.lock is JSON");
    assert_eq!(lock["packages"], json!({}));
}
