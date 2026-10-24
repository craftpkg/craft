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
#[ignore = "fixed by 1.12: craft exits 1 instead of the script's exit code"]
fn exits_with_the_script_exit_code() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "fail": "exit 3" } }));

    project.craft().args(["run", "fail"]).assert().code(3);
}

#[test]
#[ignore = "fixed by 1.12: node_modules/.bin is not on PATH for scripts"]
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
#[ignore = "fixed by 1.12: pre and post scripts are not run"]
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
#[ignore = "fixed by 1.12: arguments are joined into the shell command unquoted"]
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
