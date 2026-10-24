use serde_json::json;

use crate::common::World;

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn aliases_resolve_at_the_root_and_inside_dependencies() {
    let world = World::new(&json!({ "dependencies": { "@isaacs/cliui": "8.0.2" } }));

    world
        .craft()
        .args(["add", "sw@npm:string-width@4.2.3"])
        .assert()
        .success();

    assert_eq!(
        world.package_json()["dependencies"]["sw"],
        "npm:string-width@^4.2.3"
    );
    let output = world.node(
        "const ui = require('@isaacs/cliui')({ width: 20 }); ui.div('craft'); console.log(require('sw')('abc'), ui.toString().trim());",
    );
    assert_eq!(output, "3 craft\n");
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn git_and_tarball_dependencies_install_and_stay_locked() {
    let world = World::new(&json!({
        "dependencies": {
            "ms": "github:vercel/ms#2.1.3",
            "ms-range": "git+https://github.com/vercel/ms.git#semver:~2.1.2",
            "dequal": "https://registry.npmjs.org/dequal/-/dequal-2.0.3.tgz",
        },
    }));

    world.craft().arg("install").assert().success();

    let output = world.node(
        "const { dequal } = require('dequal'); console.log(require('ms')('1h'), require('ms-range')(60000), dequal({ a: [1] }, { a: [1] }));",
    );
    assert_eq!(output, "3600000 1m true\n");
    let lockfile = world.lockfile();
    let commits: Vec<&serde_json::Value> = lockfile["packages"]
        .as_object()
        .expect("packages")
        .values()
        .map(|package| &package["resolution"]["commit"])
        .filter(|commit| commit.is_string())
        .collect();
    assert_eq!(
        commits,
        [&json!("1c6264b795492e8fdecbc82cb8802fcfbfc08d26")]
    );

    world.remove("node_modules");
    world
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .success();
    assert_eq!(
        world
            .node("console.log(require('dequal/package.json').version, require('ms-range')(1000))"),
        "2.0.3 1s\n"
    );
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn installs_from_a_scoped_registry() {
    let world = World::new(&json!({ "type": "module" }));
    world.write(".npmrc", "@jsr:registry=https://npm.jsr.io\n");

    world
        .craft()
        .args(["add", "@jsr/std__path@^1.0.0"])
        .assert()
        .success();

    assert_eq!(
        world.node_module("import { join } from '@jsr/std__path'; console.log(join('a', 'b'));"),
        "a/b\n"
    );
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn local_packages_bring_their_registry_dependencies() {
    let world = World::new(&json!({ "name": "app" }));
    world.write(
        "libs/greet/package.json",
        &json!({ "name": "greet", "version": "1.0.0", "dependencies": { "kleur": "4.1.5" } })
            .to_string(),
    );
    world.write(
        "libs/greet/index.js",
        "module.exports = (name) => require('kleur').bold(`hi ${name}`);\n",
    );
    world.write(
        "libs/linked/package.json",
        &json!({ "name": "linked", "version": "1.0.0" }).to_string(),
    );
    world.write("libs/linked/index.js", "module.exports = 'linked';\n");

    world
        .craft()
        .args(["add", "./libs/greet", "link:libs/linked"])
        .assert()
        .success();

    let dependencies = &world.package_json()["dependencies"];
    assert_eq!(dependencies["greet"], "file:libs/greet");
    assert_eq!(dependencies["linked"], "link:libs/linked");
    assert_eq!(
        world
            .node("console.log(require('greet')('craft').includes('hi craft'), require('linked'))"),
        "true linked\n"
    );

    world.write("libs/linked/index.js", "module.exports = 'edited';\n");
    assert_eq!(world.node("console.log(require('linked'))"), "edited\n");
}
