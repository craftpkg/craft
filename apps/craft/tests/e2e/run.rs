use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;
use testkit::{MockRegistry, PackageFixture};

use crate::common::{Project, print_script};

#[test]
fn runs_a_script_from_package_json() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "greet": "echo hello" } }));

    project
        .craft()
        .args(["run", "greet"])
        .assert()
        .success()
        .stdout(contains("hello"));
}

#[test]
fn passes_arguments_to_the_script() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "greet": "echo hello" } }));

    project
        .craft()
        .args(["run", "greet", "world"])
        .assert()
        .success()
        .stdout(contains("hello world"));
}

#[test]
fn runs_start_and_test_shortcuts() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "start": "echo starting", "test": "echo testing" } }),
    );

    project
        .craft()
        .arg("start")
        .assert()
        .success()
        .stdout(contains("starting"));
    project
        .craft()
        .arg("test")
        .assert()
        .success()
        .stdout(contains("testing"));
}

#[test]
fn runs_an_unknown_command_as_a_script() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "lint": "echo linting" } }));

    project
        .craft()
        .arg("lint")
        .assert()
        .success()
        .stdout(contains("linting"));
}

#[test]
fn fails_when_the_script_fails() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "fail": "exit 3" } }));

    project.craft().args(["run", "fail"]).assert().failure();
}

#[test]
fn exits_with_the_script_exit_code() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "fail": "exit 3" } }));

    project
        .craft()
        .args(["run", "fail"])
        .assert()
        .code(3)
        .stderr("error: fail exited with code 3\n");
}

#[test]
fn passes_arguments_only_to_the_main_script() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": {
            "pretest": "node print.js pre",
            "test": "node print.js test",
            "posttest": "node print.js post",
        } }),
    )
    .with_print_script();

    project
        .craft()
        .args(["run", "test", "--watch"])
        .assert()
        .success()
        .stdout("pre\ntest --watch\npost\n");
}

#[test]
fn stops_at_a_failing_pre_script() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "prebuild": "exit 4", "build": "echo build" } }),
    );

    project
        .craft()
        .args(["run", "build"])
        .assert()
        .code(4)
        .stdout("");
}

#[test]
fn reports_a_missing_script_with_the_available_ones() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": { "test": "echo test", "build": "echo build" } }),
    );

    project
        .craft()
        .args(["run", "deploy"])
        .assert()
        .failure()
        .stderr(contains(
            "script \"deploy\" not found; available scripts: build, test",
        ));
}

#[test]
fn sets_npm_environment_variables() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({
            "name": "app",
            "version": "1.2.3",
            "scripts": { "env": "node -e \"const e = process.env; console.log(e.npm_lifecycle_event, e.npm_package_name, e.npm_package_version, e.npm_config_user_agent)\"" },
        }),
    );

    project
        .craft()
        .args(["run", "env"])
        .assert()
        .success()
        .stdout(contains("env app 1.2.3 craft/"));
}

#[test]
fn runs_dependency_bins_directly() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(&registry, json!({ "dependencies": { "cowsay": "^1.0.0" } }));
    project.craft().arg("install").assert().success();
    std::fs::remove_file(project.path().join("package.json")).expect("remove package.json");

    project
        .craft()
        .args(["cowsay", "a; echo INJECTED"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"))
        .stdout(contains("INJECTED\n").not());
}

#[test]
fn scripts_can_call_dependency_bins() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(
        &registry,
        json!({ "scripts": { "moo": "cowsay" }, "dependencies": { "cowsay": "^1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["run", "moo"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));
}

#[test]
fn runs_pre_and_post_scripts_in_order() {
    let registry = MockRegistry::start();
    let project = Project::new(
        &registry,
        json!({ "scripts": {
            "prebuild": print_script("pre"),
            "build": print_script("build"),
            "postbuild": print_script("post"),
        } }),
    );

    project
        .craft()
        .args(["run", "build"])
        .assert()
        .success()
        .stdout("pre\nbuild\npost\n");
}

#[test]
fn passes_arguments_verbatim() {
    let registry = MockRegistry::start();
    let project = Project::new(&registry, json!({ "scripts": { "show": "node print.js" } }))
        .with_print_script();

    project
        .craft()
        .args(["run", "show", "a; echo INJECTED"])
        .assert()
        .success()
        .stdout("a; echo INJECTED\n");
}

#[test]
fn passes_shell_metacharacters_through_scripts_and_bins_unchanged() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("argv-json", "1.0.0")
            .file(
                "cli.js",
                "#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2)));\n",
            )
            .manifest_field("bin", json!({ "argv-json": "cli.js" })),
    );
    let project = Project::new(
        &registry,
        json!({
            "dependencies": { "argv-json": "1.0.0" },
            "scripts": {
                "direct": "node -e \"console.log(JSON.stringify(process.argv.slice(1)))\" --",
                "via-bin": "argv-json",
            },
        }),
    );
    project.craft().arg("install").assert().success();
    let args = [
        "a; echo INJECTED",
        "a&b|c",
        "100%",
        "say \"hi\"",
        r"C:\dir\",
        "(x) <y>",
        "^caret!",
        "",
    ];
    let expected = format!("{}\n", json!(args));

    for script in ["direct", "via-bin"] {
        project
            .craft()
            .args(["run", script])
            .args(args)
            .assert()
            .success()
            .stdout(expected.clone());
    }
    project
        .craft()
        .args(["exec", "argv-json"])
        .args(args)
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn exec_runs_dependency_bins_but_not_scripts() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "cowsay": "^1.0.0" }, "scripts": { "build": "echo build" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["exec", "cowsay"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));
    project
        .craft()
        .args(["exec", "build"])
        .assert()
        .failure()
        .stderr(contains("no build in node_modules/.bin"));
}

#[test]
fn dlx_runs_a_package_without_adding_it() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("cowsay", "1.6.0").bin("cowsay", "cli.js"));
    let project = Project::new(&registry, json!({ "name": "app" }));

    project
        .craft()
        .args(["dlx", "cowsay"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));

    assert!(project.package_json().get("dependencies").is_none());
    assert!(!project.path().join("node_modules").exists());

    registry.publish(PackageFixture::new("cowsay", "1.7.0").bin("cowsay", "cli.js"));
    project
        .craft()
        .args(["x", "cowsay"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.7.0"));
    project
        .craft()
        .args(["dlx", "cowsay@1.6.0"])
        .assert()
        .success()
        .stdout(contains("cowsay cowsay@1.6.0"));
}

#[test]
fn dlx_runs_a_named_bin_and_passes_its_exit_code() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("toolbox", "1.0.0")
            .bin("tool-a", "a.js")
            .file("b.js", "#!/usr/bin/env node\nprocess.exit(3);\n")
            .manifest_field("bin", json!({ "tool-a": "a.js", "tool-b": "b.js" })),
    );
    let project = Project::new(&registry, json!({}));

    project
        .craft()
        .args(["dlx", "-p", "toolbox", "tool-a"])
        .assert()
        .success()
        .stdout(contains("tool-a toolbox@1.0.0"));
    project
        .craft()
        .args(["dlx", "-p", "toolbox", "tool-b"])
        .assert()
        .code(3);
    project
        .craft()
        .args(["dlx", "toolbox"])
        .assert()
        .failure()
        .stderr(contains(
            "toolbox has no bin named toolbox; it provides: tool-a, tool-b",
        ));
}

#[test]
fn exec_passes_arguments_and_the_exit_code() {
    let registry = MockRegistry::start();
    registry.publish(
        PackageFixture::new("argv-tool", "1.0.0")
            .file(
                "cli.js",
                "#!/usr/bin/env node\nconst args = process.argv.slice(2);\nconsole.log(args.join(' '));\nprocess.exit(Number(process.env.EXIT_CODE || 0));\n",
            )
            .manifest_field("bin", json!({ "argv-tool": "cli.js" })),
    );
    let project = Project::new(
        &registry,
        json!({ "dependencies": { "argv-tool": "1.0.0" } }),
    );
    project.craft().arg("install").assert().success();

    project
        .craft()
        .args(["exec", "argv-tool", "--flag", "two words", "-x"])
        .assert()
        .success()
        .stdout("--flag two words -x\n");

    let mut command = assert_cmd::Command::from_std(project.craft_process());
    command
        .args(["exec", "argv-tool"])
        .env("EXIT_CODE", "7")
        .assert()
        .code(7)
        .stderr(contains("error: argv-tool exited with code 7"));
}

#[test]
fn dlx_runs_scoped_packages_and_reuses_pinned_installs_offline() {
    let registry = MockRegistry::start();
    registry.publish(PackageFixture::new("@acme/tool", "1.0.0").string_bin("cli.js"));
    let project = Project::new(&registry, json!({}));

    for args in [
        vec!["dlx", "@acme/tool@1.0.0"],
        vec!["--offline", "dlx", "@acme/tool@1.0.0"],
        vec!["dlx", "@acme/tool"],
        vec!["--offline", "dlx", "@acme/tool"],
    ] {
        project
            .craft()
            .args(&args)
            .assert()
            .success()
            .stdout("tool @acme/tool@1.0.0\n");
    }
}
