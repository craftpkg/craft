use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::Project;

const TOKEN: &str = "npm_SecretTokenValue";

fn host_of(registry: &MockRegistry) -> String {
    registry.url().trim_start_matches("http://").to_string()
}

#[test]
fn authenticates_with_a_token_from_the_project_npmrc() {
    let registry = MockRegistry::start();
    registry
        .require_token(TOKEN)
        .publish(PackageFixture::new("private-lib", "1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "private-lib": "^1.0.0" } }),
    );
    project.write_file(
        ".npmrc",
        &format!("//{}/:_authToken=${{PRIVATE_TOKEN}}\n", host_of(&registry)),
    );

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("PRIVATE_TOKEN", TOKEN)
        .assert()
        .success();

    assert_eq!(
        project.required_version("private-lib"),
        Ok("1.0.0".to_string())
    );
}

#[test]
fn reports_missing_credentials() {
    let registry = MockRegistry::start();
    registry
        .require_token(TOKEN)
        .publish(PackageFixture::new("private-lib", "1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "private-lib": "^1.0.0" } }),
    );

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(
            "denied access to private-lib (HTTP 401); check your registry credentials",
        ));
}

#[test]
fn fetches_scoped_packages_from_their_registry_with_their_token_only() {
    let public = MockRegistry::start();
    public.publish(PackageFixture::new("ms", "2.1.3"));
    let company = MockRegistry::start();
    company
        .require_token(TOKEN)
        .publish(PackageFixture::new("@acme/ui", "1.0.0").dependency("ms", "^2.0.0"));
    let project = Project::new(&public, json!({ "dependencies": { "@acme/ui": "^1.0.0" } }));
    std::fs::write(
        project.craft_home().join(".npmrc"),
        format!(
            "@acme:registry={}\n//{}/:_authToken={TOKEN}\n",
            company.url(),
            host_of(&company)
        ),
    )
    .expect("write user .npmrc");

    project.craft().arg("install").assert().success();

    let ui = project.require("@acme/ui").expect("@acme/ui loads");
    assert_eq!(ui["dependencies"]["ms"], "2.1.3");
    assert!(company.requested_paths().contains(&"/@acme/ui".to_string()));
    assert!(
        !public
            .requested_paths()
            .iter()
            .any(|path| path.contains("acme"))
    );
    let public_saw_credentials = public.authorizations().iter().any(Option::is_some);
    assert!(!public_saw_credentials);
}

#[test]
fn never_prints_tokens() {
    let registry = MockRegistry::start();
    registry
        .require_token(TOKEN)
        .publish(PackageFixture::new("private-lib", "1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "private-lib": "^1.0.0" } }),
    );
    project.write_file(
        ".npmrc",
        &format!("//{}/:_authToken={TOKEN}\n", host_of(&registry)),
    );

    let output = project
        .craft()
        .args(["--verbose", "install"])
        .output()
        .expect("craft runs");

    assert!(output.status.success());
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!printed.contains(TOKEN));
}
