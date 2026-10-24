use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn gives_each_subtree_the_peer_its_parent_provides() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react", "18.3.1"))
        .publish(PackageFixture::new("react", "19.2.0"))
        .publish(PackageFixture::new("ui", "1.0.0").peer_dependency("react", "*"))
        .publish(
            PackageFixture::new("legacy-app", "1.0.0")
                .dependency("react", "^18.0.0")
                .dependency("ui", "^1.0.0")
                .file(
                    "index.js",
                    r#"module.exports = { react: require("ui").dependencies.react };"#,
                ),
        )
        .publish(
            PackageFixture::new("modern-app", "1.0.0")
                .dependency("react", "^19.0.0")
                .dependency("ui", "^1.0.0")
                .file(
                    "index.js",
                    r#"module.exports = { react: require("ui").dependencies.react };"#,
                ),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "legacy-app": "^1.0.0", "modern-app": "^1.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.require("legacy-app").expect("legacy-app loads")["react"],
        "18.3.1"
    );
    assert_eq!(
        project.require("modern-app").expect("modern-app loads")["react"],
        "19.2.0"
    );
    let virtual_store = project.path().join("node_modules/.craft");
    assert!(virtual_store.join("ui@1.0.0_react@18.3.1").is_dir());
    assert!(virtual_store.join("ui@1.0.0_react@19.2.0").is_dir());
}

#[test]
fn installs_a_peer_nobody_provides() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react", "19.2.0"))
        .publish(
            PackageFixture::new("@tanstack/react-query", "5.90.10")
                .peer_dependency("react", "^18 || ^19"),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "@tanstack/react-query": "^5.0.0" } }),
    );

    project.craft().arg("install").assert().success();

    let query = project
        .require("@tanstack/react-query")
        .expect("react-query loads");
    assert_eq!(query["dependencies"]["react"], "19.2.0");
    assert!(project.require("react").is_err());
}

#[test]
fn warns_when_a_peer_range_is_not_met() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react", "17.0.2"))
        .publish(
            PackageFixture::new("@tanstack/react-query", "5.90.10")
                .peer_dependency("react", "^18 || ^19"),
        );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "react": "^17.0.0", "@tanstack/react-query": "^5.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains(
            "warning: @tanstack/react-query@5.90.10 wants peer react@^18 || ^19 but gets 17.0.2",
        ));

    let query = project
        .require("@tanstack/react-query")
        .expect("react-query loads");
    assert_eq!(query["dependencies"]["react"], "17.0.2");
}

#[test]
fn leaves_optional_peers_out_unless_provided() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("react-native", "0.76.0"))
        .publish(
            PackageFixture::new("ui-kit", "1.0.0")
                .peer_dependency("react-native", "*")
                .manifest_field(
                    "peerDependenciesMeta",
                    json!({ "react-native": { "optional": true } }),
                )
                .file("index.js", "module.exports = { loaded: true };"),
        );
    let project = Project::new(&registry, json!({ "dependencies": { "ui-kit": "^1.0.0" } }));

    project.craft().arg("install").assert().success().stderr("");

    assert_eq!(
        project.require("ui-kit").expect("ui-kit loads")["loaded"],
        true
    );
    let installed_react_native = std::fs::read_dir(project.path().join("node_modules/.craft"))
        .expect("virtual store exists")
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("react-native@")
        });
    assert!(!installed_react_native);
    let requested_react_native = registry
        .requested_paths()
        .iter()
        .any(|path| path.contains("react-native"));
    assert!(!requested_react_native);
}
