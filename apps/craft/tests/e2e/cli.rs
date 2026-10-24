use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn prints_its_version() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));

    for flag in ["--version", "-v", "-V"] {
        project
            .craft()
            .arg(flag)
            .assert()
            .success()
            .stdout(format!("craft {}\n", env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn passes_version_flags_to_bins() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("echo-args", "1.0.0")
            .file(
                "cli.js",
                "#!/usr/bin/env node\nconsole.log(process.argv.slice(2).join(' '));\n",
            )
            .manifest_field("bin", json!({ "echo-args": "cli.js" })),
    );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "echo-args": "1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["echo-args", "-v", "--version"])
        .assert()
        .success()
        .stdout("-v --version\n");
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
        .stdout(contains("Install the project's dependencies"))
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

fn publish_summary_packages(registry: &MockRegistry) {
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"))
        .publish(PackageFixture::new("react", "18.3.1"))
        .publish(PackageFixture::new("react", "19.2.0"));
}

#[test]
fn summarizes_what_an_install_changed() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0", "react": "^19.0.0" }, "devDependencies": { "ms": "^2.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains(
            "+ debug 4.4.0\n+ ms 2.1.3 (dev)\n+ react 19.2.0\n",
        ))
        .stdout(contains("3 packages installed in"));

    project.write_file(
        "package.json",
        &json!({ "dependencies": { "debug": "^4.0.0", "react": "^18.0.0" } }).to_string(),
    );
    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains("- ms 2.1.3\n~ react 19.2.0 → 18.3.1\n"));

    project
        .craft()
        .args(["remove", "debug"])
        .assert()
        .success()
        .stdout("- debug 4.4.0\n");
}

#[test]
fn silent_prints_nothing() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project
        .craft()
        .args(["--silent", "install"])
        .assert()
        .success()
        .stdout("")
        .stderr("");

    assert!(project.require("debug").is_ok());
}

#[test]
fn lists_and_explains_installed_packages() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0" }, "devDependencies": { "react": "^19.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["ls", "--depth", "1"])
        .assert()
        .success()
        .stdout("dependencies:\ndebug 4.4.0\n└── ms 2.1.3\n\ndevDependencies:\nreact 19.2.0\n");
    project
        .craft()
        .args(["why", "ms"])
        .assert()
        .success()
        .stdout("ms 2.1.3\n  dependencies: debug 4.4.0 → ms 2.1.3\n");
}

#[test]
fn trusts_a_package_and_runs_its_scripts() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("native", "1.0.0").manifest_field(
        "scripts",
        json!({ "postinstall": "node -e \"require('fs').writeFileSync('built.txt', 'yes')\"" }),
    ));
    let project = Project::new(&registry, json!({ "dependencies": { "native": "^1.0.0" } }));
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["trust", "native"])
        .assert()
        .success()
        .stdout(contains("Trusted native"))
        .stdout(contains("Ran install scripts of native@1.0.0"));

    assert_eq!(
        project.package_json()["trustedDependencies"],
        json!(["native"])
    );
}

#[test]
fn saves_optional_and_peer_dependencies() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(&registry, json!({ "name": "lib" }));

    project.craft().args(["add", "-O", "ms"]).assert().success();
    project
        .craft()
        .args(["add", "-P", "react@^19.0.0"])
        .assert()
        .success();

    let package_json = project.package_json();
    assert_eq!(package_json["optionalDependencies"]["ms"], "^2.1.3");
    assert_eq!(package_json["peerDependencies"]["react"], "^19.2.0");
    assert_eq!(package_json["devDependencies"]["react"], "^19.2.0");
    assert!(package_json.get("dependencies").is_none());
    let lock: serde_json::Value =
        serde_json::from_str(&project.read_file("craft.lock")).expect("craft.lock is JSON");
    assert_eq!(
        lock["importers"]["."]["optionalDependencies"]["ms"]["version"],
        "ms@2.1.3"
    );
    assert!(project.require("ms").is_ok());
    assert!(project.require("react").is_ok());
}

#[test]
fn skips_a_missing_optional_dependency_of_the_project() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0" }, "optionalDependencies": { "ghost": "^1.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains(
            "warning: skipping optional dependency ghost@^1.0.0",
        ));

    assert!(project.require("debug").is_ok());
}

#[test]
fn production_installs_leave_dev_dependencies_out() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": "^4.0.0" }, "devDependencies": { "react": "^19.0.0" } }),
    );
    project.craft().arg("install").assert().success();
    let lock = project.read_file("craft.lock");

    project
        .craft()
        .args(["install", "--production"])
        .assert()
        .success();

    assert!(project.require("debug").is_ok());
    assert!(project.require("react").is_err());
    assert_eq!(project.read_file("craft.lock"), lock);

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains("Already up to date").not());
    assert!(project.require("react").is_ok());
}

#[test]
fn reports_and_updates_outdated_dependencies() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "react": "^18.0.0", "debug": "4.4.0" } }),
    );
    project.craft().arg("install").assert().success();

    project.craft().arg("outdated").assert().success().stdout(
        "Package  Current  Wanted  Latest  Type\nreact    18.3.1   18.3.1  19.2.0  dependencies\n",
    );

    project
        .craft()
        .args(["update", "--latest"])
        .assert()
        .success()
        .stdout(contains("~ react 18.3.1 → 19.2.0"));

    let package_json = project.package_json();
    assert_eq!(package_json["dependencies"]["react"], "^19.2.0");
    assert_eq!(package_json["dependencies"]["debug"], "4.4.0");
    assert_eq!(project.required_version("react"), Ok("19.2.0".to_string()));
    project
        .craft()
        .arg("outdated")
        .assert()
        .success()
        .stdout("All dependencies are up to date\n");
}

#[test]
fn updates_within_the_specifier_without_touching_package_json() {
    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    project.craft().arg("install").assert().success();
    registry.publish(PackageFixture::new("ms", "2.1.4"));
    let package_json = project.read_file("package.json");

    project.craft().args(["update", "debug"]).assert().success();
    assert_eq!(
        project.require("debug").expect("debug loads")["dependencies"]["ms"],
        "2.1.3"
    );

    project.craft().arg("update").assert().success();
    assert_eq!(
        project.require("debug").expect("debug loads")["dependencies"]["ms"],
        "2.1.4"
    );
    assert_eq!(project.read_file("package.json"), package_json);

    project
        .craft()
        .args(["update", "lodash"])
        .assert()
        .failure()
        .stderr(contains("lodash is not a dependency of this project"));
}

#[test]
fn reports_errors_with_their_causes() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "dependencies": { "ghost": "^1.0.0" } }));

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(format!(
            "error: failed to install ghost@^1.0.0\n  caused by: package ghost not found in {}\n",
            registry.url()
        ));
}

#[test]
fn says_when_the_registry_cannot_be_reached() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(".npmrc", "fetch-retries=0\n");

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("CRAFT_REGISTRY", "http://127.0.0.1:9")
        .assert()
        .failure()
        .stderr(contains("could not reach 127.0.0.1:9 ("))
        .stderr(contains("or use --offline"));
}

#[cfg(unix)]
#[test]
fn names_the_path_and_hints_on_permission_errors() {
    use std::os::unix::fs::PermissionsExt;

    let registry = MockRegistry::start();
    publish_summary_packages(&registry);
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    std::fs::set_permissions(project.path(), std::fs::Permissions::from_mode(0o555))
        .expect("make the project read-only");
    let is_enforced = std::fs::write(project.path().join("probe"), "").is_err();

    let assert = project.craft().arg("install").assert();
    std::fs::set_permissions(project.path(), std::fs::Permissions::from_mode(0o755))
        .expect("restore permissions");
    if !is_enforced {
        return;
    }

    assert
        .failure()
        .stderr(contains("could not create directory"))
        .stderr(contains("node_modules: Permission denied"))
        .stderr(contains("hint: check that you may write to the path above"));
}
