use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn installs_file_dependencies() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.1.0".to_string())
    );
}

#[test]
fn installs_the_registry_dependencies_of_a_file_dependency() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:libs/local-lib" } }),
    );
    project.write_file(
        "libs/local-lib/package.json",
        r#"{ "name": "local-lib", "version": "0.1.0", "dependencies": { "ms": "^2.0.0" } }"#,
    );
    project.write_file(
        "libs/local-lib/index.js",
        r#"module.exports = { version: require("ms").version };"#,
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("2.1.3".to_string())
    );
}

#[test]
fn copies_file_dependencies_again_on_every_install() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");
    project.craft().arg("install").assert().success();

    write_local_lib(&project, "0.2.0");
    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.2.0".to_string())
    );
}

#[test]
fn links_link_dependencies_in_place() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "link:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");

    project.craft().arg("install").assert().success();
    write_local_lib(&project, "0.2.0");

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.2.0".to_string())
    );
    let link = project.path().join("node_modules/local-lib");
    assert!(link.is_symlink());
    assert_eq!(
        link.canonicalize().expect("link resolves"),
        project
            .path()
            .join("libs/local-lib")
            .canonicalize()
            .expect("local-lib exists")
    );
}

#[test]
fn installs_file_tarball_dependencies() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "ms": "file:vendor/ms-2.1.3.tgz" } }),
    );
    let tarball = PackageFixture::new("ms", "2.1.3").tarball();
    std::fs::create_dir_all(project.path().join("vendor")).expect("create vendor");
    std::fs::write(project.path().join("vendor/ms-2.1.3.tgz"), tarball).expect("write tarball");

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn installs_tarball_url_dependencies() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": registry.tarball_url("debug", "4.4.0") } }),
    );

    project.craft().arg("install").assert().success();

    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["version"], "4.4.0");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn adds_a_local_directory_under_its_package_name() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "name": "app" }));
    write_local_lib(&project, "0.1.0");

    project
        .craft()
        .args(["add", "./libs/local-lib"])
        .assert()
        .success();

    assert_eq!(
        project.package_json()["dependencies"]["local-lib"],
        "file:libs/local-lib"
    );
    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.1.0".to_string())
    );
}

#[test]
fn adds_a_tarball_url_verbatim() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));
    let url = registry.tarball_url("ms", "2.1.3");

    project.craft().args(["add", &url]).assert().success();

    assert_eq!(project.package_json()["dependencies"]["ms"], url.as_str());
    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

fn write_local_lib(project: &Project, version: &str) {
    project.write_file(
        "libs/local-lib/package.json",
        &format!(r#"{{ "name": "local-lib", "version": "{version}" }}"#),
    );
    project.write_file(
        "libs/local-lib/index.js",
        &format!(r#"module.exports = {{ name: "local-lib", version: "{version}" }};"#),
    );
}
