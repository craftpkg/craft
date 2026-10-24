use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn prints_its_version() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));

    project
        .craft()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("craft {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn describes_every_command_in_help() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));

    project
        .craft()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Add packages to package.json and install them"))
        .stdout(contains("install  Install the project's dependencies"))
        .stdout(contains("[aliases: rm, uninstall, un]"))
        .stdout(contains("Create a package.json"));
}

#[test]
fn installs_packages_through_npm_style_aliases() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("typescript", "5.9.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project.craft().args(["i", "ms"]).assert().success();
    project
        .craft()
        .args(["install", "-D", "typescript"])
        .assert()
        .success();

    let package_json = project.package_json();
    assert_eq!(package_json["dependencies"]["ms"], "^2.1.3");
    assert_eq!(package_json["devDependencies"]["typescript"], "^5.9.3");

    project.craft().args(["rm", "ms"]).assert().success();
    project
        .craft()
        .args(["un", "typescript"])
        .assert()
        .success();
    assert!(project.package_json().get("dependencies").is_none());
    assert!(project.package_json().get("devDependencies").is_none());
}

#[test]
fn lists_scripts_when_run_gets_no_name() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "name": "app", "scripts": { "test": "jest", "build": "vite build" } }),
    );

    project
        .craft()
        .arg("run")
        .assert()
        .success()
        .stdout("Scripts in app:\n  build\n    vite build\n  test\n    jest\n");
}

#[test]
fn creates_a_package_json_with_init() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project
        .craft()
        .arg("init")
        .assert()
        .success()
        .stdout("Created package.json\n");
    project
        .craft()
        .arg("init")
        .assert()
        .failure()
        .stderr(contains("package.json already exists"));

    assert_eq!(project.package_json()["version"], "1.0.0");
    project.craft().arg("install").assert().success();
}

#[test]
fn suggests_init_without_a_package_json() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(
            "no package.json found; run `craft init` to create one",
        ));
}
