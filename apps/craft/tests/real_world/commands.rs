use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;

use crate::common::World;

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn outdated_and_update_move_old_dependencies_forward() {
    let world = World::new(&json!({
        "dependencies": { "ms": "2.0.0", "debug": "~3.1.0" },
    }));
    world.craft().arg("install").assert().success();

    world
        .craft()
        .arg("outdated")
        .assert()
        .success()
        .stdout(contains("ms       2.0.0").and(contains("debug    3.1.0")));

    world.craft().arg("update").assert().success();
    assert_eq!(world.package_json()["dependencies"]["ms"], "2.0.0");
    assert_eq!(
        world.node("console.log(require('debug/package.json').version)"),
        "3.1.0\n"
    );

    world
        .craft()
        .args(["update", "--latest", "ms"])
        .assert()
        .success()
        .stdout(contains("~ ms 2.0.0 → 2.1.3"));
    let dependencies = &world.package_json()["dependencies"];
    assert_eq!(dependencies["ms"], "2.1.3");
    assert_eq!(dependencies["debug"], "~3.1.0");

    world.craft().args(["update", "-L"]).assert().success();
    let debug = world.package_json()["dependencies"]["debug"]
        .as_str()
        .expect("debug has a specifier")
        .to_string();
    assert!(debug.starts_with('~') && !debug.starts_with("~3."));
    world
        .craft()
        .arg("outdated")
        .assert()
        .success()
        .stdout("All dependencies are up to date\n");
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn list_why_and_remove_follow_the_real_tree() {
    let world = World::new(&json!({
        "dependencies": { "express": "4.21.2", "lodash": "4.17.21" },
    }));
    world.craft().arg("install").assert().success();

    world
        .craft()
        .args(["list", "--depth", "1"])
        .assert()
        .success()
        .stdout(contains("express 4.21.2\n├── accepts").and(contains("lodash 4.17.21")));
    world
        .craft()
        .args(["why", "mime-types"])
        .assert()
        .success()
        .stdout(contains(
            "dependencies: express 4.21.2 → accepts 1.3.8 → mime-types",
        ));
    world
        .craft()
        .args(["why", "left-pad"])
        .assert()
        .success()
        .stdout("left-pad is not installed\n");

    let before = world.virtual_store().len();
    world
        .craft()
        .args(["remove", "express"])
        .assert()
        .success()
        .stdout(contains("- express 4.21.2"));

    assert_eq!(world.virtual_store(), ["lodash@4.17.21"]);
    assert!(before > 50);
    let lockfile = world.lockfile();
    assert_eq!(lockfile["packages"].as_object().map(|p| p.len()), Some(1));
    assert_eq!(
        world.node("console.log(require('lodash').chunk([1, 2, 3], 2).length)"),
        "2\n"
    );
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn dlx_runs_tools_without_touching_the_project() {
    let world = World::new(&json!({ "name": "untouched" }));

    world
        .craft()
        .args(["dlx", "cowsay@1.6.0", "hello", "craft"])
        .assert()
        .success()
        .stdout(contains("< hello craft >"));
    world
        .craft()
        .args(["x", "-p", "typescript@5.6.3", "tsc", "--version"])
        .assert()
        .success()
        .stdout("Version 5.6.3\n");
    world
        .craft()
        .args([
            "dlx",
            "-p",
            "typescript@5.6.3",
            "tsc",
            "--noEmit",
            "missing.ts",
        ])
        .assert()
        .code(2)
        .stdout(contains("TS6053"));

    assert_eq!(world.package_json(), json!({ "name": "untouched" }));
    assert!(!world.path().join("node_modules").exists());
    assert!(!world.path().join("craft.lock").exists());
}
