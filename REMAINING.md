# Craft — Remaining Work

Status of the Rust package manager as of **2026-10-26**, and the ordered list of work needed to finalize it.
Work through tasks top to bottom: each phase unblocks the next. Tick a task's box when it is merged.

**Known-bug tests:** every open bug that has a scenario is an `#[ignore = "fixed by <task>: …"]` end-to-end test in
`apps/craft/tests/e2e/`. `cargo test -p craft --test e2e -- --ignored` lists what's still broken; when a task
lands, delete the `#[ignore]` on its tests.

**Real-world tests:** `make real-world` runs `apps/craft/tests/real_world/` (ignored by default, and in CI's
`registry-tests` job) against registry.npmjs.org, JSR and GitHub with pinned real packages: an Express server, React SSR
with Redux and React Query sharing one React, a `craft dlx create-vite` project that installs and builds, TypeScript and
Prettier through bins and scripts, esbuild / SWC / Lightning CSS platform binaries, a trusted `bcrypt` build, foreign
platform installs, git / tarball / JSR / `file:` / `link:` sources, aliases, outdated / update / list / why / remove,
offline and frozen installs, and dlx. Projects share one craft home per run, so the store is exercised concurrently.

Legend: **P0** broken installs / crashes · **P1** wrong behaviour · **P2** performance, DX, hygiene ·
✅ reproduced against the real binary · 📖 found by code reading

---

## Current state

Works today:

- `craft install` / `craft add` / `craft remove` / `craft run` / `craft cache clean --force`
- Parallel resolution + download from `registry.npmjs.org`, global tarball cache in `~/.craft/packages`
- Happy-path projects install and run (`express`, `lodash`, `openai`, `axios`, `typescript`, `eslint`)
- `craft <bin>` dispatches to `node_modules/.bin` (`craft tsc --version` works)
- `cargo build` and `cargo test` green (62 tests)

Broken or missing, in short: non-semver specs panic, peer deps and aliases don't link, `@types/*` links dangle,
the global cache is mutated per project (races), no lockfile, no integrity checks, no optional/platform deps,
no lifecycle scripts, `craft run` doesn't put `.bin` on `PATH`.

---

## Progress tracker

| ID | Task | Pri | Status |
|---|---|---|---|
| **Phase 0 — Foundations** | | | |
| 0.1 | Explicit `Context` (cwd + `CRAFT_HOME`) instead of process-global env/cwd | P2 | [x] |
| 0.2 | Test isolation: no real installs or env mutation in unit tests | P2 | [x] |
| 0.3 | Mock registry + end-to-end test harness | P2 | [x] |
| 0.4 | Lint/CI hygiene (`clippy --all-targets`, fmt, workspace lints, dead code) | P2 | [x] |
| **Phase 1 — Correctness fixes (current layout)** | | | |
| 1.1 | Dependency spec classification — no more panics | P0 | [x] |
| 1.2 | `InstallPipe` returns a resolution graph, not a flat list | P0 | [x] |
| 1.3 | npm alias linking (root + transitive) | P0 | [x] |
| 1.4 | Tarball root-directory stripping | P0 | [x] |
| 1.5 | HTTP status checks + friendly registry errors | P0 | [x] |
| 1.6 | Atomic download and extraction | P0 | [x] |
| 1.7 | Integrity verification (`dist.integrity` / `shasum`) | P0 | [x] |
| 1.8 | Symlink-safe filesystem helpers (dangling links) | P0 | [x] |
| 1.9 | `craft add` fixes | P1 | [x] |
| 1.10 | Preserve `package.json` formatting and key order | P1 | [x] |
| 1.11 | `craft remove` fixes | P1 | [x] |
| 1.12 | `craft run` fixes (PATH, exit code, quoting, pre/post, env) | P1 | [x] |
| 1.13 | Git / tarball-URL / `file:` / `link:` dependencies | P1 | [x] |
| 1.14 | Bin linking edge cases | P1 | [x] |
| **Phase 2 — Store & layout redesign** | | | |
| 2.1 | Global content-addressed store (read-only) | P0 | [x] |
| 2.2 | Per-project virtual store `node_modules/.craft` | P0 | [x] |
| 2.3 | Peer dependency resolution | P0 | [x] |
| 2.4 | Bins inside the virtual store | P1 | [x] |
| 2.5 | Prune extraneous packages | P1 | [x] |
| **Phase 3 — Lockfile** | | | |
| 3.1 | Lockfile format (text, deterministic) | P0 | [x] |
| 3.2 | Write lockfile after install/add/remove | P0 | [x] |
| 3.3 | Install from lockfile (skip resolution) | P0 | [x] |
| 3.4 | `--frozen-lockfile` / `craft ci` | P1 | [x] |
| **Phase 4 — Platform packages & lifecycle scripts** | | | |
| 4.1 | `optionalDependencies` | P0 | [x] |
| 4.2 | `os` / `cpu` / `libc` filtering | P0 | [x] |
| 4.3 | Lifecycle scripts with an allowlist | P0 | [x] |
| **Phase 5 — Network & performance** | | | |
| 5.1 | Abbreviated packuments | P2 | [-] deferred |
| 5.2 | Memoize packument fetches per package name | P2 | [x] |
| 5.3 | Global concurrency limits | P2 | [x] |
| 5.4 | Timeouts, retries, User-Agent | P2 | [x] |
| 5.5 | Streaming downloads (hash while writing) | P2 | [x] |
| 5.6 | Metadata disk cache (ETag / 304) | P2 | [x] |
| 5.7 | Registry configuration (`.npmrc`, scopes, auth) | P1 | [x] |
| 5.8 | Clone whole store entries into `node_modules` | P2 | [x] |
| 5.9 | Lean packuments: abbreviated, parsed lazily, not all kept (supersedes 5.1) | P2 | [ ] |
| 5.10 | Fetch tarballs off the resolution path | P2 | [ ] |
| 5.11 | Install straight from the lockfile's graph | P2 | [ ] |
| 5.12 | Extract while downloading | P2 | [ ] |
| 5.13 | One network client, fewer manifest reads, no graph clones | P2 | [ ] |
| 5.14 | Benchmark: isolate pnpm's store, add a stale-cache scenario | P2 | [x] |
| 5.15 | Skip freeing memory at exit | P2 | [x] |
| 5.16 | Link while importing, prune only what changed | P2 | [x] |
| 5.17 | Trust fresh-enough metadata (minimum release age) | P2 | [ ] |
| **Phase 6 — CLI & UX** | | | |
| 6.1 | Progress output and install summary | P2 | [x] |
| 6.2 | Error reporting (no panics, contextual messages) | P2 | [x] |
| 6.3 | Missing commands and flags | P2 | [x] |
| 6.4 | Windows support (shims, junctions) | P2 | [x] |
| 6.5 | Release & docs accuracy | P2 | [x] |
| **Phase 7 — Later** | | | |
| 7.x | Workspaces, `package-lock.json` import, overrides, `dlx`, global installs | — | [ ] |

---

## Phase 0 — Foundations

Do these first: every later fix needs a fast, offline, isolated way to prove it works.

### 0.1 Explicit `Context` instead of process-global state · P2 📖 — ✔ done

> Done: `contract::Context { cwd, craft_home, verbose }` built in `main` via `Context::from_env`; `Actor::with(context, payload)`; `InstallPipe`/`LinkerPipe`/`Resolver` take the context or cache dir; `CRAFT_HOME` override; missing home is an error, not a panic; `set_verbose`/`CRAFT_VERBOSE` and `RemoveActorPayload::working_dir` removed.

**Problem**
- `contract::get_package_cache_dir()` reads `HOME` and `expect`s it (`crates/contract/src/constants.rs:11`) — panics without `HOME`.
- Every actor/pipe calls `std::env::current_dir()` itself, so tests must `set_current_dir` / `set_var("HOME")`, which is process-global and races between parallel tests (`linker_pipe.rs:335-338`, `install_actor.rs:48`).
- `CraftManager::set_verbose` sets `CRAFT_VERBOSE` via `unsafe set_var`, but nothing ever reads it.

**Fix**
- Add a `Context { cwd: PathBuf, home: PathBuf /* ~/.craft */, verbose: bool }` in `contract`, built once in `main`.
- `home` resolves from `CRAFT_HOME`, then `dirs::home_dir()/.craft`; return an error, not a panic.
- Pass `&Context` (or `Arc<Context>`) into actors and pipes; remove all `current_dir()` / `get_package_cache_dir()` calls below `main`.
- Delete `set_verbose` / `CRAFT_VERBOSE` or read it in `debug::init`.

**Done when**
- `grep -rn "current_dir()\|set_var\|get_package_cache_dir" crates/` only hits `main` and `Context::from_env`.
- Tests construct a `Context` pointing at a `tempdir` — no global mutation.

### 0.2 Test isolation · P2 ✅ — ✔ done

> Done: manager tests rewritten as offline dispatch tests (start/run scripts, install without package.json, remove, cache clean) in a tempdir `Context`; 6 live-registry resolver tests are `#[ignore]`d and run in a separate `registry-tests` CI job via `--ignored`. Verified: `cargo test` passes with all HTTP(S) routed to a dead proxy, and leaves `git status` and `~/.craft` untouched. The ignored tests get offline equivalents once the mock registry (0.3) exists.

**Problem**
- `crates/manager/src/manager.rs:99-115` runs a real `craft add react` against the crate dir — every `cargo test` creates `crates/manager/node_modules` and writes to the real `~/.craft`.
- `test_handle_start_command` tries to run a real `start` script.
- Resolver tests (`npm.rs`, `resolver.rs`) require network access and the live registry state.

**Fix**
- Replace manager tests with dispatch tests that don't execute actors (or run them against a `Context` in a tempdir + mock registry from 0.3).
- Mark live-registry tests `#[ignore]` (run in a nightly CI job with `--ignored`), and cover the same logic via the mock registry.

**Done when** `cargo test` works offline and leaves `git status` and `~/.craft` untouched.

### 0.3 Mock registry + end-to-end harness · P2 — ✔ done

> Done: `CRAFT_REGISTRY` / `Context::with_registry` make the registry configurable. `crates/testkit` provides `MockRegistry` (a `tiny_http` server on `127.0.0.1` serving packuments, tarballs and npm-style 404s, recording request paths) and `PackageFixture` (builds tarballs in memory with real `sha512` integrity, configurable root dir, bins, peer/optional deps; its default `index.js` requires its dependencies so `require()` proves the subtree resolves). `apps/craft/tests/e2e/` runs the real binary per test in a tempdir project + craft home: 20 passing scenarios and 23 `#[ignore]`d known-bug scenarios, each confirmed to fail today for the documented reason. CI installs Node 22 for these tests.

**Fix**
- Make the registry base URL configurable (`Context.registry`, default `https://registry.npmjs.org`) — also needed by 5.7.
- Add a test-support crate (or `tests/common`) that:
  - serves packuments + tarballs from fixtures with `httpmock`/`wiremock` (or a tiny `axum` server);
  - builds fixture tarballs on the fly (`tar` + `flate2`, like `crates/tarball/src/gzip.rs` tests do), with configurable root dir name (`package/`, `node/`, …).
- E2E tests run the `craft` binary via `assert_cmd` in a tempdir with `CRAFT_HOME` and the mock registry URL, then assert with `node -e "require(...)"` where Node is available.

**Fixture scenarios to add now** (each later task turns its scenario green):
peer deps, root alias, transitive alias, `@types`-style tarball root, dist-tag spec, `file:` spec,
dangling symlink in `node_modules`, failing script exit code, scoped package with string `bin`,
404 package, corrupt tarball / wrong integrity, optional platform dep.

### 0.4 Lint / CI hygiene · P2 ✅ — ✔ done

> Done: `make lint` runs `cargo fmt --check` + `clippy --workspace --all-targets -D warnings`; `clippy.toml` sets `allow-unwrap-in-tests`; `process`/`tarball` moved to edition 2024 with workspace lints; template `add()`, `InstallPackage::satisfies` and the `semver` dependency removed; the verbose log filter covers every workspace crate (actor logs were previously dropped); `Cli::normalize` returns a new `cli::Action` enum so the manager only sees `Add`/`Remove`/`Run`/`Cache`/`Install`; CI runs `make` on Ubuntu and macOS.
>
> Deferred: denying `clippy::expect_used` outside tests waits for 1.1 (most remaining `expect`s are the panics 1.1 removes); `windows-latest` joins the CI matrix in 6.4, since bins and directory links don't work on Windows yet.

**Problem**
- `cargo clippy --all-targets` fails: 14 `unwrap_used` errors in `crates/actor/src/{install_actor,remove_package_actor}.rs` tests. CI passes only because the Makefile lints without `--all-targets`.
- `crates/process` and `crates/tarball` use edition 2021 and don't opt into `[lints] workspace = true`.
- Dead code: `crates/tarball/src/lib.rs` template `add()` fn; `InstallPackage::satisfies` (`install_package.rs:58`) is unused and uses Cargo semver, not npm semver; `crates/lockfile` and `crates/progress` are not depended on by anything (keep — used in Phase 3 / 6.1).
- `debug::init` filter (`crates/debug/src/debug.rs:11`) omits `actor`, `network`, `tarball`, `process`, `lockfile`.
- `manager.rs` handles `Start`, `Test`, `External` again after `Cli::normalize` already rewrote them — unreachable branches.

**Fix**
- Makefile `lint`: `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --check`.
- Allow `unwrap`/`expect` in tests via `#![cfg_attr(test, allow(clippy::unwrap_used))]` or switch tests to `-> Result<()>` with `?`.
- Edition 2024 + workspace lints everywhere; consider also denying `clippy::expect_used` outside tests (would have caught 1.1).
- Remove the dead code listed above; make `normalize()` return a smaller enum so the manager can't see un-normalized variants.
- CI matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`.

---

## Phase 1 — Correctness fixes (current layout)

Targeted fixes that don't need the store redesign. Some code (linker) will be rewritten in Phase 2;
keep fixes there minimal and put the logic in reusable helpers so it carries over.

### 1.1 Dependency spec classification — no more panics · P0 ✅ — ✔ done

> Done: `package::DependencySpec` (`Registry`/`Alias`/`Git`/`Tarball`/`File`/`Link`/`Workspace`) with `VersionRequirement` (`Exact`/`Range`/`Tag`) parses every `package.json` spec and `craft add` literal; `""`/`*` mean any version, invalid specs and unsupported kinds return `SpecError` instead of panicking. `select_version` follows npm: a tag's version, else `latest` when it satisfies the range, else the highest satisfying stable version. `from_literal` splits at the first `@` after a scope (aliases like `sw@npm:string-width@^4` now parse) and keeps git URLs/paths whole. The broken `GitResolver` and `InstallPackage::is_git` were removed. Root packages are now linked to the exact artifact they resolved to (see 1.2). `clippy::expect_used` is denied outside tests; `testkit` and the e2e crate allow it. 4 known-bug e2e tests now pass.

**Problem** — `crates/resolver/src/npm.rs:38` does `Range::parse(req_version).expect(..)`. Any spec that isn't a semver range panics (exit 101):

```
$ craft add ms@latest
thread 'main' panicked at crates/resolver/src/npm.rs:38:57:
should not panic: SemverError { input: "latest", ... NoValidRanges }
```

Reproduced for `latest`, `file:../a`, `github:lodash/lodash`, `https://…/ms-2.1.3.tgz`, `workspace:*`.
`linker_pipe.rs:61,137,144` also `expect` on `Version::parse` (panics for git artifacts with version `"git"`).
`is_git()` checks the package **name** (`install_package.rs:79`), but in `package.json` git URLs live in the **version** field.

**Fix** — replace `DependencySpec { package_name, version }` with an enum parsed once from `(name, spec)`:

```rust
pub enum DependencySpec {
    Range   { name: String, range: Range },        // "^1.2.3", "1.x || >=3", "", "*"
    Tag     { name: String, tag: String },         // "latest", "next", "beta"
    Version { name: String, version: Version },    // "1.2.3" (fast path)
    Alias   { alias: String, target: Box<DependencySpec> }, // "npm:pkg@^1"
    Git     { name: String, url: String, committish: Option<String> }, // git+https, git+ssh, github:, gitlab:, bitbucket:, "user/repo"
    Tarball { name: String, url: String },         // http(s)://…/*.tgz
    File    { name: String, path: PathBuf },       // file:
    Link    { name: String, path: PathBuf },       // link:
    Workspace { name: String, range: String },     // workspace:
}
```

- Empty string and `*` → any range. Invalid input → `Err` with the package name and spec in the message.
- `InstallPackage::from_literal` (`craft add pkg@spec`) parses through the same function.
- Dist-tag resolution: look up `dist-tags[tag]`.
- Range resolution should match npm: prefer `dist-tags.latest` when it satisfies the range, else highest satisfying non-prerelease.
- Until 1.13 / Phase 7, `Git`/`Tarball`/`File`/`Link`/`Workspace` return a clear `unsupported dependency type` error — never a panic.
- Replace every `expect("should not panic")` in `npm.rs` and `linker_pipe.rs` with fallible code (skip unparsable registry versions with a `debug::warning!`).
- Then deny `clippy::expect_used` outside tests in the workspace lints (deferred from 0.4).

**Done when** the spec table above is unit-tested; the E2E dist-tag fixture installs; no `expect` remains in `resolver`/`pipeline` non-test code.

### 1.2 `InstallPipe` returns a resolution graph · P0 📖 — ✔ done

**Problem** — `InstallPipe::run` returns a deduplicated `Vec<ResolvedArtifact>` (`install_pipe.rs:149-165`). The linker then **re-guesses** which artifact each dependency edge meant by name + "highest satisfying version" (`linker_pipe.rs:46-64`, `125-148`). Every alias, multi-version and "which version did the root resolve to" bug comes from this lost information.

> Done: `resolver::graph` defines `ResolutionGraph { roots, nodes }`, `ResolvedNode { artifact, dependencies }` and `Edge { name, spec, target, kind }` keyed by `NodeId` (`name@version`, `BTreeMap` for deterministic order). `InstallPipe::resolve_package` returns the `NodeId` each request resolved to and records edges as it recurses; each version is downloaded and walked once (claimed-node set), and cycles resolve without waiting on themselves. `LinkerPipe` links exactly the graph's edges and no longer depends on `node-semver`. `craft add` reads root edges from the graph.

**Fix**
- Return a graph:

```rust
pub struct ResolutionGraph {
    pub nodes: HashMap<NodeId, ResolvedNode>,         // NodeId = "name@version"
    pub roots: Vec<Edge>,                             // from package.json
}
pub struct ResolvedNode { artifact: ResolvedArtifact, deps: Vec<Edge> }
pub struct Edge { link_name: String /* alias-aware */, spec: String, target: NodeId, kind: DepKind /* Prod|Dev|Optional|Peer */ }
```

- Each `resolve_package` call returns its `NodeId`; record the edge from the parent at that point.
- Linker consumes edges directly: no `Range::parse` in the linker at all.
- This graph is what the lockfile (Phase 3) serializes.

**Done when** `LinkerPipe` has no version matching logic and links exactly the edges it's given.

### 1.3 npm alias linking · P0 ✅ — ✔ done

> Done with 1.2: every link is created at `node_modules/<edge.name>`, so root and transitive aliases resolve under Node, and `craft add sw@npm:string-width@^4.2.0` saves `"sw": "npm:string-width@^4.2.3"`. Verified against the real registry with `@isaacs/cliui` (`string-width-cjs`).

**Problem** — aliases resolve but never link. `artifact_map` is keyed by real name, lookups use the alias name (`linker_pipe.rs:46`, `117`), and the destination uses `artifact.name` (`linker_pipe.rs:168`).

```
"sw": "npm:string-width@^4.2.0"   → node_modules/ is empty, exit 0
"@isaacs/cliui": "8.0.2"          → Error: Cannot find module 'string-width-cjs'
```

**Fix** — with 1.2 in place, link at `node_modules/<edge.link_name>` → target node. Same for transitive edges.
`craft add sw@npm:string-width@^4` must save `"sw": "npm:string-width@^4.x.y"`.

**Done when** both fixtures above `require()` successfully.

### 1.4 Tarball root-directory stripping · P0 ✅ — ✔ done

> Done: `tarball::gzip::unzip` strips whatever root folder entries sit under and extracts into `Context::package_dir` (`<cache>/<name>-<version>/package`), the one path the install and linker pipes share. Entries with `..`, absolute paths or a non-folder root are rejected (`TarballError::UnsafePath`); symlinks and hardlinks are skipped, as npm does. Files and dirs get default permissions, with the execute bit kept for files that had one. The `<name>-<version>/package` layout stays until the store redesign (2.1). Caches extracted with the old code under a non-`package/` root are re-extracted, since their `package/` dir is missing.

**Problem** — the linker assumes the archive root is `package/` (`linker_pipe.rs:33`, `163`). `@types/*` tarballs use `node/`, some older packages use other names:

```
node_modules/@types/node -> ~/.craft/packages/@types/node-22.20.5/package   (does not exist)
```

**Fix** — in `tarball::gzip::unzip`, strip the first path component of every entry (iterate `archive.entries()`, rewrite path, `entry.unpack_in`-style safety checks for `..` and absolute paths). Extract to `<store>/<name>@<version>/` directly so the linker never guesses a subdirectory.
Also normalize permissions (some tarballs have `0o000` dirs) — files `0o644`, dirs and bins `0o755`.

**Done when** the `@types`-style fixture links to a dir containing `index.d.ts`.

### 1.5 HTTP status checks + friendly registry errors · P0 ✅ — ✔ done

> Done: `Network::fetch` / `download` turn non-2xx responses into `NetworkError::Status { url, status }` before reading the body, so error pages are never parsed or written to the cache. `NpmResolver` maps 404 → `RegistryError::PackageNotFound`, 401/403 → `RegistryError::AccessDenied` (credentials hint); anything else keeps its cause under "failed to fetch {name} from {registry}". `InstallPipe` threads the resolved ancestry through recursion and wraps resolve/fetch failures once, at the failing request: `failed to install ghost@^1.0.0 (required by express@4.21.2 > body-parser@1.20.3)`. Retrying 429/5xx is left to 5.4.

**Problem**
- `Network::fetch` and `Network::download` (`crates/network/src/network.rs:21-37`) never check `response.status()`. A 404/500 body is saved as a `.tgz`, or parsed as a packument.
- Unknown package error today:

```
Caused by: missing field `name` at line 1 column 21
```

**Fix**
- `response.error_for_status()`; map 404 → `PipelineError::PackageNotFound { name }`, 401/403 → auth error hint, 429/5xx → retryable (5.4).
- Error messages name the package and the spec that requested it, plus the parent chain if transitive (`express > body-parser > foo`).

### 1.6 Atomic download and extraction · P0 📖 — ✔ done

> Done: `Network::download` writes to a `NamedTempFile` beside the target and renames it into place; `tarball::gzip::unzip` extracts into a `.extract-*` staging dir beside `package/` and renames it, keeping the other copy if another process got there first. A failed extraction removes its staging dir. `InstallPipe::fetch` checks for the extracted package before downloading, so a missing `.tgz` no longer triggers a re-download. Verified by `kill -9` at several points of a 71-package real-registry install (2–69 packages extracted): every re-run succeeded. Not done: a completion marker — dirs half-extracted by craft builds older than this are still trusted (`craft cache clean` clears them). Concurrent installs still race while linking into the shared cache (1.8 / 2.2), integrity before rename is 1.7.

**Problem**
- `Resolver::download` treats `file_path.exists()` as "cached" (`crates/resolver/src/resolver.rs:67`). `Network::download` creates the final file then writes into it — a killed process leaves a truncated `.tgz` that is reused forever.
- Same for extraction: `if !unzip_dir.exists() { unzip(..) }` (`install_pipe.rs:88`) — half-extracted dirs are reused forever.
- Locks are in-process only (`download_locks`, `unzip_locks`), so two `craft` processes race.

**Fix**
- Download to `<path>.tmp-<pid>-<rand>`, verify integrity (1.7), then `rename` into place (atomic on the same filesystem).
- Extract into `<dir>.tmp-<rand>`, then `rename`; if the rename fails because the target now exists, another process won — delete the temp dir and continue.
- Optional: a marker file (`.craft-complete`) written last, and treat dirs without it as absent.

**Done when** killing `craft` mid-install and re-running produces a working install; two concurrent installs never error.

### 1.7 Integrity verification · P0 📖 — ✔ done

> Done: `PackageDist` reads `integrity` and `shasum`; `network::Integrity` parses SRI (`sha512`/`sha384`/`sha256`/`sha1`, strongest listed algorithm wins, `?options` and unknown algorithms ignored) or falls back to the sha1 `shasum`, and is stored on `ResolvedArtifact` for the lockfile. `Network::download` verifies before the atomic rename, so a mismatch (`IntegrityError::Mismatch`, both hashes in the message) leaves nothing in the cache. A cached `.tgz` is re-verified before reuse and downloaded again if it no longer matches. Hashing still buffers the whole body; streaming is 5.5. Also fixed: the e2e suite stalled for 90s in about half the runs, because tiny_http's thread pool loses wakeups while threads sit on keep-alive connections; `MockRegistry` now uses a std `TcpListener` that answers one request per connection.

**Problem** — `PackageDist` only has `tarball` (`crates/package/src/npm.rs:58-60`); `dist.integrity` / `dist.shasum` are ignored. Nothing stops a corrupted or tampered tarball from being installed.

**Fix**
- Add `integrity: Option<String>` and `shasum: Option<String>` to `PackageDist`.
- Verify SRI (`sha512-<base64>`, also accept `sha256`/`sha1`) using `ssri` or `sha2` + `base64`; fall back to `shasum` (sha1 hex). Hash while streaming (5.5).
- Mismatch → delete the temp file, error `IntegrityMismatch { name, version, expected, actual }`.
- Store integrity on `ResolvedArtifact` (needed by the lockfile).

**Done when** the corrupt-tarball fixture fails with an integrity error and leaves nothing in the cache.

### 1.8 Symlink-safe filesystem helpers · P0 ✅ — ✔ done

> Done: new `fsutil` crate. `remove_entry` removes a file, tree or symlink (dangling included) without following it. `force_symlink_dir` / `force_symlink_file` create the link under a unique staging name and rename it over the destination, so stale entries, dangling links and concurrent writers never surface as `EEXIST`. `ensure_dir` replaces anything in the way of a directory (a dangling `.bin` or `@scope` link, a stray file). `relative_target` gives `.bin` links npm-style targets (`../typescript/bin/tsc`); package links stay absolute because they point into `~/.craft`. The linker and `craft remove` use them everywhere. The concurrent-install e2e scenario now passes (25/25 runs), so it is no longer ignored; the cross-project interference it stood for remains a 2.2 concern.

**Problem** — `Path::exists()` follows symlinks, so a dangling link looks absent; the next `symlink()` fails with `EEXIST`, and `remove` skips it.

```
$ ln -s /nonexistent node_modules/express && craft install
Error: File exists (os error 17)
$ craft remove lodash      # lodash is a dangling link → left in place, exit 0
```

Occurrences: `linker_pipe.rs:79`, `176`, `236`; `remove_package_actor.rs:32`.

**Fix** — one helper module (e.g. `crates/fsutil` or inside `pipeline`):

```rust
async fn remove_entry(path: &Path) -> io::Result<()>   // symlink_metadata: file/symlink → remove_file, dir → remove_dir_all, NotFound → Ok
async fn force_symlink_dir(target: &Path, link: &Path) -> io::Result<()>  // remove_entry + symlink, retry once on EEXIST
```

Use relative symlink targets where possible (relocatable `node_modules`). Use it everywhere links are created or removed.

### 1.9 `craft add` fixes · P1 ✅ — ✔ done

> Done: `craft add` reads `package.json` before resolving anything, so a missing one fails with no registry traffic. Saved specs follow npm: `^<resolved>` for tags, ranges and exact versions, the exact version with the new `-E/--exact`, and `npm:<target>@…` for aliases (from the graph's root edges since 1.2). Adding with or without `-D` moves an existing entry out of the other section. Left for later: suggesting `craft init` (6.3), saving git/file specs verbatim (1.13), and installing all dependencies so the lockfile stays consistent (Phase 3).

**Problems** (`crates/actor/src/add_package_actor.rs`)
- ~~`craft add react@18` installs but does not update `package.json`~~ (fixed in 1.1).
- ~~When several versions of a name are resolved, `find` picks an arbitrary one~~ (fixed in 1.1).
- ~~`package.json` is read **after** installing (line 37); with no `package.json`, all work is done then it fails.~~
- Only the added packages are installed/linked — existing deps aren't verified, and once the lockfile exists it must be updated too.

**Fix**
- Read `package.json` first; error early if missing (suggest `craft init`, 6.3).
- Save rules (match npm): tag or range spec → `^<resolved>`; exact version spec → `^<resolved>` unless `-E/--exact`; alias → `npm:<target>@^<resolved>`; git/file → the original spec.
- If the package already exists in the other section (`dependencies` vs `devDependencies`), move it.
- After Phase 3: run a full install (all deps) so the lockfile stays consistent.

### 1.10 Preserve `package.json` formatting · P1 ✅ — ✔ done

> Done: `package::ProjectManifest` edits `package.json` as an order-preserving `serde_json` map (`preserve_order` enabled) and writes back the detected indent (spaces or tabs, 2 by default for minified files), CRLF line endings and trailing newline; a BOM is dropped. New entries go in at their alphabetical position, updated ones keep theirs, and a section emptied by removal is deleted. `craft add` and `craft remove` both use it, and `remove` only writes when something changed. Verified byte-identical add → remove round trips on this repo's `package.json` and a tab/CRLF file against the real registry. The typed `PackageJson` remains for reading.

**Problem** — writes go through the typed `PackageJson` struct with a `HashMap` `other` field (`crates/package/src/npm.rs:28-29`) and `serde_json` without `preserve_order`. Result: top-level keys reorder between runs (`private`/`type` swapped in testing), indentation is forced to 2 spaces, trailing newline is dropped → noisy git diffs.

**Fix**
- Enable `serde_json`'s `preserve_order` feature.
- Edit `package.json` as a `serde_json::Value` (only touching `dependencies`/`devDependencies`/…), not by re-serializing the typed struct.
- Detect indentation (tab / N spaces) and the trailing newline from the original file and reproduce them.
- New dependency maps are sorted alphabetically (npm behaviour); existing ones keep order with the new key inserted sorted.
- Keep the typed `PackageJson` for reading registry manifests only.

**Done when** `add` then `remove` of the same package produces a byte-identical `package.json`.

### 1.11 `craft remove` fixes · P1 ✅ — ✔ done

> Done: `craft remove` deletes every `.bin` link whose target (resolved lexically, relative or absolute) lies inside the removed package, so other packages' bins stay. A package listed in neither dependency section prints `warning: <pkg> is not a dependency of this project` to stderr and leaves `package.json` untouched. Dangling links (1.8) and write-only-when-changed (1.10) were already done. Left for later: pruning orphaned transitive deps (2.5) and updating the lockfile (Phase 3).

**Problems** (`crates/actor/src/remove_package_actor.rs`)
- Dangling symlinks not removed (1.8).
- The package's bins in `node_modules/.bin` are left behind.
- `package.json` is rewritten even when nothing changed; a package not in `package.json` gives no user-facing message.
- Orphaned transitive deps are not pruned (fixed properly by 2.5).

**Fix** — use 1.8 helpers; remove bins whose link target is inside the removed package; only write when changed (1.10); warn `<pkg> is not a dependency`; update lockfile (Phase 3).

### 1.12 `craft run` fixes · P1 ✅ — ✔ done

> Done: scripts run with every ancestor's `node_modules/.bin` in front of `PATH`, `INIT_CWD`, `npm_lifecycle_event`/`_script`, `npm_package_name`/`_version`/`_json` and `npm_config_user_agent=craft/<ver> <platform> <arch>`. `pre<name>`/`post<name>` run around the script, stopping at the first failure, and only the main script gets the arguments, each shell-quoted. Without a matching script, `craft <name>` / `craft run <name>` spawns the nearest `node_modules/.bin/<name>` directly with an argv array, and works without a `package.json`. Otherwise it fails with `script "<name>" not found; available scripts: …` — the raw-shell fallback is gone (`craft exec`, 6.3). A failing script exits craft with its own code (128 + signal when killed) and one `error: <name> exited with code N` line. craft ignores SIGINT while the child runs, so Ctrl-C reaches the script through the terminal's process group and craft exits with the script's code. Not done: adding the `node` binary's directory to `PATH`, and Windows `cmd` quoting beyond the basics (6.4).

**Problems** (`crates/actor/src/run_script_actor.rs`, `crates/process/src/process.rs`)

| Problem | Repro | Location |
|---|---|---|
| `node_modules/.bin` not on `PATH` | `"ver": "semver 1.2.3"` → `sh: semver: command not found` | `process.rs:43-67` |
| Exit code replaced by 1 | `"fail": "exit 3"` → craft exits 1 with `Error: Command failed with status: exit status: 3` | `process.rs:63` + `main.rs` |
| Args joined unquoted into `sh -c` | `craft run show 'a; echo INJECTED'` runs `echo INJECTED` | `run_script_actor.rs:57-60` |
| `pre<script>` / `post<script>` not run | `craft run build` skips `prebuild` | `run_script_actor.rs:49` |
| Requires `package.json` even for bins | `craft tsc` outside a project errors | `run_script_actor.rs:24` |
| No `npm_*` env | scripts reading `npm_package_version`, `npm_lifecycle_event` break | — |

**Fix**
- Prepend to `PATH`: `<cwd>/node_modules/.bin` and every ancestor's `node_modules/.bin` (npm behaviour), plus the directory of the current `node` binary.
- Shell-quote each arg (`shell-escape`/`shlex` on Unix; `cmd` quoting on Windows) before appending to the script.
- Run `pre<name>`, `<name>`, `post<name>`; stop on the first failure.
- Set `npm_lifecycle_event`, `npm_lifecycle_script`, `npm_package_name`, `npm_package_version`, `npm_package_json`, `npm_config_user_agent=craft/<ver>`, `INIT_CWD`.
- Return a dedicated `ScriptFailed { code }` error; `main` calls `std::process::exit(code)` without printing the anyhow chain. Forward signals (Ctrl-C) to the child.
- Bins (`craft <bin>`) should be spawned directly (no shell), with args passed as an argv array.
- Unknown script and no matching bin → `error: script "<name>" not found` plus the available scripts, instead of running it as a raw shell command (decide: keep fallback to `PATH` lookup only for explicit `craft exec`).

### 1.13 Git / tarball / `file:` / `link:` dependencies · P1 📖 — ✔ done

> Done (first half): `ResolvedArtifact.source` is an `ArtifactSource` (`Registry` / `Tarball` / `LocalTarball` / `Directory` / `Link`) that decides the node id (`name@version` for the registry, `name@<url>` / `name@file:<abs path>` / `name@link:<abs path>` otherwise) and the package directory. Local and URL sources are materialized while resolving, since their name, version and dependencies come from their own `package.json`: tarball URLs are downloaded once per URL and extracted by content hash (sha512, recorded as integrity); `file:<tarball>` is extracted by content hash; `file:<dir>` is copied fresh into the cache on every install, without `node_modules`/`.git`, and its registry dependencies are installed; `link:<dir>` is symlinked in place with no dependencies installed. Relative paths resolve against the dependent's directory (`InstallPackage.base_dir`), `~/` against the home directory. `craft add ./libs/x` names the dependency after the package found there and saves `file:libs/x`; URLs and `file:`/`link:` specs are saved verbatim. Also fixed: manifests read from tarballs failed on loosely typed metadata (lodash 4.17.21 has a string `keywords`), so the unused `description`/`keywords` fields are no longer typed. Verified against the real registry and a GitHub codeload tarball.
>
> Done (git): `resolver::git` parses npm's git forms (`github:`/`gitlab:`/`bitbucket:`/`gist:`, `owner/repo`, `git+https`/`git+ssh`/`git+file`/`git://`, scp-style `git@host:o/r`) with `#<branch|tag|sha>` or `#semver:<range>`, and pins the commit with `git ls-remote` (peeled annotated tags, highest semver tag, short shas passed through). Hosted repos are downloaded as the host's tarball of that commit (codeload / GitLab archive / Bitbucket get), with no clone; other repos, or hosted ones whose tarball fails (private), are fetched with `git fetch --depth 1 <sha>` (full fetch as fallback) and copied in without `.git`. Packages are cached per repository + commit (`ArtifactSource::Git`, node id `name@git+<url>#<sha>`), and the spec is saved verbatim. Verified against GitHub (tag, `semver:` range, `git+https` URL, shorthand). Not done: running `prepare` for repos that need a build step (4.3); `workspace:` (Phase 7).

**Problem** — `GitResolver` (`crates/resolver/src/git.rs`) returns the git URL as `download_url`, which reqwest then tries to `GET` — git deps can never install. Version is the literal `"git"`.

**Fix** (each behind the `DependencySpec` variants from 1.1)
- **Tarball URL**: download as-is, read `package.json` from the tarball for name/version/deps. NodeId `name@<url-hash>`.
- **GitHub/GitLab/Bitbucket shorthands**: resolve committish via the host API or `git ls-remote`, download `codeload` tarball (no git needed). Generic git: `git clone --depth 1` into a temp dir, then `git archive` / pack. Run `prepare` if present (Phase 4.3).
- **`file:` dir**: copy (or pack) into the store; **`file:*.tgz`**: treat as tarball. **`link:`**: plain symlink, no store, deps not installed.
- Lockfile records the resolved commit / integrity.

Can be split: tarball URL + `file:` + `link:` first (simple), git hosts second.

### 1.14 Bin linking edge cases · P1 📖 — ✔ done

> Done: `pipes::bins::package_bins` normalizes bins like `npm-normalize-package-bin`: names become their last path segment (`@scope/tool` → `tool`, `../../x` → `x`), targets are clamped inside the package, and without `bin` every file under `directories.bin` becomes a bin. The linker links packages in parallel, then bins sequentially in dependency-name order; a later package declaring a taken bin is skipped with `warning: bin <name> of <pkg> conflicts with <owner>`, so the winner is stable across installs. Targets are relative (since 1.8) and get `+x` added without clobbering other mode bits.

**Problems** (`linker_pipe.rs:203-266`)
- String `bin` on a scoped package uses `@scope/name` as the bin name → tries to create `.bin/@scope/name` (ENOENT). Correct name is the part after `/`.
- ~~Bin source path for transitive deps uses `artifact.name`, not the alias link name~~ (fixed in 1.2: bins resolve through `edge.name`).
- `directories.bin` not supported.
- Bin names/paths from manifests are not sanitized (a bin named `../../x` writes outside `.bin`).
- Two packages providing the same bin race in parallel linking (`buffer_unordered(10)` over roots, `linker_pipe.rs:304-311`) → nondeterministic winner (no longer `EEXIST` since 1.8).

**Fix** — normalize bins in one function (`HashMap<SafeBinName, RelativePath>`), reject names containing `/` or `..`, link bins sequentially (or with a deterministic rule: direct dependency wins over transitive), use relative symlink targets, `chmod 0o755` the target.

---

## Phase 2 — Store & layout redesign

**Why** — today each package's dependency symlinks are written **into the shared global cache**
(`~/.craft/packages/<pkg>/package/node_modules/<dep>`, `linker_pipe.rs:41-90`). Consequences:

- ~~Concurrent installs in different projects race: 2 of 6 parallel installs failed with `File exists (os error 17)`.~~ No longer errors since 1.8's atomic link replacement, but the projects still rewrite each other's links in the shared cache.
- One project's resolution silently rewrites another project's dependency tree.
- ✅ Peer deps can't work: Node resolves symlinks to their real path in `~/.craft`, so a package can never see the project's `react` (`@tanstack/react-query` → `Cannot find module 'react'`).

Target layout (pnpm-style, proven to work with Node's realpath resolution):

```
~/.craft/store/v1/<name>@<version>-<hash12>/   # immutable extracted package (2.1)

project/node_modules/
├── .craft/                                    # virtual store (2.2)
│   ├── react@19.2.0/node_modules/react/              → hardlinks/clones from store
│   ├── @tanstack+react-query@5.90.10_react@19.2.0/   # peer-specific variant (2.3)
│   │   └── node_modules/
│   │       ├── @tanstack/react-query/                → hardlinks from store
│   │       ├── @tanstack/query-core  -> ../../../@tanstack+query-core@5.90.10/node_modules/@tanstack/query-core
│   │       └── react                 -> ../../../react@19.2.0/node_modules/react
│   └── lock.yaml (optional copy of the lockfile for fast "is up to date" checks)
├── .bin/
├── react                  -> .craft/react@19.2.0/node_modules/react
└── @tanstack/react-query  -> ../.craft/@tanstack+react-query@5.90.10_react@19.2.0/node_modules/@tanstack/react-query
```

### Phase 2 design

**Store (2.1)** — `~/.craft/store/v1/<escaped-name>@<version>-<hash12>/`, where `hash12` is the first 12 hex chars of sha256 over the integrity string (registry `dist.integrity`, the computed sha512 for tarball sources, `url#sha` for git). Readable when debugging, collision-free across registries. Entries are extracted atomically (1.6) and never written to again; the downloaded `.tgz` is deleted once extracted. `file:` directories and `link:` targets don't go through the store. The old `~/.craft/packages/` is ignored (no migration); `craft cache clean` removes both.

**Virtual store (2.2)** — each graph node becomes `node_modules/.craft/<id>/node_modules/<name>/`:
- `<id>` = `<escaped-name>@<version>` (`@scope/x` → `@scope+x`); non-registry nodes use `name@<source>+<hash12>`; peer variants (2.3) append `_<peer>@<ver>…`. Ids longer than 120 chars are truncated + hashed, like pnpm.
- Files are imported from the store per file: **clone** (APFS `clonefile`, Linux `FICLONE` via `reflink-copy`) → **hardlink** → **copy**, falling back once per install when a method isn't supported (e.g. cross-device). Clones mean editing a file in `node_modules` never corrupts the store; hardlinks (ext4) share it, as pnpm accepts.
- A node's dependencies are relative symlinks next to it: `.craft/<id>/node_modules/<dep-name> -> ../../<dep-id>/node_modules/<dep-pkg>`. Aliases use the edge name as the link name.
- Root edges: `node_modules/<edge.name> -> .craft/<id>/node_modules/<pkg>` (relative). `link:` roots stay direct symlinks to their target.
- Import is staged and renamed, and skipped when the entry exists (ids are immutable; `file:` directory nodes are re-imported every install).
- **Hidden hoisting**: every package name is also linked once into `node_modules/.craft/node_modules/<name>` (highest version wins). Node's lookup from inside `.craft/<id>/…` reaches that directory, so packages that `require` an undeclared dependency (phantom deps, common in older packages) still work — pnpm's default `hoist-pattern=*`. Root `node_modules` stays strict: only direct deps are visible to the project.
- Every write is under the project's `node_modules`, so projects can't affect each other; the shared store is append-only.

**Bins (2.4)** — `.craft/<id>/node_modules/.bin` per node for its dependencies (lifecycle scripts in 4.3 need them); root `.bin` only for direct dependencies, as now.

**Prune (2.5)** — after linking: remove `.craft/<id>` entries and root `node_modules` entries (and their bins) that aren't in the graph. Replaces leftover links from the old layout too.

**Peers (2.3)** — after the normal resolution pass, a second pass walks from the roots: a node's peer resolves to what its parent can see (the parent's dependencies, then the parent's own resolved peers, recursively); unresolved non-optional peers are auto-installed at the top level (npm ≥ 7). A node is duplicated per distinct peer resolution, which is what the `_peer@ver` id suffix encodes. Unmet ranges warn; `--strict-peer-dependencies` fails.

**Order**: 2.1 store → 2.2 virtual store + 2.4 bins → 2.5 prune → 2.3 peers. Each lands green on its own; the e2e suite already covers the observable behaviour, plus new tests for isolation (two projects, different versions, same store), deleting `~/.craft` after install, phantom deps via hidden hoisting, and peers.

**Decisions (confirmed)**
1. Import method: clone → hardlink → copy.
2. Hidden hoisting into `.craft/node_modules` on by default.
3. No migration of `~/.craft/packages`; it just stops being used.

### 2.1 Global content-addressed store · P0 — ✔ done

> Done: packages are extracted to `~/.craft/store/v1/<name>@<version>-<hash12>` (`@scope/x` → `@scope+x`; hash of the integrity, or of the tarball URL when a registry gives none), and tarball/`file:`/git sources to `tarball-`/`file-`/`git-<hash12>`. Entries hold the package files directly (no `package/` subdir). Registry tarballs are downloaded beside their entry and deleted once extracted, so a store entry is only ever produced from a verified download; the cached-tarball recheck from 1.7 went with them. `craft cache clean` removes the store and the old `~/.craft/packages`. Each process downloads to its own `<entry>.<pid>.tgz`: with one shared name, concurrent installs into an empty store deleted each other's tarball before extracting it (found by the real-world suite; `concurrent_installs_into_an_empty_shared_store_succeed` reproduces it every run). Until 2.2 the linker still writes each package's dependency links inside its store entry, so entries are not yet immutable.

- Extracted packages live in `~/.craft/store/v1/` keyed by integrity (or `name@version` + integrity check), written atomically (1.6), never modified afterwards (optionally `chmod -w`).
- Tarballs can be deleted after extraction (or kept under `~/.craft/cache/tarballs` for `cache clean` granularity).
- `craft cache clean` targets the store; add `craft cache dir` / `craft cache prune` (remove entries unused by any lockfile — later).

### 2.2 Per-project virtual store · P0 — ✔ done

> Done: `LinkerPipe` lays every node out as `node_modules/.craft/<entry>/node_modules/<name>` (`pipes::virtual_store`: `<name>@<version>`, `@scope+x`, `name@file+<hash>` / `@tarball+` / `@git+` for other sources, truncated + hashed past 120 chars). Store entries are imported once with `fsutil::import_dir` (clone → hardlink → copy via `reflink-copy`, stepping down once per process when a method fails); `file:` directories are re-imported from their source every install with clone → copy only, never hardlinked. Dependencies are relative symlinks beside each package, root dependencies are relative links into `.craft`, `link:` targets stay absolute. Every package name is hoisted once into `.craft/node_modules` (the project's own dependency, else the highest version), so undeclared requires resolve while the project's root stays strict. Nothing is written outside the project, so the store is now immutable. Verified: concurrent installs 20/20; two projects on one store keep different transitive versions; the project keeps working after `~/.craft` is deleted; on APFS a second 80 MB project cost ~7 MB of disk (clones); linking a 218-package tree takes ~60 ms. The peer scenario (`@tanstack/react-query` + root `react`) now passes through the project root and hoisting; per-subtree peer variants are still 2.3.

- Materialize each graph node into `node_modules/.craft/<escaped-id>/node_modules/<name>`:
  macOS `clonefile` (APFS), Linux `ioctl FICLONE` → hardlink → copy fallback; Windows hardlink → copy.
  (`reflink-copy` crate covers clone + fallback.)
- Dependencies of a node are relative symlinks siblings inside the same `node_modules/` (Windows: junctions, 6.4).
- Root edges link `node_modules/<link_name>` → virtual store entry.
- All writes are project-local → no cross-project races, no global locks needed beyond the store.
- `bundleDependencies`: already inside the package's tarball — don't overwrite them.
- Optional later: `public-hoist-pattern` (hoist `*eslint*`, `*prettier*`) and `node-linker=hoisted` for tools that break under symlinks.

**Done when** the concurrent-install scenario passes 20/20 and deleting `~/.craft` doesn't break an already-installed project if materialization used copies/hardlinks.

### 2.3 Peer dependency resolution · P0 ✅ — ✔ done

> Done: `InstallPipe` records each package's peers (`PackageJson::peer_dependencies`, with `peerDependenciesMeta` optional flags) and resolves every required peer on its own as a fallback candidate. A pure pass (`pipes::peers::resolve_peers`) then rewrites the graph from the roots: a peer resolves from the parent's `node_modules` (its dependencies and its own resolved peers), then the project's dependencies, then the candidate (npm ≥ 7 auto-install); optional peers are linked only when provided; a peer the package also depends on is just that dependency. Each distinct resolution is its own instance, `NodeId::with_peers` → `ui@1.0.0_react@18.3.1`, which names its virtual store entry, and peers are `DependencyKind::Peer` edges linked beside the package. Unmet ranges warn (`… wants peer react@^18 || ^19 but gets 17.0.2`). Because a package's scope is exactly its own dependencies and peers (plus the project root), the instance id determines the whole subtree, so instances are memoized; mutual sibling peers are cycle-guarded. Verified with React Redux + Redux Toolkit + React Router + React Query: hooks render through `renderToString` with a single React instance. Not done: `--strict-peer-dependencies` (6.3).

**Problem** — `peerDependencies` are ignored entirely (`install_pipe.rs:95` only reads `dependencies`).

**Fix**
- During resolution, a peer edge resolves to whatever the **parent** provides (the parent's own dependency or the parent's peer, recursively); if nobody provides it, auto-install the peer (npm ≥ 7 behaviour) at the highest satisfying version.
- Respect `peerDependenciesMeta[x].optional` (don't auto-install, don't warn).
- Packages whose peers resolve differently in different parts of the tree get separate virtual-store entries (`name@ver_peer@ver` suffix).
- Warn (not fail) on unmet peer ranges; `--strict-peer-dependencies` to fail.

**Done when** `require('@tanstack/react-query')` works with `react` as a root dependency and the same project builds with two different `react` peers in two subtrees.

### 2.4 Bins inside the virtual store · P1 — ✔ done

> Done with 2.2: each `.craft/<entry>/node_modules/.bin` gets its package's dependency bins (same normalization and conflict rule as 1.14); the root `.bin` holds direct dependencies only.

- Each virtual-store `node_modules/` gets a `.bin` for its node's dependencies, so lifecycle scripts (4.3) and nested tools find their bins.
- Root `node_modules/.bin` only for direct dependencies (npm also exposes transitive bins at root when hoisted — decide and document).

### 2.5 Prune extraneous packages · P1 — ✔ done

> Done: `pipeline::prune_node_modules` runs after every link. It removes top-level symlinks for packages outside the graph (old-layout leftovers included; real directories and dot entries such as `.cache` are left alone), then every `.craft/<entry>` no top-level link reaches by following each entry's dependency links (staging leftovers too), then dangling hoist links and root bins, and empty `@scope` directories. `craft remove` runs the same reachability pass without the network, so orphaned transitive dependencies go with the package while shared ones stay. `craft add` now installs the project's existing dependencies together with the added ones (an added name replaces the existing entry), since pruning needs the full graph — as npm does. Verified on the Vite/React project: `craft remove eslint` took the virtual store from 218 to 135 entries, re-adding restored 218 and `eslint` ran.

- After linking, delete root `node_modules/*` entries and `.craft/*` entries not in the current graph (respecting scoped dirs and `.bin`).
- Makes `craft remove` and editing `package.json` + `craft install` correct.

---

## Phase 3 — Lockfile

`crates/lockfile` exists (JSON + bincode, `Lockfile { version, packages }`), but nothing depends on it.
README says "`package-lock.json` support (coming soon)" — decide whether that means *reading* npm's lockfile (Phase 7) or Craft's own.

### 3.1 Lockfile format · P0 — ✔ done

> Done: the `lockfile` crate was rewritten as format types only (bincode and the old `PackageEntry` dropped). `craft.lock` is pretty JSON with sorted maps and a trailing newline: `lockfileVersion`, `importers["."]` (`dependencies` / `devDependencies` → `{ specifier, version }`), and `packages` keyed by instance id (`ui@1.0.0_react@19.2.0`) with `name`, `version`, a tagged `resolution` (`registry` with tarball + integrity, `tarball`, `localTarball`, `directory`, `link`, `git` with commit) and `dependencies` / `peerDependencies` as package keys. Local paths and the keys of local packages are relative to the project, so the file is identical on every machine. A newer `lockfileVersion` is rejected with an upgrade hint. `os` / `cpu` / `optionalDependencies` / `hasBin` come with Phase 4.

- File: `craft.lock` at project root, **text** (JSON or YAML) with stable, sorted keys (`BTreeMap`), trailing newline — it must be reviewable in PRs. Drop bincode as the primary format (optionally keep a binary cache under `node_modules/.craft` for speed).
- Contents:

```jsonc
{
  "lockfileVersion": 1,
  "importers": {
    ".": {
      "dependencies":    { "react": { "specifier": "^19.2.0", "version": "react@19.2.0" } },
      "devDependencies": { "eslint": { "specifier": "*", "version": "eslint@9.39.1" } }
    }
  },
  "packages": {
    "react@19.2.0": {
      "resolved": "https://registry.npmjs.org/react/-/react-19.2.0.tgz",
      "integrity": "sha512-…",
      "dependencies": { "loose-envify": "loose-envify@1.4.0" },
      "peerDependencies": {}, "optionalDependencies": {},
      "os": [], "cpu": [], "hasBin": false, "hasInstallScript": false
    }
  }
}
```

- `importers` keeps room for workspaces (Phase 7).

### 3.2 Write lockfile · P0 — ✔ done

> Done: `pipeline::lockfile_from_graph` serializes the peer-resolved graph, taking each importer specifier from package.json (so `craft add ms` locks `^2.1.3`, not `ms`). `install` and `add` write it after linking; `remove` edits the existing lockfile directly — dropping the importer entry and every package nothing else reaches — without touching the network. Writes are atomic and skipped when unchanged (`fsutil::write_if_changed`).

- Serialize the `ResolutionGraph` (1.2) after every successful `install`/`add`/`remove`; write atomically; only write when contents changed.

### 3.3 Install from lockfile · P0 — ✔ done

> Done: `resolver::LockPreferences` answers registry requests with the highest locked version satisfying the range (a dist-tag takes the highest locked version) and git URLs with the locked commit, before any registry or `git ls-remote` call. Locked artifacts carry no manifest; `InstallPipe::fetch` reads it from the store entry, downloading by locked tarball + integrity if needed. Project dependencies whose specifier changed — and every package passed to `craft add` — resolve afresh (`InstallPackage::without_lock`), while their subtrees still prefer locked versions; the peer pass simply reruns. After each install a copy of the lockfile goes to `node_modules/.craft/lock.json`; when package.json, craft.lock and that copy agree (and no `file:` directory needs re-copying), `craft install` prints `Already up to date`. Measured on the Vite/React project: up to date 6 ms; `node_modules` deleted with a warm store 0.4 s; fresh checkout with an empty store 2.4 s (tarballs only); resolving without a lockfile 13.5 s.

- If every importer specifier in `package.json` matches the lockfile, skip all registry metadata requests: download/verify by `resolved` + `integrity` only.
- If some specifiers changed, keep locked versions for unchanged deps and re-resolve only the changed subtrees.
- Fast path: if `node_modules/.craft/lock` equals `craft.lock`, print "Already up to date" and exit (measured warm install today: ~3.3 s, all from re-fetching packuments).

### 3.4 `--frozen-lockfile` / `craft ci` · P1 — ✔ done

> Done: `craft install --frozen-lockfile` (on by default when `CI` is set and not `0`/`false`, via `Context::ci`) fails before any request when craft.lock is missing (`craft.lock is missing; run `craft install` to create it`) or disagrees with package.json, listing every difference (`debug (^4.4.0 in package.json, ^4.0.0 in craft.lock), ms (not in craft.lock), react (not in package.json)`). The resolver then runs frozen: a registry or git request the lockfile can't answer is an error (`ms@^2.1.3 is not in craft.lock`) instead of a metadata fetch, and the lockfile is never rewritten. `craft ci` removes `node_modules` and runs a frozen install. The e2e harness clears `CI` so the suite behaves the same on CI runners.

- `--frozen-lockfile`: fail if the lockfile is missing or out of date with `package.json`. Default on when `CI=true`.
- `craft ci`: remove `node_modules`, then frozen install.

---

## Phase 4 — Platform packages & lifecycle scripts

~~Today `esbuild` installs but `require('esbuild')` fails: its platform binary ships as an optional dependency (`@esbuild/darwin-arm64`) and its `postinstall` never runs. Same class: `@rollup/rollup-*`, `@swc/core-*`, `lightningcss-*`, `sharp`, `@next/swc-*`, `bcrypt`, `sqlite3`.~~ Platform binaries install since 4.1/4.2; packages that build or download in `postinstall` (`bcrypt`, `sqlite3`, `sharp` < 0.33) still need 4.3.

### 4.1 `optionalDependencies` · P0 — ✔ done

> Done: packages' `optionalDependencies` resolve as `DependencyKind::Optional` edges (overriding a same-named `dependencies` entry, as npm does); a failing one is skipped with `warning: skipping optional dependency <pkg> of <dependent>: <cause>`. The lockfile records them under `optionalDependencies`. Not done: `optionalDependencies` in the project's own package.json (and `craft add -O`), `engines` / `deprecated` warnings.

- Add `optional_dependencies` (and `peer_dependencies`, `peer_dependencies_meta`, `os`, `cpu`, `libc`, `has_install_script`, `deprecated`, `engines`) to the manifest struct (`crates/package/src/npm.rs`).
- Resolve optional deps; any failure (resolve, download, platform mismatch, install script) skips that subtree with a warning instead of failing the install.

### 4.2 `os` / `cpu` / `libc` filtering · P0 — ✔ done

> Done: `contract::HostPlatform` names the host the Node way (`darwin`/`linux`/`win32`, `x64`/`arm64`/…, `glibc`/`musl` from `/lib/ld-musl-*` on Linux) and can be overridden with `CRAFT_OS` / `CRAFT_CPU` / `CRAFT_LIBC` (e.g. installing linux binaries while building a container image on a Mac). `package::PlatformConstraints` reads `os`/`cpu`/`libc` with npm's rules (`!` negation, libc only on Linux) and travels on `ResolvedArtifact` and in the lockfile. Packages for another platform are resolved and locked but never downloaded, imported, linked, hoisted or given bins, so one lockfile installs on every platform; a non-optional dependency on such a package fails (`mac-only@1.0.0 does not support linux-x64-glibc, but the project requires it`). Verified: `vite build` (rollup's 25 platform variants locked, one installed), and esbuild, `@swc/core` and `lightningcss` load their native binaries. Known gap: a foreign-platform package's own dependencies are not walked, so re-resolving drops them from the lockfile (binary packages rarely have any).

- Skip packages whose `os`/`cpu` (supports `!` negation) don't match the host (`std::env::consts::{OS, ARCH}` mapped to Node names: `darwin`, `linux`, `win32`; `x64`, `arm64`, `ia32`).
- `libc` (`glibc`/`musl`) detection on Linux.
- Lockfile keeps all platform variants so it's portable across machines; filtering happens at link time.

### 4.3 Lifecycle scripts with an allowlist · P0 — ✔ done

> Done (decided: deny by default, top-level `trustedDependencies` as in Bun): `pipeline::LifecyclePipe` runs `preinstall` / `install` / `postinstall` (and an implicit `node-gyp rebuild` for a `binding.gyp` without an install script) only for packages named in package.json `trustedDependencies`; every other package with scripts is listed after the install with how to trust it. Builds run once per virtual store entry (a `.built` marker; a failed build removes the entry so the next install retries), dependencies before dependents (by depth, cycles cut), up to 8 in parallel, with output captured and shown on failure or with `--verbose`. Before its first build a package is re-imported by copy, so scripts never write through hardlinks into the store. Scripts get the `craft run` environment (`process::script_env`: every `node_modules/.bin` upwards on `PATH`, `npm_*`, `INIT_CWD`). A failing build of a package reached only through optional dependencies is a warning; otherwise the install fails with the script's output. The project's own `preinstall` / `install` / `postinstall` / `prepare` run after dependencies on `craft install` / `craft ci`. `--ignore-scripts` (install, add, ci) disables all of it. The script policy is part of the installed state, so trusting a package makes the next `craft install` build it. Verified: `bcrypt` builds and hashes; untrusted `esbuild` is reported and still works through its platform binary. Not done: `prepare` for git dependencies, a `craft trust` command, and Bun's default-trusted list.

- Dependency scripts: `preinstall`, `install`, `postinstall` (and implicit `node-gyp rebuild` when `binding.gyp` exists and there's no `install`/`preinstall`), run after linking in **dependency order** (deps before dependents), with the virtual-store `.bin` on `PATH` and `npm_*` env (reuse 1.12).
- Root project: `preinstall`, `install`, `postinstall`, `prepare`.
- Security: install scripts of dependencies run **only** for packages listed in `package.json` (`"craft": { "trustedDependencies": [...] }`, like Bun/pnpm `onlyBuiltDependencies`); print which packages were skipped and how to allow them. `--ignore-scripts` disables everything.
- Run scripts in parallel where the graph allows, with output prefixed by package name; failure of a non-optional package fails the install.

---

## Phase 5 — Network & performance

Measured: cold install of this repo's `package.json` ≈ 4.8 s, 149 MB RSS; warm ≈ 3.3 s.

**Benchmark findings (2026-10-25).** `make bench` (`bench/README.md`) races craft against npm, pnpm and Yarn. craft wins already-installed projects by 6–140×, but loses every warm-cache scenario to pnpm by 2–6×, cold installs of `large` by 2.6× (an artefact: pnpm was reaching its existing store, see 5.14), and peaks at 5.5 GB without a lockfile (pnpm: 663 MB). Profiling `large` (1,391 packages, 112,916 files) traced it to four causes, fixed by 5.8–5.13. A second round after 5.8 (2026-10-26) added 5.14–5.17; work through them as 5.14, 5.15, 5.16, 5.9, 5.17, then 5.10–5.13. After each, run `make` and `make bench`, and update the tables in `bench/README.md`. Memory budget for `large`: under 500 MB peak without a lockfile, under 150 MB with one; every new concurrent stage reuses an existing limit.

### 5.1 Abbreviated packuments · P2 — ⏸ deferred, superseded by 5.9

> Deferred after measuring: the registry's abbreviated ("corgi") manifests drop `libc` (so `@rollup/rollup-linux-x64-musl` and `-gnu` become indistinguishable on Linux, breaking 4.2) and `scripts` (only `hasInstallScript` remains), and with gzip from 5.4 the saving is small — `typescript` 2.02 MB full vs 1.70 MB abbreviated compressed, `@types/node` 1.43 vs 1.11 MB. Revisit with a fallback to the full manifest for packages whose `os` includes `linux`. Meanwhile install scripts are read from the installed package's own package.json, which also covers registries that serve abbreviated metadata.

- Send `Accept: application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8` (`network.rs:22`). `typescript` packument: 15.8 MB full vs 8.7 MB abbreviated. Abbreviated manifests still include `bin`, `dist.integrity`, `optionalDependencies`, `os`/`cpu`, `hasInstallScript`.
- `NpmPackage` needs `#[serde(default)]` on optional fields; make manifest fields tolerant (old packages have `keywords` as a string, `dependencies` as an array, etc.) — use `serde_json::Value` or lenient deserializers for fields that aren't needed, so one bad historical version doesn't fail the whole packument.

### 5.2 Memoize packument fetches per name · P2 — ✔ done

> Done: `NpmResolver` keeps one `tokio::sync::OnceCell<Arc<NpmPackage>>` per package name, so every range requesting a package — concurrently or not — shares a single fetch; a failed fetch isn't cached and is retried by the next request. Measured on the Vite/React project without a lockfile: 276 packument requests, one per distinct name (it was 415 for a smaller tree before; the tree grew with 4.1's platform variants).

- Today every `(name, range)` pair fetches the full packument again (`npm.rs:23`) — measured: 415 packument requests for a 218-package tree (`debug` ×11, `@babel/types` ×11). Cache `Arc<Packument>` per name in a `DashMap<String, OnceCell<…>>` (or `futures::Shared`) so concurrent requests for the same name share one fetch.

### 5.3 Global concurrency limits · P2 — ✔ done

> Done: each `Network` client (one for metadata, one for tarballs) holds a semaphore of `network-concurrency` permits (pnpm's `.npmrc` key, default 16) for the whole request including its body, so the nested `buffer_unordered` of resolution can no longer open connections without bound; retries wait outside the permit. Tarball extraction is capped at the number of CPU cores. Verified with 30 dependencies at `network-concurrency=2`: at most 4 requests in flight. Vite/React project without a lockfile: 3.1–7.0 s; fresh store with a lockfile: 2.0 s.

- `buffer_unordered(10)` nests at every recursion level (`install_pipe.rs:106`) → effectively unbounded parallel requests. Replace with a `tokio::sync::Semaphore` for metadata (e.g. 64) and one for tarballs (e.g. 16), configurable via `network-concurrency`.
- Also cap blocking extraction tasks.

### 5.4 Timeouts, retries, User-Agent · P2 — ✔ done

> Done: reqwest 0.11 → 0.12 with `gzip` + `brotli`, a 10 s connect timeout and a 60 s read (stall) timeout rather than a whole-request cap, and `User-Agent: craft/<version> <os> <arch>` (`contract::user_agent`, shared with scripts' `npm_config_user_agent`). Every GET — packuments and tarballs, body included — is retried on connection errors, timeouts, interrupted bodies and 408/429/500/502/503/504, `fetch-retries` times (npm's `.npmrc` key, default 2), with exponential backoff from 500 ms plus jitter, or the server's `Retry-After` (capped at 30 s); 404 and auth errors fail at once. Measured on the Vite/React project without a lockfile: 4.4–8.4 s (was 8.3–16.8 s), mostly from compressed packuments. Not done: `rustls-tls` (still native TLS).

- `reqwest::Client::builder()` with connect + read timeouts, gzip/brotli, HTTP/2, `User-Agent: craft/<version> <os> <arch>`.
- Retry idempotent requests on connect errors / 429 / 5xx with exponential backoff + jitter (3 attempts), honouring `Retry-After`.
- Upgrade `reqwest` 0.11 → 0.12; consider `rustls-tls` to avoid OpenSSL in release builds.

### 5.5 Streaming downloads · P2 — ✔ done

> Done: `Network::download` streams the body chunk by chunk into a temp file beside the destination while an incremental `IntegrityHasher` checks it, and renames the file into place only after the hash matches; a retry starts a fresh temp file and a failure or mismatch leaves nothing behind. Request permits are held until the body is fully read. Measured installing `next` + React + TypeScript from a lockfile into an empty store: peak RSS 197 MB → 21 MB, same wall time. Extraction still reads the downloaded file rather than the stream.

- `response.bytes()` buffers whole tarballs in memory (`network.rs:30`). Stream chunks to the temp file while updating the integrity hasher (1.7); or stream straight into `GzDecoder` + `tar` and extract without writing the `.tgz`.

### 5.6 Metadata disk cache · P2 — ✔ done

> Done: packuments are cached under `~/.craft/cache/metadata/<registry hash>/<name>.json` with a `.meta.json` holding `ETag`, `Last-Modified`, `max-age` and when they were fetched. Online, a cached packument within its `max-age` is used as is; otherwise it is revalidated with `If-None-Match` / `If-Modified-Since` and a `304` reuses it. `--prefer-offline` (or `prefer-offline=true`) uses any cached copy without asking; `--offline` (or `offline=true`) never touches the network and reports `<url> is not cached and craft is offline`, tarballs included. `craft cache clean` removes the cache. Measured on the Vite/React project without a lockfile: 6.6 s with a cold cache, 0.8 s warm (the npm registry sends `max-age=300`), 0.8 s `--offline`. Follow-up: the cache stores uncompressed JSON (138 MB for that project); compress or abbreviate it.

- Cache packuments under `~/.craft/cache/metadata/` with `ETag`/`Last-Modified`; revalidate with `If-None-Match` (304). `--prefer-offline` / `--offline` flags.

### 5.7 Registry configuration · P1 — ✔ done

> Done: `contract::RegistryConfig` layers `~/.npmrc` (or `NPM_CONFIG_USERCONFIG`), the project's `.npmrc`, then `npm_config_*` variables, with `CRAFT_REGISTRY` overriding the default registry. It reads `registry`, `@scope:registry`, `//host/path/:_authToken`, `:_auth`, `:username` + `:_password`, legacy top-level `_authToken`, `https-proxy` / `proxy` / `noproxy`, `strict-ssl` and `cafile`, with `${VAR}` expansion and quoting. `Network` builds its client from it and sends each URL only the credentials of its longest matching `//host/path/` prefix, so tarballs on a private host are authenticated and other hosts never see the token; credentials print as `<redacted>`. Scoped packuments are requested as `@scope%2fname`. Verified: a token-protected mock registry via `${PRIVATE_TOKEN}`, a scoped registry with its own token while the public one never sees it, `--verbose` never printing the token, and JSR (`@jsr:registry=https://npm.jsr.io`) against the real network. Not done: `always-auth`, client certificates (`certfile`/`keyfile`), and `.npmrc` sections.

- Registry is hardcoded (`npm.rs:23`). Read `.npmrc` (project, user `~/.npmrc`, env `npm_config_*`): `registry`, `@scope:registry`, `//host/:_authToken`, `_auth`, `always-auth`, `strict-ssl`, `cafile`, proxies (`https-proxy`, `HTTPS_PROXY`).
- URL-encode scoped names: `@scope%2fname`.
- Never log tokens (verbose mode prints URLs).

### 5.8 Clone whole store entries · P2 — ✔ done

> Done: on macOS, `fsutil::import_dir` clones a store entry into its staging directory with one `clonefile` call (through `reflink-copy`) and renames it in as before. When the filesystem can't (not APFS, another volume) it falls back to the per-file clone → hardlink → copy path for the rest of the process; other failures fall back for that package only. Copies that skip entries (`file:` directories) stay per file. Measured with `make bench` (warm cache and lockfile): `large` 9.03 s → 1.97 s (pnpm 1.85 s), `medium` 756 → 254 ms (pnpm 271 ms), `small` 286 → 135 ms (pnpm 212 ms); without a lockfile `large` 12.78 → 5.54 s. Memory unchanged. Linux is unchanged and still unmeasured.

Warm-cache installs spend their time in the kernel: `fsutil::copy::copy_tree` clones file by file, 112,916 `clonefileat` calls for `large`. A warm-cache install with a lockfile measured 16 s wall, 0.5 s user, 48 s sys and 491k involuntary context switches. Cloning the same store per file took 15.9 s on one thread and about 9 s on 4–16 threads; `clonefile()` once per package directory took **1.15 s** for all 1,246 entries.

- On macOS, import a store entry with one `clonefile(entry, staging)` and keep the staging-then-rename publish. Fall back to the per-file path when it fails (another volume, not APFS).
- Linux keeps per-file reflinks or hardlinks; get a Linux baseline from the Benchmark workflow before tuning it.
- Packages whose install scripts run keep getting a real copy (`replace_with_copy`).
- Expected: `large` warm cache with a lockfile, 9 s → about 2 s.

### 5.9 Lean packuments · P2

Without a lockfile, craft peaks at 5.5 GB on `large`. It requests full packuments (no `Accept` header): 1.1 GB on disk for 1,280 names, with vite 37 MB, next 30 MB and firebase 29 MB. It parses every version into `PackageJson`, whose `#[serde(flatten)] other: HashMap<String, Value>` turns each field into a JSON tree. Then it keeps all of them in `NpmResolver.packuments` until the install ends. Abbreviated metadata is far smaller (vite 37 MB → 2.3 MB, firebase 29 → 9.6 MB), but not always (next 30 → 25 MB, from thousands of canaries), so parsing must get lazier too.

- Send `Accept: application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*`, and give the metadata cache a new directory for the new format.
- Parse `versions` as raw JSON (`Box<RawValue>`); select from the keys and `dist-tags`; fully parse only the chosen version into a typed manifest without a flattened catch-all map. Keep only that per name.
- Store it like pnpm's `pnpm-meta-v1` cache: each file starts with a small index of `dist-tags` and every version with the byte offset and length of its manifest, so resolution reads the index, picks a version, and parses only that version's bytes.
- Measured 2026-10-26 (`large`, fresh cache): resolution takes 2.6 s and about 3 s of CPU, almost all of it reading and parsing 1.1 GB of JSON; pnpm resolves the same tree in about 0.15 s from its indexed cache. Expected: a few hundred milliseconds, peak memory under 500 MB.
- `libc`, which 5.1 was deferred over: abbreviated manifests lack it, so a package for the other Linux libc is downloaded, then skipped once its own `package.json` shows `libc`. That costs a few small downloads on Linux and never installs the wrong package. Scripts are already read from the installed `package.json`.

### 5.10 Fetch tarballs off the resolution path · P2

`InstallPipe::resolve_package` downloads and extracts a package before resolving its dependencies, so a cold install's critical path is tree depth × (metadata request + download + extraction): 31–37 s for `large`, level with Yarn's 30 s, while 5.11's empty-cache install with a lockfile shows the same waterfall at 20 s against npm's 12 s.

- Resolve dependencies from the packument as soon as a version is chosen; start each tarball's fetch as its own task, bounded by the network and extraction permits, and join them all before linking.

### 5.11 Install straight from the lockfile's graph · P2

`craft.lock` records every package's resolved `dependencies`, `peerDependencies` and `optionalDependencies`, but `LockPreferences` keeps only versions and returns `package: None`. A package's dependencies are therefore found by extracting it and reading its `package.json`, the same waterfall again: 22 s for `large` with an empty cache, against npm's 13.5 s.

- When package.json matches craft.lock, build the `ResolutionGraph` from the lockfile alone, fetch every missing tarball in one parallel pass (5.10), and read bins from the store while linking. When only some dependencies changed, re-resolve just those.
- Test with the mock registry: a lockfile install makes no packument requests and downloads in parallel.

### 5.12 Extract while downloading · P2

Each tarball is written to disk, reopened and extracted, with a `create_dir_all` and a separate `set_permissions` per file.

- Decompress and unpack the response stream into the staging directory while hashing the compressed bytes; rename into the store only once the integrity matches.
- flate2's `zlib-rs` backend; create files with their mode; skip directories already created.

### 5.13 One network client, fewer manifest reads, no graph clones · P2

- `Resolver` and `NpmResolver` each build a `Network`: two connection pools and two `network-concurrency` semaphores. Share one.
- `LifecyclePipe::pending` reads `package.json` for every package on every install (1,243 for `large`); filter by `trustedDependencies` first.
- The graph is cloned whole into the linker and the scripts pass, every node carrying its manifest; borrow it or share it behind an `Arc`.
- Then tune metadata concurrency over HTTP/2 against the benchmark rather than by guess.
- `bench/report.mjs`: report peak memory for every fixture, so each change is judged on time and memory.
- The scripts scan is 0.15 s of a 1.97 s install on `large` with a lockfile (measured 2026-10-26).

### 5.14 Benchmark: isolate pnpm's store, add a stale-cache scenario · P2 — ✔ done

> Done: `bench/run.sh` gives pnpm `PNPM_HOME` inside its own home and unsets `npm_config_*`, `PNPM_*`, `YARN_*` and `COREPACK_*` from the caller's environment. The new `stale` scenario copies each home right after setup, waits until the registry's 300 s `max-age` has passed, and restores the copy before every run. Re-measured 2026-10-26: pnpm's cold install of `large` is 43.65 s, not 14.26 s, and craft (31.08 s) is level with Yarn (29.80 s); past the `max-age`, `large` takes craft 9.72 s, npm 40.50 s, Yarn 10.46 s and pnpm 1.79 s.

pnpm keeps its store under `PNPM_HOME`, not `HOME`, so a `PNPM_HOME` exported by the shell leaks into the benchmark: every run so far used and wrote to the user's own `~/Library/pnpm/store/v11` (25 GB). pnpm's cold and lockfile-with-empty-cache rows therefore had its tarballs already; its warm rows are unaffected.

- Set `PNPM_HOME` to the package manager's own home in `install_command`, and check for other variables that leak the same way (`npm_config_*`, `YARN_*`, `COREPACK_*`).
- Re-run the cold and lockfile scenarios and replace those rows in `bench/README.md`.
- Add a scenario whose metadata cache is older than the registry's `max-age` (300 s). In the warm-cache scenario every run lands within it, which hides that craft then revalidates every packument: 22–24 s on `large`, against pnpm's 2.0 s.

### 5.15 Skip freeing memory at exit · P2 — ✔ done

> Done: `InstallPipe::into_graph` resolves the graph and then leaks the pipe, with the resolver's packuments, instead of dropping it; `install`, `update` and `add` use it. Measured with `make bench` (no lockfile): warm cache `large` 5.54 → 4.47 s, `medium` 1.37 → 1.02 s, `small` 307 → 263 ms; past the `max-age` `large` 9.72 → 8.50 s. Peak memory unchanged (5.9). While measuring, the `large` warm-cache runs turned out to cross the registry's `max-age` mid-run, since setting up all four package managers takes about 4.7 minutes: `bench/run.sh` now times each package manager on its own and sets it up again right before its warm-cache runs.

Without a lockfile, `large` spends about 1 s after its last phase dropping 5.8 GB of parsed packuments and the graph. Leaving the resolver undropped took the install from 5.62 s to 4.65 s.

- At the end of a command, leak the resolver and the graph (`std::mem::forget`) instead of dropping them; the process exits right after and the OS reclaims the memory. Keep the lockfile and state writes before it.
- Still worth it after 5.9, which shrinks the memory but not the graph.

### 5.16 Link while importing, prune only what changed · P2 — ✔ done

> Done: `LinkerPipe` creates every dependency link, the hoisted links and the root links while the imports run, then links bins once the imports are in (linking a bin makes its file executable). Hoisting runs concurrently instead of one link at a time. Pruning is skipped when `node_modules` didn't exist before the install; otherwise `prune_to_graph` keeps exactly the graph's entries and roots, without following every link in the virtual store. `craft remove` still prunes by reachability. Measured with `make bench` (warm cache and lockfile): `large` 1.97 → 1.72 s (pnpm 1.80 s), `medium` 254 → 217 ms (pnpm 246 ms), `small` 135 → 103 ms (pnpm 128 ms); without a lockfile, `large` 4.47 → 4.29 s. Short of the 1.3 s expected: dependency links compete with the clones and finish only about 60 ms after them. What's left besides the clones is the scripts scan (0.155 s) and copying the graph into the linker (about 0.06–0.1 s), both in 5.13.

After 5.8, `large` with a warm cache and a lockfile takes 1.97 s against pnpm's 1.85 s. Measured breakdown: resolving from the lockfile 60–90 ms, cloning 1,249 entries 1.22–1.35 s, linking dependencies 0.19 s, hoisting 0.14 s, pruning 0.20 s, the scripts scan 0.15 s (5.13). The clone is a floor: one `clonefile` per entry takes 1.15–1.20 s on 1 to 32 threads, as APFS serializes them (hardlinking the same files took 13.5 s). pnpm 12 is a native binary that calls the same `clonefile`, so craft wins only by cutting the rest.

- Link each entry's dependencies as soon as that entry is imported, and run hoisting and the root links concurrently, not one link at a time. Symlinking mostly overlaps with clones: 1.20 s + 0.26 s apart, 1.35 s together.
- Skip pruning when `node_modules` didn't exist before the install. On an update, prune what the previous `node_modules/.craft/state` has and the new graph lacks, instead of reading every link in the virtual store.
- Expected with 5.13's scripts scan: `large` about 1.3 s, ahead of pnpm. The same applies without a lockfile, where linking takes 1.8 s.
- Below the clone floor only a shared global virtual store would go (projects symlink entries in a store-wide `links` directory, as pnpm's opt-in global virtual store does). It breaks packages that rely on undeclared dependencies, so leave it for later, opt-in.

### 5.17 Trust fresh-enough metadata (minimum release age) · P2

Once a cached packument is older than its `max-age` (300 s on the npm registry), craft revalidates it before resolving, one dependency level at a time and 16 at a time, and any packument that changed comes back in full: 22–24 s for `large`. pnpm installed the same project without a lockfile in 2.0 s from a cache about 21 hours old, with no network connections at all (sampled throughout the install, and it also succeeded against an unreachable registry). Its cache records `minimumReleaseAge: 1440`. When versions younger than that age aren't eligible, a packument cached more recently than that age already lists every version resolution may choose.

- Add `minimum-release-age` (pnpm's `.npmrc` key, in minutes): versions published more recently are skipped while resolving, which also protects against freshly published malicious versions. Use a cached packument without asking while it is younger than that age. Whether craft defaults to pnpm's one day is a behaviour change to decide.
- Without it, still revalidate every name the cache holds in parallel, ahead of the dependency walk instead of level by level. 5.9's abbreviated metadata keeps the changed responses small.

---

## Phase 6 — CLI & UX

### 6.1 Progress output and summary · P2 — ✔ done

> Done: `progress::Reporter` (carried by `Context`) draws one live status line on an interactive terminal — `Resolving dependencies… 143 packages`, `Linking 218 packages`, `Running install scripts` — and falls back to plain output when stderr isn't a terminal, in CI or with `--verbose`; `--silent` prints nothing but warnings and errors. Warnings go through `Reporter::warn`, which pauses the status line instead of garbling it. After `install` / `add` / `ci` the project's own dependency changes are listed against the previous lockfile (`+ react 19.2.0`, `+ ms 2.1.3 (dev)`, `- lodash 4.17.21`, `~ react 19.2.0 → 18.3.1`), then `N packages installed in X.Xs`; `remove` lists what it removed. Not done: deprecation warnings (needs `deprecated` from the packument).

- `craft install` prints nothing without `-v`. Wire `crates/progress` (indicatif): resolving / downloading / linking phases with counts.
- Final summary: `+ react 19.2.0`, `- lodash 4.17.21`, `N packages installed in X.Xs`, peer warnings, skipped install scripts, deprecations.
- Respect non-TTY / `CI` (plain output), `--silent`, `NO_COLOR`.

### 6.2 Error reporting · P2 — ✔ done

> Done: no panic is reachable outside tests — clippy denies `unwrap`/`expect` workspace-wide, there is no `panic!`/`unreachable!`/`todo!`, and a `clippy::indexing_slicing` + `string_slice` audit found only indices on ASCII boundaries or bounded by `zip`. Errors print as `error: <message>`, one `  caused by: …` line per cause and, for a fixable cause, `  hint: …` (permission denied → check write access; disk full → `craft cache clean --force`). Every `fsutil` operation names what it did to which path (`could not create directory …/node_modules: Permission denied`) while keeping the IO error kind; tarball extraction, download persistence, package.json reads/writes, store directories and pruning carry their paths too. A registry that can't be reached collapses reqwest's nested errors into `could not reach <host> (<reason>); check your network connection and proxy settings, or use --offline`.

- No panics reachable from user input (1.1, 0.1, `linker_pipe.rs:211` `expect`).
- Print `error: <message>` + cause chain + hint, exit 1 (scripts: exit with their code, 1.12). Install `miette` or format anyhow chains manually.
- Context on every IO error (which path, which package).

### 6.3 Missing commands and flags · P2 — ✔ done

> Done (first half): `craft --version` / `-V` prints the release tag (the release workflow passes it as `CRAFT_RELEASE_VERSION`; local builds fall back to the crate version), every command and flag has help text, and npm's aliases work: `i` (with package names `install` behaves like `add`, including `-D`/`-E`), `rm`/`uninstall`/`un`, `t`. `craft run` without a name lists the scripts. `craft init` writes a minimal package.json named after the directory (normalized to a valid npm name) and refuses to overwrite one; a missing package.json now suggests it.
>
> Done (second half): `craft list`/`ls [--depth N]` and `craft why <pkg>` answer from craft.lock alone (tree with peer/optional labels and cycle marks; every chain grouped by version). `craft trust <pkg…>` adds to `trustedDependencies` and installs so the scripts run. `craft add -O`/`--save-optional` (the project's own `optionalDependencies` now install, failures skipped with a warning, locked in their own importer section) and `-P`/`--save-peer` (saved to `peerDependencies` and `devDependencies`, like pnpm). `craft install --production`/`--prod` leaves dev dependencies out without rewriting craft.lock. `craft outdated` shows Current / Wanted / Latest for direct registry dependencies; `craft update [pkg…]` re-resolves within specifiers (all of them ignore the lockfile, named ones only themselves) and `--latest`/`-L` moves to the latest tag rewriting the specifier in its style (exact, `~`, `^`, `npm:` alias). `craft exec <bin>` runs a dependency bin only; `craft dlx`/`x <pkg>[@spec]` installs into a cached `~/.craft/dlx/<hash>` (tags re-resolved each run) and runs the package's only or same-named bin, or `-p <pkg> <bin>`, passing the exit code through. Verified: `craft dlx create-vite@6` scaffolds a project and, reading craft's user agent, prints `craft install` / `craft run dev`.
>
> `--verbose` became `--debug` (long only), so `-v` / `--version` print the version (`-V` still works).

- `craft --version` currently errors (`unexpected argument '--version'`) — add `#[command(version)]`, and keep `Cargo.toml` version in sync with release tags (tags are `0.0.8`, crate is `0.1.0`).
- Aliases: `i`/`install <pkg…>` (→ add), `rm`/`uninstall`/`un`, `x`/`exec`.
- New: `init` (`-y`), `update [pkg]` (`--latest`), `outdated`, `why <pkg>`, `list`/`ls`, `dlx`/`x` (temporary install + run).
- Flags: `add -E/--exact`, `-O/--save-optional`, `-P/--save-peer`; `install --production`/`--omit=dev`, `--ignore-scripts`, `--frozen-lockfile`, `--offline`.
- `craft run` with no script: list scripts.

### 6.4 Windows support · P2 — ✔ done

> Done: bins are `cmd-shim` style `.cmd` / `.ps1` / sh shims (`fsutil::shims`) that run the bin's shebang program, preferring one beside the shim, and pass the exit code through without cmd's "Terminate batch job" prompt; `fsutil::bins` (`link_bin`, `bin_target`, `remove_bin`, `bin_names`, `bin_command`) hides symlink vs shim from the linker, prune, `craft remove`, bin lookup and dlx. Directory links are junctions (absolute targets, no privileges needed); `read_link_resolved` strips the `\\?\` prefix so prune can compare paths, and replacing a link removes the old entry and confirms the move, since Windows can't rename onto a directory. `file:` copies turn linked directories into junctions and fall back to copying linked files. Scripts run as `cmd /d /s /c "<script>"` passed verbatim (`raw_arg`); appended arguments use npm's quoting (MSVC rules plus `^` before cmd metacharacters, twice when the script's first word resolves to a `.cmd`/`.bat` on `PATH`). The lockfile writes local paths and keys with `/`, so it is identical on every platform. e2e fixtures no longer depend on `sh` (`node -e` / `print.js` helpers), and link assertions go through `fsutil`. CI runs `windows-latest`; `docker/windows/test.sh` cross-compiles every test executable with mingw and runs it under Wine (staging, for junctions) with Windows Node.js and Git.
>
> Verified under Wine: all unit tests, and 177 of 178 e2e tests (each failure of a parallel run passes alone); the eight-process concurrent-install test, and installs of real packages from the npm registry, are killed by a Rosetta bug (`xsave_header.xfeatures == kSupportedXFeatureBits` in `rt_sigreturn`) when Docker on Apple Silicon emulates x86 with Rosetta instead of QEMU — no Windows exception is raised before it. Real Windows is verified by CI's `windows-latest` job.

- Bins: generate `.cmd` + `.ps1` + sh shims (like `cmd-shim`) instead of `symlink_file` (`linker_pipe.rs:259`).
- Directory links: junctions (no admin/Developer Mode needed) instead of `symlink_dir` (`linker_pipe.rs:90`, `191`).
- `PATH` separator and `Path`/`PATH` casing in 1.12; `cmd /C` quoting.
- Add `windows-latest` to the CI matrix (deferred from 0.4) once the e2e suite passes there.

### 6.5 Release & docs accuracy · P2 — ✔ done

> Done: the release workflow builds `aarch64-unknown-linux-gnu` (native arm64 runner) and static `x86_64`/`aarch64-unknown-linux-musl`, and publishes a `<asset>.sha256` beside every binary. reqwest now uses rustls with bundled Mozilla root certificates instead of OpenSSL, so musl builds need no system libraries or CA bundle (npm and JSR installs re-verified over HTTPS); like npm, private CAs are trusted through `cafile`. OS roots were tried first and dropped: loading the macOS keychain made parallel runs 6× slower and would fail in containers without `ca-certificates`. `install.sh` picks the musl build on musl systems, verifies the checksum (refuses a mismatch, warns when a release has none), installs to `/usr/local/bin` when writable or `~/.local/bin` without sudo, and takes `CRAFT_INSTALL_DIR`, `CRAFT_VERSION` and `CRAFT_DOWNLOAD_URL`; tested against a local release server with a valid, a tampered and a missing checksum. The README drops the `package-lock.json` and "`craft init` coming soon" claims and documents every command, the store and virtual-store layout, `craft.lock`, `trustedDependencies`, `.npmrc` keys and the `CRAFT_*` variables.
>
> Not verified locally: the new CI matrix entries themselves; they run on the next tag.
>
> Docs site: `docs/` is a VitePress site (guide, CLI reference, contributing) whose dependencies craft installs itself; `make docs` previews it and `.github/workflows/docs.yml` builds it with the craft of the same commit and deploys to GitHub Pages from `main`. Dogfooding it found that a published manifest with a non-string script (`speakingurl`'s `"blanket": { … }`) failed to parse; like npm, non-string `scripts` / `dependencies` / `devDependencies` entries are now ignored.

- Release workflow: publish `sha256` checksums; `install.sh` verifies them; support `CRAFT_INSTALL_DIR` / `~/.local/bin` without sudo.
- Linux: add `aarch64-unknown-linux-gnu` and a `musl` static build.
- README: remove claims that aren't true yet (`package-lock.json` support, `craft init`), document cache location, lockfile, trusted dependencies.

---

## Phase 7 — Later

- Workspaces (`workspaces` field, `workspace:` protocol, `--filter`, `importers` in lockfile).
- Import `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` for migration.
- `overrides` / `resolutions`.
- Global installs (`craft add -g`), `craft link` for local development.
- `craft audit` (npm advisory API), `craft publish` / `pack`.
- Hoisted linker mode for tools incompatible with symlinks.
