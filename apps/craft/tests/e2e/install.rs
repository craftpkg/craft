use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn installs_dependencies_and_their_subtree() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project.craft().arg("install").assert().success();

    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["version"], "4.4.0");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn installs_dev_dependencies() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("typescript", "5.9.3"));
    let project = Project::new(
        &registry,
        json!({ "devDependencies": { "typescript": "^5.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("typescript"),
        Ok("5.9.3".to_string())
    );
}

#[test]
fn defaults_to_install_without_a_command() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project.craft().assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn picks_the_highest_version_satisfying_the_range() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.0.0"))
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("ms", "3.0.0"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn links_different_versions_of_one_package_where_each_is_required() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("ms", "3.0.0"))
        .publish(PackageFixture::new("legacy", "1.0.0").dependency("ms", "^2.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "ms": "^3.0.0", "legacy": "1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("3.0.0".to_string()));
    let legacy = project.require("legacy").expect("legacy loads");
    assert_eq!(legacy["dependencies"]["ms"], "2.1.3");
}

#[test]
fn installs_cyclic_dependencies() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("chicken", "1.0.0").dependency("egg", "^1.0.0"))
        .publish(PackageFixture::new("egg", "1.0.0").dependency("chicken", "^1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "chicken": "^1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    let chicken = project.require("chicken").expect("chicken loads");
    assert_eq!(chicken["dependencies"]["egg"], "1.0.0");
}

#[test]
fn downloads_each_version_once_for_overlapping_ranges() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("a", "1.0.0").dependency("ms", "^2.0.0"))
        .publish(PackageFixture::new("b", "1.0.0").dependency("ms", "~2.1.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "a": "1.0.0", "b": "1.0.0", "ms": "2.1.3" } }),
    );

    project.craft().arg("install").assert().success();

    let ms_downloads = registry
        .requested_paths()
        .iter()
        .filter(|path| path.ends_with("/ms-2.1.3.tgz"))
        .count();
    assert_eq!(ms_downloads, 1);
    assert_eq!(
        project.require("a").expect("a loads")["dependencies"]["ms"],
        "2.1.3"
    );
    assert_eq!(
        project.require("b").expect("b loads")["dependencies"]["ms"],
        "2.1.3"
    );
}

#[test]
fn installs_scoped_packages() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("@tanstack/query-core", "5.90.10"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "@tanstack/query-core": "^5.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("@tanstack/query-core"),
        Ok("5.90.10".to_string())
    );
}

#[test]
fn links_bins_of_direct_dependencies() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(&registry, json!({ "dependencies": { "cowsay": "^1.0.0" } }));

    project.craft().arg("install").assert().success();

    assert!(project.path().join("node_modules/.bin/cowsay").exists());
    project
        .craft()
        .arg("cowsay")
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));
}

#[test]
fn reuses_cached_tarballs_across_projects() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let first = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    let second = first.sibling(json!({ "dependencies": { "ms": "^2.0.0" } }));

    first.craft().arg("install").assert().success();
    second.craft().arg("install").assert().success();

    let tarball_downloads = registry
        .requested_paths()
        .iter()
        .filter(|path| path.ends_with(".tgz"))
        .count();
    assert_eq!(tarball_downloads, 1);
    assert_eq!(second.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn fails_for_a_package_missing_from_the_registry() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "dependencies": { "nope": "^1.0.0" } }));

    project.craft().arg("install").assert().failure();
}

#[test]
fn fails_without_package_json() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains("no package.json found"));
}

#[test]
fn resolves_dist_tags() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("next", "15.0.0"))
        .publish(PackageFixture::new("next", "16.0.0-canary.1"))
        .set_dist_tag("next", "latest", "15.0.0")
        .set_dist_tag("next", "canary", "16.0.0-canary.1");
    let project = Project::new(&registry, json!({ "dependencies": { "next": "canary" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("next"),
        Ok("16.0.0-canary.1".to_string())
    );
}

#[test]
fn prefers_latest_dist_tag_within_range() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("ms", "2.2.0"))
        .set_dist_tag("ms", "latest", "2.1.3");
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn links_aliased_root_dependencies() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("string-width", "4.2.3"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "sw": "npm:string-width@^4.2.0" } }),
    );

    project.craft().arg("install").assert().success();

    let sw = project.require("sw").expect("alias loads");
    assert_eq!(sw["name"], "string-width");
    assert_eq!(sw["version"], "4.2.3");
}

#[test]
fn links_aliased_transitive_dependencies() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("string-width", "4.2.3"))
        .publish(
            PackageFixture::new("@isaacs/cliui", "8.0.2")
                .dependency("string-width-cjs", "npm:string-width@^4.2.0"),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "@isaacs/cliui": "8.0.2" } }),
    );

    project.craft().arg("install").assert().success();

    let cliui = project.require("@isaacs/cliui").expect("cliui loads");
    assert_eq!(cliui["dependencies"]["string-width-cjs"], "4.2.3");
}

#[test]
fn installs_tarballs_with_a_non_standard_root_dir() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("@types/node", "22.20.5")
            .root_dir("node")
            .file("index.d.ts", "export {};"),
    );
    let project = Project::new(
        &registry,
        json!({ "devDependencies": { "@types/node": "^22.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert!(
        project
            .path()
            .join("node_modules/@types/node/index.d.ts")
            .exists()
    );
}

#[test]
fn names_the_missing_package_in_the_error() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "this-package-does-not-exist": "^1.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains("this-package-does-not-exist"))
        .stderr(contains("not found"));
}

#[test]
fn names_the_dependency_chain_of_a_missing_transitive_package() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("app-framework", "4.0.0").dependency("body-parser", "^1.0.0"))
        .publish(PackageFixture::new("body-parser", "1.2.0").dependency("ghost", "^1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "app-framework": "^4.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(
            "failed to install ghost@^1.0.0 (required by app-framework@4.0.0 > body-parser@1.2.0)",
        ))
        .stderr(contains("package ghost not found in"));
}

#[test]
fn reports_a_missing_tarball_without_caching_the_error_body() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .remove_tarball("ms", "2.1.3");
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains("failed to download ms@2.1.3"))
        .stderr(contains("HTTP 404"));

    assert!(!project.craft_home().join("packages/ms-2.1.3.tgz").exists());
}

#[test]
fn rejects_tarballs_with_mismatched_integrity() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3").wrong_integrity());
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains("failed to download ms@2.1.3"))
        .stderr(contains("tarball integrity mismatch: expected sha512-AAAA"));

    let packages = project.craft_home().join("packages");
    let cached_entries = std::fs::read_dir(&packages).map_or(0, |entries| entries.count());
    assert_eq!(cached_entries, 0);
    assert!(!project.path().join("node_modules/ms").exists());
}

#[test]
fn replaces_a_corrupted_cached_tarball() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let first = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    first.craft().arg("install").assert().success();
    let packages = first.craft_home().join("packages");
    std::fs::write(packages.join("ms-2.1.3.tgz"), b"corrupted").expect("corrupt tarball");
    std::fs::remove_dir_all(packages.join("ms-2.1.3")).expect("remove extracted package");
    let second = first.sibling(json!({ "dependencies": { "ms": "^2.0.0" } }));

    second.craft().arg("install").assert().success();

    assert_eq!(second.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
#[ignore = "fixed by 1.8: a dangling symlink in node_modules fails the install with EEXIST"]
fn replaces_dangling_symlinks_in_node_modules() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    std::fs::create_dir_all(project.path().join("node_modules")).expect("create node_modules");
    symlink_dir("/nonexistent/ms", &project.path().join("node_modules/ms"));

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
#[ignore = "fixed by 1.13: file: dependencies are reported as unsupported"]
fn installs_file_dependencies() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:./libs/local-lib" } }),
    );
    project.write_file(
        "libs/local-lib/package.json",
        r#"{ "name": "local-lib", "version": "0.1.0" }"#,
    );
    project.write_file(
        "libs/local-lib/index.js",
        r#"module.exports = { name: "local-lib", version: "0.1.0" };"#,
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.1.0".to_string())
    );
}

#[test]
#[ignore = "fixed by 1.14: a string bin on a scoped package is linked as .bin/@scope/name"]
fn links_string_bins_of_scoped_packages() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("@scope/tool", "1.0.0").string_bin("cli.js"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "@scope/tool": "^1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    project
        .craft()
        .arg("tool")
        .assert()
        .success()
        .stdout(contains("tool @scope/tool@1.0.0"));
}

#[test]
#[ignore = "fixed by 2.3: peer dependencies are never linked"]
fn resolves_peer_dependencies_from_the_project() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react", "19.2.0"))
        .publish(
            PackageFixture::new("@tanstack/react-query", "5.90.10")
                .peer_dependency("react", "^18 || ^19"),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "@tanstack/react-query": "^5.0.0", "react": "^19.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    let query = project
        .require("@tanstack/react-query")
        .expect("react-query loads");
    assert_eq!(query["dependencies"]["react"], "19.2.0");
}

#[test]
fn ignores_leftovers_of_an_interrupted_install() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    let packages = project.craft_home().join("packages");
    let partial_extraction = packages.join("ms-2.1.3/.extract-a1b2c3");
    std::fs::create_dir_all(&partial_extraction).expect("create partial extraction");
    std::fs::write(partial_extraction.join("package.json"), "{").expect("write partial file");
    std::fs::write(packages.join(".tmpA1b2C3"), b"\x1f\x8b").expect("write partial download");

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn reuses_extracted_packages_without_downloading_again() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let first = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    first.craft().arg("install").assert().success();
    std::fs::remove_file(first.craft_home().join("packages/ms-2.1.3.tgz")).expect("remove tarball");
    let second = first.sibling(json!({ "dependencies": { "ms": "^2.0.0" } }));

    second.craft().arg("install").assert().success();

    let tarball_requests = registry
        .requested_paths()
        .iter()
        .filter(|path| path.ends_with(".tgz"))
        .count();
    assert_eq!(tarball_requests, 1);
    assert_eq!(second.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
#[ignore = "fixed by 2.2: installs sharing a craft home race on the global cache"]
fn concurrent_installs_sharing_a_craft_home_succeed() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let first = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));
    first.craft().arg("install").assert().success();
    let projects: Vec<Project> = (0..6)
        .map(|_| first.sibling(json!({ "dependencies": { "debug": "^4.0.0" } })))
        .collect();

    let children: Vec<_> = projects
        .iter()
        .map(|project| {
            project
                .craft_process()
                .arg("install")
                .spawn()
                .expect("spawn craft install")
        })
        .collect();

    for child in children {
        let status = child
            .wait_with_output()
            .expect("craft install exits")
            .status;
        assert!(status.success());
    }
}

#[test]
#[ignore = "fixed by 4.1/4.2: optional platform-specific dependencies are skipped"]
fn installs_optional_dependencies_matching_the_host_platform() {
    let host = format!("{}-{}", node_platform(), node_arch());
    let registry = MockRegistry::start();
    registry
        .publish(
            PackageFixture::new(&format!("@esbuild/{host}"), "0.25.0")
                .manifest_field("os", json!([node_platform()]))
                .manifest_field("cpu", json!([node_arch()])),
        )
        .publish(
            PackageFixture::new("@esbuild/other-platform", "0.25.0")
                .manifest_field("os", json!(["aix"]))
                .manifest_field("cpu", json!(["ppc64"])),
        )
        .publish(
            PackageFixture::new("esbuild", "0.25.0")
                .optional_dependency(&format!("@esbuild/{host}"), "0.25.0")
                .optional_dependency("@esbuild/other-platform", "0.25.0")
                .file(
                    "index.js",
                    &format!(
                        r#"module.exports = {{ name: "esbuild", version: "0.25.0", binary: require("@esbuild/{host}").version }};"#
                    ),
                ),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "esbuild": "0.25.0" } }),
    );

    project.craft().arg("install").assert().success();

    let esbuild = project
        .require("esbuild")
        .expect("esbuild loads its platform binary");
    assert_eq!(esbuild["binary"], "0.25.0");
}

fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "ia32",
        other => other,
    }
}

#[cfg(unix)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("create dangling symlink");
}

#[cfg(windows)]
fn symlink_dir(target: &str, link: &std::path::Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("create dangling symlink");
}
