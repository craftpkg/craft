use predicates::str::contains;
use serde_json::{Value, json};
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

/// `esbuild` with one optional binary package per platform, like the real one.
fn publish_esbuild(registry: &MockRegistry) {
    let platforms = [
        ("darwin-arm64", "darwin", "arm64", None),
        ("linux-x64", "linux", "x64", Some("glibc")),
        ("linux-x64-musl", "linux", "x64", Some("musl")),
    ];
    let mut esbuild = PackageFixture::new("esbuild", "0.25.0").file(
        "index.js",
        r#"
const platforms = ["darwin-arm64", "linux-x64", "linux-x64-musl"];
const found = platforms.find((p) => { try { require.resolve(`@esbuild/${p}`); return true; } catch { return false; } });
module.exports = { binary: found };
"#,
    );
    for (suffix, os, cpu, libc) in platforms {
        let mut binary = PackageFixture::new(&format!("@esbuild/{suffix}"), "0.25.0")
            .manifest_field("os", json!([os]))
            .manifest_field("cpu", json!([cpu]));
        if let Some(libc) = libc {
            binary = binary.manifest_field("libc", json!([libc]));
        }
        registry.publish(binary);
        esbuild = esbuild.optional_dependency(&format!("@esbuild/{suffix}"), "0.25.0");
    }
    registry.publish(esbuild);
}

fn craft_for(project: &Project, os: &str, cpu: &str, libc: &str) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .env("CRAFT_OS", os)
        .env("CRAFT_CPU", cpu)
        .env("CRAFT_LIBC", libc);
    command
}

fn binary(project: &Project) -> Value {
    project.require("esbuild").expect("esbuild loads")["binary"].clone()
}

#[test]
fn installs_the_binary_for_the_target_platform() {
    let registry = MockRegistry::start();
    publish_esbuild(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "esbuild": "0.25.0" } }),
    );

    craft_for(&project, "linux", "x64", "glibc")
        .arg("install")
        .assert()
        .success();

    assert_eq!(binary(&project), "linux-x64");
    let downloaded_binaries: Vec<String> = registry
        .requested_paths()
        .into_iter()
        .filter(|path| path.starts_with("/@esbuild/") && path.ends_with(".tgz"))
        .collect();
    assert_eq!(
        downloaded_binaries,
        ["/@esbuild/linux-x64/-/linux-x64-0.25.0.tgz"]
    );
}

#[test]
fn picks_the_musl_binary_on_musl() {
    let registry = MockRegistry::start();
    publish_esbuild(&registry);
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "esbuild": "0.25.0" } }),
    );

    craft_for(&project, "linux", "x64", "musl")
        .arg("install")
        .assert()
        .success();

    assert_eq!(binary(&project), "linux-x64-musl");
}

#[test]
fn locks_every_platform_so_the_lockfile_installs_anywhere() {
    let registry = MockRegistry::start();
    publish_esbuild(&registry);
    let linux = Project::new(
        &registry,
        json!({ "dependencies": { "esbuild": "0.25.0" } }),
    );
    craft_for(&linux, "linux", "x64", "glibc")
        .arg("install")
        .assert()
        .success();
    let lock = linux.read_file("craft.lock");
    let parsed: Value = serde_json::from_str(&lock).expect("craft.lock is JSON");
    assert_eq!(
        parsed["packages"]["@esbuild/darwin-arm64@0.25.0"]["os"],
        json!(["darwin"])
    );
    assert_eq!(
        parsed["packages"]["esbuild@0.25.0"]["optionalDependencies"]["@esbuild/linux-x64-musl"],
        "@esbuild/linux-x64-musl@0.25.0"
    );

    let mac = linux.sibling(json!({ "dependencies": { "esbuild": "0.25.0" } }));
    mac.write_file("craft.lock", &lock);
    let metadata_before = registry
        .requested_paths()
        .iter()
        .filter(|path| !path.ends_with(".tgz"))
        .count();
    craft_for(&mac, "darwin", "arm64", "")
        .args(["install", "--frozen-lockfile"])
        .assert()
        .success();

    let metadata_after = registry
        .requested_paths()
        .iter()
        .filter(|path| !path.ends_with(".tgz"))
        .count();
    assert_eq!(metadata_after, metadata_before);
    assert_eq!(binary(&mac), "darwin-arm64");
}

#[test]
fn skips_optional_dependencies_that_fail_to_install() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("watcher", "1.0.0")
            .optional_dependency("fsevents-gone", "^2.0.0")
            .file("index.js", "module.exports = { loaded: true };"),
    );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "watcher": "^1.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains(
            "warning: skipping optional dependency fsevents-gone@^2.0.0 of watcher",
        ));

    assert_eq!(
        project.require("watcher").expect("watcher loads")["loaded"],
        true
    );
}

#[test]
fn fails_when_a_required_dependency_does_not_support_the_platform() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("mac-only", "1.0.0").manifest_field("os", json!(["darwin"])));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "mac-only": "^1.0.0" } }),
    );

    craft_for(&project, "linux", "x64", "glibc")
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(
            "mac-only@1.0.0 does not support linux-x64-glibc, but the project requires it",
        ));
}
