use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn runs_a_script_from_package_json() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "greet": "echo hello" } }));

    project
        .craft()
        .args(["run", "greet"])
        .assert()
        .success()
        .stdout(contains("hello"));
}

#[test]
fn passes_arguments_to_the_script() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "greet": "echo hello" } }));

    project
        .craft()
        .args(["run", "greet", "world"])
        .assert()
        .success()
        .stdout(contains("hello world"));
}

#[test]
fn runs_start_and_test_shortcuts() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "start": "echo starting", "test": "echo testing" } }),
    );

    project
        .craft()
        .arg("start")
        .assert()
        .success()
        .stdout(contains("starting"));
    project
        .craft()
        .arg("test")
        .assert()
        .success()
        .stdout(contains("testing"));
}

#[test]
fn runs_an_unknown_command_as_a_script() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "lint": "echo linting" } }));

    project
        .craft()
        .arg("lint")
        .assert()
        .success()
        .stdout(contains("linting"));
}

#[test]
fn fails_when_the_script_fails() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "fail": "exit 3" } }));

    project.craft().args(["run", "fail"]).assert().failure();
}

#[test]
fn exits_with_the_script_exit_code() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "fail": "exit 3" } }));

    project
        .craft()
        .args(["run", "fail"])
        .assert()
        .code(3)
        .stderr("error: fail exited with code 3\n");
}

#[test]
fn passes_arguments_only_to_the_main_script() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "pretest": "echo pre", "test": "echo test", "posttest": "echo post" } }),
    );

    project
        .craft()
        .args(["run", "test", "--watch"])
        .assert()
        .success()
        .stdout("pre\ntest --watch\npost\n");
}

#[test]
fn stops_at_a_failing_pre_script() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "prebuild": "exit 4", "build": "echo build" } }),
    );

    project
        .craft()
        .args(["run", "build"])
        .assert()
        .code(4)
        .stdout("");
}

#[test]
fn reports_a_missing_script_with_the_available_ones() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "test": "echo test", "build": "echo build" } }),
    );

    project
        .craft()
        .args(["run", "deploy"])
        .assert()
        .failure()
        .stderr(contains(
            "script \"deploy\" not found; available scripts: build, test",
        ));
}

#[test]
fn sets_npm_environment_variables() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({
            "name": "app",
            "version": "1.2.3",
            "scripts": { "env": "echo $npm_lifecycle_event $npm_package_name $npm_package_version $npm_config_user_agent" },
        }),
    );

    project
        .craft()
        .args(["run", "env"])
        .assert()
        .success()
        .stdout(contains("env app 1.2.3 craft/"));
}

#[test]
fn runs_dependency_bins_directly() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(&registry, json!({ "dependencies": { "cowsay": "^1.0.0" } }));
    project.craft().arg("install").assert().success();
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project
        .craft()
        .args(["cowsay", "a; echo INJECTED"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"))
        .stdout(contains("INJECTED\n").not());
}

#[test]
fn scripts_can_call_dependency_bins() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(
        &registry,
        json!({ "scripts": { "moo": "cowsay" }, "dependencies": { "cowsay": "^1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["run", "moo"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));
}

#[test]
fn runs_pre_and_post_scripts_in_order() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "prebuild": "echo pre", "build": "echo build", "postbuild": "echo post" } }),
    );

    project
        .craft()
        .args(["run", "build"])
        .assert()
        .success()
        .stdout("pre\nbuild\npost\n");
}

#[test]
fn passes_arguments_verbatim() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "show": "echo" } }));

    project
        .craft()
        .args(["run", "show", "a; echo INJECTED"])
        .assert()
        .success()
        .stdout("a; echo INJECTED\n");
}
