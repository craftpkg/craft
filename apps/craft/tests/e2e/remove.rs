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
#[ignore = "fixed by 1.11: craft remove leaves the package's bins in node_modules/.bin"]
fn removes_the_bins_of_a_removed_package() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(&registry, json!({ "dependencies": { "cowsay": "^1.0.0" } }));
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["remove", "cowsay"])
        .assert()
        .success();

    let bin = project.path().join("node_modules/.bin/cowsay");
    assert!(std::fs::symlink_metadata(bin).is_err());
}

#[cfg(unix)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("create dangling symlink");
}

#[cfg(windows)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("create dangling symlink");
}
