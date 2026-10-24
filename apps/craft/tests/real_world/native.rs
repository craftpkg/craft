use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;

use crate::common::World;

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn platform_binaries_load_without_running_install_scripts() {
    let world = World::new(&json!({
        "dependencies": {
            "esbuild": "0.24.0",
            "@swc/core": "1.9.3",
            "lightningcss": "1.28.2",
        },
    }));

    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("not in trustedDependencies").and(contains("esbuild@0.24.0")));

    let output = world.node(
        r#"
const esbuild = require('esbuild');
const swc = require('@swc/core');
const css = require('lightningcss');
const js = esbuild.transformSync('let x: number = 1', { loader: 'ts' }).code.trim();
const swcOut = swc.transformSync('const y = () => 2', { jsc: { target: 'es5' } }).code;
const minified = css.transform({ filename: 'a.css', code: Buffer.from('a { color: #ff0000 }'), minify: true });
console.log(js, swcOut.includes('function') ? 'swc-ok' : 'swc-bad', minified.code.toString());
"#,
    );
    assert_eq!(output, "let x = 1; swc-ok a{color:red}\n");
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn trusting_a_package_runs_its_build() {
    let world = World::new(&json!({ "dependencies": { "bcrypt": "6.0.0" } }));
    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("bcrypt@6.0.0"));

    world
        .craft()
        .args(["trust", "bcrypt"])
        .assert()
        .success()
        .stdout(contains("Trusted bcrypt"));

    assert_eq!(
        world.package_json()["trustedDependencies"],
        json!(["bcrypt"])
    );
    let output = world.node(
        "const bcrypt = require('bcrypt'); const hash = bcrypt.hashSync('secret', 4); console.log(bcrypt.compareSync('secret', hash), bcrypt.compareSync('wrong', hash));",
    );
    assert_eq!(output, "true false\n");
    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("trustedDependencies").not());
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn installs_another_platforms_binaries_and_locks_every_platform() {
    let world = World::new(&json!({ "dependencies": { "esbuild": "0.24.0" } }));

    let mut install = world.craft();
    install
        .arg("install")
        .env("CRAFT_OS", "linux")
        .env("CRAFT_CPU", "x64")
        .env("CRAFT_LIBC", "glibc")
        .assert()
        .success();

    let store = world.virtual_store();
    assert!(store.contains(&"@esbuild+linux-x64@0.24.0".to_string()));
    assert!(
        !store
            .iter()
            .any(|entry| entry.starts_with("@esbuild+darwin"))
    );
    let lockfile = world.lockfile();
    let packages = lockfile["packages"].as_object().expect("packages");
    for platform in ["darwin-arm64", "linux-x64", "win32-x64", "linux-arm64"] {
        assert!(packages.contains_key(&format!("@esbuild/{platform}@0.24.0")));
    }
    assert_eq!(
        packages["@esbuild/linux-x64@0.24.0"]["os"],
        json!(["linux"])
    );
}
