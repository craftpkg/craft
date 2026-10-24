use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;

use crate::common::World;

const EXPRESS_SERVER: &str = r#"
const express = require('express');
const http = require('http');
const app = express();
app.get('/', (req, res) => res.send('hello from craft'));
const server = app.listen(0, () => {
  http.get({ port: server.address().port, path: '/' }, (res) => {
    let body = '';
    res.on('data', (chunk) => (body += chunk));
    res.on('end', () => { console.log(body); server.close(); });
  });
});
"#;

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn express_app_installs_reinstalls_and_serves() {
    let world = World::isolated(&json!({ "name": "api", "dependencies": { "express": "4.21.2" } }));

    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains("+ express 4.21.2"));
    assert_eq!(world.node(EXPRESS_SERVER), "hello from craft\n");
    assert!(
        world
            .virtual_store()
            .contains(&"body-parser@1.20.3".to_string())
    );

    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout("Already up to date\n");

    world.remove("node_modules");
    world
        .craft()
        .args(["--offline", "install"])
        .assert()
        .success();
    assert_eq!(world.node(EXPRESS_SERVER), "hello from craft\n");

    let lockfile = world.read("craft.lock");
    let mut ci = world.craft();
    ci.arg("ci").env("CI", "true").assert().success();
    assert_eq!(world.read("craft.lock"), lockfile);
    assert_eq!(world.node(EXPRESS_SERVER), "hello from craft\n");
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn a_fresh_checkout_installs_exactly_what_was_locked() {
    let package_json = json!({ "dependencies": { "express": "^4.0.0", "lodash": "^4.0.0" } });
    let original = World::new(&package_json);
    original.craft().arg("install").assert().success();

    let checkout = World::isolated(&package_json);
    checkout.write("craft.lock", &original.read("craft.lock"));
    checkout
        .craft()
        .args(["install", "--frozen-lockfile"])
        .assert()
        .success();

    assert_eq!(checkout.read("craft.lock"), original.read("craft.lock"));
    assert_eq!(checkout.virtual_store(), original.virtual_store());
    let resolve_express = "console.log(require('express/package.json').version)";
    assert_eq!(
        checkout.node(resolve_express),
        original.node(resolve_express)
    );
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn the_same_package_json_locks_the_same_everywhere() {
    let package_json = json!({
        "dependencies": { "react": "19.1.0", "react-dom": "19.1.0", "debug": "4.4.0" },
        "devDependencies": { "typescript": "5.6.3" },
    });
    let first = World::new(&package_json);
    let second = World::isolated(&package_json);

    first.craft().arg("install").assert().success();
    second.craft().arg("install").assert().success();

    assert_eq!(first.read("craft.lock"), second.read("craft.lock"));
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn production_installs_leave_dev_tools_out() {
    let world = World::new(&json!({
        "dependencies": { "express": "4.21.2" },
        "devDependencies": { "typescript": "5.6.3" },
    }));

    world
        .craft()
        .args(["install", "--production"])
        .assert()
        .success();
    assert!(world.path().join("node_modules/express").exists());
    assert!(!world.path().join("node_modules/typescript").exists());
    assert!(!world.path().join("node_modules/.bin/tsc").exists());

    world.craft().arg("install").assert().success();
    world
        .craft()
        .args(["tsc", "--version"])
        .assert()
        .success()
        .stdout("Version 5.6.3\n");
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn init_add_and_run_a_formatter() {
    let world = World::new(&json!({}));
    world.remove("package.json");

    world.craft().arg("init").assert().success();
    world
        .craft()
        .args(["add", "-D", "-E", "prettier@3.3.3"])
        .assert()
        .success()
        .stdout(contains("+ prettier 3.3.3 (dev)"));
    assert_eq!(world.package_json()["devDependencies"]["prettier"], "3.3.3");

    let mut package_json = world.package_json();
    package_json["scripts"] = json!({
        "prefmt": "echo before > order.txt",
        "fmt": "prettier --write",
        "postfmt": "echo after >> order.txt",
    });
    world.write("package.json", &package_json.to_string());
    world.write("ugly.js", "const  x = {a:1,\n b:2}\n");

    world
        .craft()
        .args(["run", "fmt", "ugly.js"])
        .assert()
        .success();

    assert_eq!(world.read("ugly.js"), "const x = { a: 1, b: 2 };\n");
    assert_eq!(world.read("order.txt"), "before\nafter\n");
    world
        .craft()
        .arg("run")
        .assert()
        .success()
        .stdout(contains("fmt"));
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn offline_installs_say_what_is_missing() {
    let world = World::isolated(&json!({ "dependencies": { "ms": "2.1.3" } }));
    world.craft().arg("install").assert().success();
    world.remove("node_modules");

    world
        .craft()
        .args(["--offline", "install"])
        .assert()
        .success();
    world
        .craft()
        .args(["--offline", "add", "left-pad"])
        .assert()
        .failure()
        .stderr(contains("is not cached and craft is offline"));
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn a_missing_package_names_the_registry() {
    let world = World::new(&json!({
        "dependencies": { "craft-real-world-no-such-package-0d1f": "^1.0.0" },
    }));

    world.craft().arg("install").assert().failure().stderr(
        "error: failed to install craft-real-world-no-such-package-0d1f@^1.0.0\n  caused by: package craft-real-world-no-such-package-0d1f not found in https://registry.npmjs.org\n",
    );
    assert!(!world.path().join("craft.lock").exists());
    assert!(
        !world
            .path()
            .join("node_modules/craft-real-world-no-such-package-0d1f")
            .exists()
    );
    world
        .craft()
        .args(["why", "ms"])
        .assert()
        .failure()
        .stderr(contains("craft.lock is missing").and(contains("panicked").not()));
}
