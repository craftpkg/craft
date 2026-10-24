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
        .args(["--debug", "install"])
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

fn requests_for(registry: &MockRegistry, path: &str) -> usize {
    registry
        .requested_paths()
        .iter()
        .filter(|requested| requested.as_str() == path)
        .count()
}

#[test]
fn retries_transient_registry_errors() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .fail_next(2, 503, Some(0));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(requests_for(&registry, "/ms"), 3);
    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn gives_up_after_the_configured_retries() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .fail_next(10, 503, Some(0));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(".npmrc", "fetch-retries=1\n");

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains("returned HTTP 503"));

    assert_eq!(requests_for(&registry, "/ms"), 2);
}

#[test]
fn waits_as_long_as_retry_after_asks() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .fail_next(1, 429, Some(1));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    let started = std::time::Instant::now();
    project.craft().arg("install").assert().success();

    assert!(started.elapsed() >= std::time::Duration::from_secs(1));
    assert_eq!(requests_for(&registry, "/ms"), 2);
}

#[test]
fn does_not_retry_missing_packages() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "dependencies": { "ghost": "^1.0.0" } }));

    project.craft().arg("install").assert().failure();

    assert_eq!(requests_for(&registry, "/ghost"), 1);
}

#[test]
fn identifies_itself_to_the_registry() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project.craft().arg("install").assert().success();

    let user_agents = registry.user_agents();
    assert!(!user_agents.is_empty());
    let all_craft = user_agents.iter().all(|agent| {
        agent
            .as_deref()
            .is_some_and(|agent| agent.starts_with("craft/"))
    });
    assert!(all_craft, "{user_agents:?}");
}

#[test]
fn limits_requests_in_flight() {
    let registry = MockRegistry::start();
    let mut dependencies = serde_json::Map::new();
    for index in 0..30 {
        let name = format!("pkg-{index}");
        registry.publish(PackageFixture::new(&name, "1.0.0"));
        dependencies.insert(name, json!("^1.0.0"));
    }
    let project = Project::new(&registry, json!({ "dependencies": dependencies }));
    project.write_file(".npmrc", "network-concurrency=2\n");

    project.craft().arg("install").assert().success();

    let max_in_flight = registry.max_concurrent_requests();
    assert!(max_in_flight <= 4, "{max_in_flight} requests at once");
    assert!(max_in_flight >= 2, "{max_in_flight} requests at once");
}

#[test]
fn retries_an_interrupted_tarball_download() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let package_json = json!({ "dependencies": { "ms": "^2.0.0" } });
    let original = Project::new(&registry, package_json.clone());
    original.craft().arg("install").assert().success();

    let checkout = Project::new(&registry, package_json);
    checkout.write_file("craft.lock", &original.read_file("craft.lock"));
    registry.fail_next(1, 502, Some(0));
    checkout.craft().arg("install").assert().success();

    let tarball = "/ms/-/ms-2.1.3.tgz";
    assert_eq!(requests_for(&registry, tarball), 3);
    assert_eq!(checkout.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn sends_basic_credentials_from_auth_or_username_and_password() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("private-lib", "1.0.0"));
    let user_and_password = "Basic dXNlcjpwYXNz";
    let configs = [
        format!("//{}/:_auth=dXNlcjpwYXNz\n", host_of(&registry)),
        format!(
            "//{0}/:username=user\n//{0}/:_password=cGFzcw==\n",
            host_of(&registry)
        ),
    ];

    for config in configs {
        let project = Project::new(
            &registry,
            json!({ "dependencies": { "private-lib": "^1.0.0" } }),
        );
        project.write_file(".npmrc", &config);
        project.craft().arg("install").assert().success();
    }

    let authorizations = registry.authorizations();
    assert_eq!(authorizations.len(), 4);
    assert!(
        authorizations
            .iter()
            .all(|authorization| authorization.as_deref() == Some(user_and_password))
    );
}

#[test]
fn layers_the_user_npmrc_under_the_project_npmrc() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("private-lib", "1.0.0"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "private-lib": "^1.0.0" } }),
    );
    let token_line = |token: &str| format!("//{}/:_authToken={token}\n", host_of(&registry));
    let sent_token = || {
        registry
            .authorizations()
            .last()
            .cloned()
            .flatten()
            .unwrap_or_default()
    };
    let reinstall = || {
        let _ = std::fs::remove_file(project.path().join("craft.lock"));
        let _ = std::fs::remove_dir_all(project.path().join("node_modules"));
        let _ = std::fs::remove_dir_all(project.craft_home().join("store"));
        let _ = std::fs::remove_dir_all(project.craft_home().join("cache"));
    };

    std::fs::write(project.craft_home().join(".npmrc"), token_line("user"))
        .expect("write the user npmrc");
    project.craft().arg("install").assert().success();
    assert_eq!(sent_token(), "Bearer user");

    let other_config = project.craft_home().join("other.npmrc");
    std::fs::write(&other_config, token_line("userconfig")).expect("write another npmrc");
    reinstall();
    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("NPM_CONFIG_USERCONFIG", &other_config)
        .assert()
        .success();
    assert_eq!(sent_token(), "Bearer userconfig");

    project.write_file(".npmrc", &token_line("project"));
    reinstall();
    project.craft().arg("install").assert().success();
    assert_eq!(sent_token(), "Bearer project");
}

#[test]
fn reads_settings_from_npm_config_variables() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    registry.fail_next(1, 503, None);

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("npm_config_fetch_retries", "0")
        .assert()
        .failure()
        .stderr(contains("503"));
}

#[test]
fn fetches_through_the_configured_proxy() {
    let proxy = MockRegistry::start();
    proxy.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&proxy, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(".npmrc", &format!("proxy={}\n", proxy.url()));

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .arg("install")
        .env("CRAFT_REGISTRY", "http://registry.example.invalid")
        .assert()
        .success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
    assert!(
        proxy
            .proxied_hosts()
            .contains(&"registry.example.invalid".to_string())
    );
}

#[test]
fn bypasses_the_proxy_for_noproxy_hosts() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(
        ".npmrc",
        "proxy=http://127.0.0.1:9\nnoproxy=127.0.0.1\nfetch-retries=0\n",
    );

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
    assert!(registry.proxied_hosts().is_empty());
}

#[test]
fn rejects_a_registry_certificate_it_does_not_trust() {
    let registry = MockRegistry::start_tls();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));

    project
        .craft()
        .arg("install")
        .assert()
        .failure()
        .stderr(contains(format!(
            "could not verify the certificate of {} (invalid peer certificate: UnknownIssuer)",
            host_of(&registry).trim_start_matches("https://")
        )))
        .stderr(contains(
            "point cafile in .npmrc at your registry's CA certificate",
        ));
    assert_eq!(registry.requested_paths().len(), 0);
}

#[test]
fn trusts_a_registry_certificate_from_cafile() {
    let registry = MockRegistry::start_tls();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(
        "ca.pem",
        registry
            .certificate_pem()
            .expect("a TLS registry has a certificate"),
    );
    let cafile = project.path().join("ca.pem");
    project.write_file(".npmrc", &format!("cafile={}\n", cafile.display()));

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}

#[test]
fn skips_certificate_checks_without_strict_ssl() {
    let registry = MockRegistry::start_tls();
    registry.publish(PackageFixture::new("ms", "2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "ms": "^2.0.0" } }));
    project.write_file(".npmrc", "strict-ssl=false\n");

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("ms"), Ok("2.1.3".to_string()));
}
