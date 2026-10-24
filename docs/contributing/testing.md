# Testing

```sh
make                 # fmt check, clippy, unit and end-to-end tests
make real-world      # scenarios against registry.npmjs.org and GitHub
docker/windows/test.sh   # the whole suite built for Windows, run under Wine
```

## Unit tests

Each crate has unit tests next to its code. They never touch the network or the real home directory: everything runs in temporary directories with an explicit `Context`.

## End-to-end tests

`apps/craft/tests/e2e/` runs the real `craft` binary against **`MockRegistry`** (`crates/testkit`), an npm-compatible registry on `127.0.0.1` that serves packuments and tarballs built in memory from `PackageFixture`s:

```rust
#[test]
fn installs_dependencies_and_their_subtree() {
    let registry = MockRegistry::start();
    registry
        .publish(PackageFixture::new("ms", "2.1.3"))
        .publish(PackageFixture::new("debug", "4.4.0").dependency("ms", "^2.1.3"));
    let project = Project::new(&registry, json!({ "dependencies": { "debug": "^4.0.0" } }));

    project.craft().arg("install").assert().success();

    assert_eq!(project.required_version("debug"), Ok("4.4.0".to_string()));
}
```

- Each test gets its own project directory and craft home, so tests run in parallel and leave nothing behind.
- Fixtures have real integrity hashes, and their default `index.js` requires their dependencies, so `project.require(…)` under Node proves the tree resolves.
- The mock registry can require tokens, fail requests with chosen statuses and `Retry-After`, send `max-age`, serve over TLS with a self-signed certificate, act as an HTTP proxy, and record request paths, headers and concurrency.
- Scripts in fixtures use `node -e` rather than shell commands, so the same tests run on Windows.

## Real-world tests

`apps/craft/tests/real_world/` installs pinned real packages from the npm registry, JSR and GitHub: an Express server, React server rendering with Redux and React Query, a `craft dlx create-vite` project that builds, TypeScript, native packages, git and tarball dependencies, and more. They are ignored by default; `make real-world` runs them, and CI runs them in the `registry-tests` job.

## Windows

CI runs the full suite on `windows-latest`. Locally, `docker/windows/test.sh` cross-compiles every test executable for `x86_64-pc-windows-gnu` and runs them under Wine with the Windows builds of Node.js and Git:

```sh
docker/windows/test.sh               # everything
docker/windows/test.sh run::         # arguments go to each test executable
```

Wine is not Windows: CI's `windows-latest` job has the final word. On Apple Silicon, Docker's Rosetta emulation can crash Wine processes; switching Docker Desktop to QEMU emulation avoids that.
