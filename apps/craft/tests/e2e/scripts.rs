use predicates::str::contains;
use serde_json::{Value, json};
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

/// A package whose `postinstall` records that it ran, from inside the package directory.
fn native(name: &str, postinstall: &str) -> PackageFixture {
    PackageFixture::new(name, "1.0.0")
        .manifest_field("scripts", json!({ "postinstall": postinstall }))
}

const WRITE_BUILT: &str =
    r#"node -e "require('fs').writeFileSync('built.txt', process.env.npm_package_name)""#;

fn entry_file(project: &Project, entry: &str, name: &str, file: &str) -> std::path::PathBuf {
    project
        .path()
        .join("node_modules/.craft")
        .join(entry)
        .join("node_modules")
        .join(name)
        .join(file)
}

fn with_trusted(dependencies: Value, trusted: &[&str]) -> Value {
    json!({ "dependencies": dependencies, "trustedDependencies": trusted })
}

#[test]
fn skips_scripts_of_untrusted_dependencies() {
    let registry = MockRegistry::start();
    registry.publish(native("native", WRITE_BUILT));
    let project = Project::new(&registry, json!({ "dependencies": { "native": "^1.0.0" } }));

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains(
            "warning: skipped install scripts of 1 package not in trustedDependencies: native@1.0.0",
        ));

    assert!(!entry_file(&project, "native@1.0.0", "native", "built.txt").exists());
}

#[test]
fn runs_scripts_of_trusted_dependencies_once_without_touching_the_store() {
    let registry = MockRegistry::start();
    registry.publish(native("native", WRITE_BUILT));
    let project = Project::new(
        &registry,
        with_trusted(json!({ "native": "^1.0.0" }), &["native"]),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains("Ran install scripts of native@1.0.0"));

    let built = entry_file(&project, "native@1.0.0", "native", "built.txt");
    assert_eq!(
        std::fs::read_to_string(&built).expect("built.txt"),
        "native"
    );
    let store_has_build_output = project
        .store_entries()
        .iter()
        .any(|entry| project.store_dir().join(entry).join("built.txt").exists());
    assert!(!store_has_build_output);

    std::fs::remove_file(&built).expect("remove built.txt");
    std::fs::remove_file(project.path().join("node_modules/.craft/state")).expect("force relink");
    project.craft().arg("install").assert().success();
    assert!(!built.exists());
}

#[test]
fn builds_a_dependency_once_it_is_trusted() {
    let registry = MockRegistry::start();
    registry.publish(native("native", WRITE_BUILT));
    let project = Project::new(&registry, json!({ "dependencies": { "native": "^1.0.0" } }));
    project.craft().arg("install").assert().success();

    project.write_file(
        "package.json",
        &with_trusted(json!({ "native": "^1.0.0" }), &["native"]).to_string(),
    );
    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stdout(contains("Ran install scripts of native@1.0.0"));

    assert!(entry_file(&project, "native@1.0.0", "native", "built.txt").exists());
}

#[test]
fn builds_dependencies_first_with_their_bins_on_path() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"))
        .publish(native("base", WRITE_BUILT).dependency("cowsay", "^1.0.0"))
        .publish(
            native(
                "addon",
                r#"node -e "require('fs').writeFileSync('built.txt', require('fs').readFileSync(require.resolve('base/package.json').replace('package.json', 'built.txt')))" && cowsay > cow.txt"#,
            )
            .dependency("base", "^1.0.0")
            .dependency("cowsay", "^1.0.0"),
        );
    let project = Project::new(
        &registry,
        with_trusted(json!({ "addon": "^1.0.0" }), &["addon", "base"]),
    );

    project.craft().arg("install").assert().success();

    let addon_built = entry_file(&project, "addon@1.0.0", "addon", "built.txt");
    assert_eq!(
        std::fs::read_to_string(addon_built).expect("addon built"),
        "base"
    );
    let cow = entry_file(&project, "addon@1.0.0", "addon", "cow.txt");
    assert!(
        std::fs::read_to_string(cow)
            .expect("cow.txt")
            .contains("cowsay cowsay@1.6.0")
    );
}

#[test]
fn fails_the_install_when_a_trusted_build_fails() {
    let registry = MockRegistry::start();
    registry.publish(native("broken", "echo compiling && exit 7"));
    let project = Project::new(
        &registry,
        with_trusted(json!({ "broken": "^1.0.0" }), &["broken"]),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(
            "postinstall script of broken@1.0.0 failed with exit code 7:\ncompiling",
        ));

    assert!(
        !project
            .path()
            .join("node_modules/.craft/broken@1.0.0")
            .exists()
    );
}

#[test]
fn skips_optional_dependencies_whose_build_fails() {
    let registry = MockRegistry::start();
    registry.publish(native("broken", "exit 1")).publish(
        PackageFixture::new("watcher", "1.0.0")
            .optional_dependency("broken", "^1.0.0")
            .file("index.js", "module.exports = { loaded: true };"),
    );
    let project = Project::new(
        &registry,
        with_trusted(json!({ "watcher": "^1.0.0" }), &["broken"]),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("warning: skipping optional broken@1.0.0"));

    assert_eq!(
        project.require("watcher").expect("watcher loads")["loaded"],
        true
    );
}

#[test]
fn ignore_scripts_runs_nothing() {
    let registry = MockRegistry::start();
    registry.publish(native("native", WRITE_BUILT));
    let mut package_json = with_trusted(json!({ "native": "^1.0.0" }), &["native"]);
    package_json["scripts"] = json!({ "postinstall": "touch project-built.txt" });
    let project = Project::new(&registry, package_json);

    project
        .craft()
        .args(["install", "--ignore-scripts"])
        .assert()
        .success()
        .stderr("");

    assert!(!entry_file(&project, "native@1.0.0", "native", "built.txt").exists());
    assert!(!project.path().join("project-built.txt").exists());
}

#[test]
fn runs_the_projects_own_lifecycle_scripts_in_order() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(
        &registry,
        json!({
            "dependencies": { "ms": "^2.0.0" },
            "scripts": {
                "preinstall": "echo preinstall >> lifecycle.txt",
                "postinstall": "echo postinstall >> lifecycle.txt",
                "prepare": "echo prepare >> lifecycle.txt",
            },
        }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.read_file("lifecycle.txt"),
        "preinstall\npostinstall\nprepare\n"
    );
}
