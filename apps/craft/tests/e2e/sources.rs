use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

#[test]
fn installs_file_dependencies() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.1.0".to_string())
    );
}

#[test]
fn installs_the_registry_dependencies_of_a_file_dependency() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:libs/local-lib" } }),
    );
    project.write_file(
        "libs/local-lib/package.json",
        r#"{ "name": "local-lib", "version": "0.1.0", "dependencies": { "ms": "^2.0.0" } }"#,
    );
    project.write_file(
        "libs/local-lib/index.js",
        r#"module.exports = { version: require("ms").version };"#,
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("2.1.3".to_string())
    );
}

#[test]
fn copies_file_dependencies_again_on_every_install() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "file:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");
    project.craft().arg("install").assert().success();

    write_local_lib(&project, "0.2.0");
    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.2.0".to_string())
    );
}

#[test]
fn links_link_dependencies_in_place() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "local-lib": "link:./libs/local-lib" } }),
    );
    write_local_lib(&project, "0.1.0");

    project.craft().arg("install").assert().success();
    write_local_lib(&project, "0.2.0");

    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.2.0".to_string())
    );
    let link = project.path().join("node_modules/local-lib");
    assert!(link.is_symlink());
    assert_eq!(
        link.canonicalize().expect("link resolves"),
        project
            .path()
            .join("libs/local-lib")
            .canonicalize()
            .expect("local-lib exists")
    );
}

#[test]
fn installs_file_tarball_dependencies() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "ms": "file:vendor/ms-2.1.3.tgz" } }),
    );
    let tarball = PackageFixture::new("ms", "2.1.3").tarball();
    std::fs::create_dir_all(project.path().join("vendor")).expect("create vendor");
    std::fs::write(project.path().join("vendor/ms-2.1.3.tgz"), tarball).expect("write tarball");

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn installs_tarball_url_dependencies() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "debug": registry.tarball_url("debug", "4.4.0") } }),
    );

    project.craft().arg("install").assert().success();

    let debug = project.require("debug").expect("debug loads");
    assert_eq!(debug["version"], "4.4.0");
    assert_eq!(debug["dependencies"]["ms"], "2.1.3");
}

#[test]
fn adds_a_local_directory_under_its_package_name() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "name": "app" }));
    write_local_lib(&project, "0.1.0");

    project
        .craft()
        .args(["add", "./libs/local-lib"])
        .assert()
        .success();

    assert_eq!(
        project.package_json()["dependencies"]["local-lib"],
        "file:libs/local-lib"
    );
    assert_eq!(
        project.required_version("local-lib"),
        Ok("0.1.0".to_string())
    );
}

#[test]
fn adds_a_tarball_url_verbatim() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "name": "app" }));
    let url = registry.tarball_url("ms", "2.1.3");

    project.craft().args(["add", &url]).assert().success();

    assert_eq!(project.package_json()["dependencies"]["ms"], url.as_str());
    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn installs_git_dependencies_by_tag_range_and_branch() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    let url = git_repository(&project);
    project.write_file(
        "package.json",
        &json!({
            "dependencies": {
                "tagged": format!("git+{url}#v1.0.0"),
                "ranged": format!("git+{url}#semver:^1.0.0"),
                "head": format!("git+{url}"),
            }
        })
        .to_string(),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("tagged"), Ok("1.0.0".to_string()));
    assert_eq!(project.required_version("ranged"), Ok("1.1.0".to_string()));
    assert_eq!(
        project.required_version("head"),
        Ok("2.0.0-dev".to_string())
    );
}

#[test]
fn installs_the_registry_dependencies_of_a_git_dependency() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({}));
    let repository = project.path().join("repos/with-deps");
    project.write_file(
        "repos/with-deps/package.json",
        r#"{ "name": "with-deps", "version": "1.0.0", "dependencies": { "ms": "^2.0.0" } }"#,
    );
    project.write_file(
        "repos/with-deps/index.js",
        r#"module.exports = { version: require("ms").version };"#,
    );
    git(
        &repository,
        &["init", "--quiet", "--initial-branch", "main"],
    );
    commit_all(&repository, "initial");
    project.write_file(
        "package.json",
        &json!({ "dependencies": { "with-deps": format!("git+file://{}", repository.display()) } })
            .to_string(),
    );

    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("with-deps"),
        Ok("2.1.3".to_string())
    );
}

#[test]
fn adds_a_git_dependency_under_its_package_name() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "name": "app" }));
    let spec = format!("git+{}#v1.0.0", git_repository(&project));

    project.craft().args(["add", &spec]).assert().success();

    assert_eq!(
        project.package_json()["dependencies"]["git-lib"],
        spec.as_str()
    );
    assert_eq!(project.required_version("git-lib"), Ok("1.0.0".to_string()));
}

#[test]
fn reports_a_git_ref_that_does_not_exist() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    let url = git_repository(&project);
    project.write_file(
        "package.json",
        &json!({ "dependencies": { "git-lib": format!("git+{url}#no-such-branch") } }).to_string(),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(predicates::str::contains(format!(
            "no commit in {url} matches no-such-branch"
        )));
}

#[test]
fn keeps_a_git_dependency_on_its_locked_commit() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({}));
    let url = git_repository(&project);
    project.write_file(
        "package.json",
        &json!({ "dependencies": { "git-lib": format!("git+{url}#main") } }).to_string(),
    );
    project.craft().arg("install").assert().success();

    let repository = project.path().join("repos/git-lib");
    project.write_file(
        "repos/git-lib/package.json",
        r#"{ "name": "git-lib", "version": "3.0.0" }"#,
    );
    commit_all(&repository, "3.0.0");
    std::fs::remove_dir_all(project.path().join("node_modules")).expect("remove node_modules");
    project.craft().arg("install").assert().success();

    assert_eq!(
        project.required_version("git-lib"),
        Ok("2.0.0-dev".to_string())
    );
}

/// A repository with `v1.0.0` (lightweight tag), `v1.1.0` (annotated tag) and an untagged
/// `2.0.0-dev` commit on `main`, returned as a `file://` URL.
fn git_repository(project: &Project) -> String {
    let repository = project.path().join("repos/git-lib");
    std::fs::create_dir_all(&repository).expect("create repository");
    git(
        &repository,
        &["init", "--quiet", "--initial-branch", "main"],
    );

    for (version, tag) in [
        ("1.0.0", Some(false)),
        ("1.1.0", Some(true)),
        ("2.0.0-dev", None),
    ] {
        project.write_file(
            "repos/git-lib/package.json",
            &format!(r#"{{ "name": "git-lib", "version": "{version}" }}"#),
        );
        project.write_file(
            "repos/git-lib/index.js",
            &format!(r#"module.exports = {{ version: "{version}" }};"#),
        );
        commit_all(&repository, version);
        match tag {
            Some(true) => git(
                &repository,
                &["tag", "-a", &format!("v{version}"), "-m", version],
            ),
            Some(false) => git(&repository, &["tag", &format!("v{version}")]),
            None => {}
        }
    }
    file_url(&repository)
}

/// `file:///C:/…` on Windows, where `file://C:\…` is not a URL git understands.
fn file_url(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let separator = if path.starts_with('/') { "" } else { "/" };
    format!("file://{separator}{path}")
}

fn commit_all(repository: &std::path::Path, message: &str) {
    git(repository, &["add", "--all"]);
    git(repository, &["commit", "--quiet", "--message", message]);
}

fn git(repository: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=craft",
            "-c",
            "user.email=craft@example.com",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(repository)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git is required to run the e2e tests");
    assert!(status.success(), "git {args:?} failed");
}

fn write_local_lib(project: &Project, version: &str) {
    project.write_file(
        "libs/local-lib/package.json",
        &format!(r#"{{ "name": "local-lib", "version": "{version}" }}"#),
    );
    project.write_file(
        "libs/local-lib/index.js",
        &format!(r#"module.exports = {{ name: "local-lib", version: "{version}" }};"#),
    );
}
