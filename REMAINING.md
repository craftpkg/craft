# Craft — Remaining Work

Status of the Rust package manager as of **2026-10-24**, and the ordered list of work needed to finalize it.
Work through tasks top to bottom: each phase unblocks the next. Tick a task's box when it is merged.

**Known-bug tests:** every open bug that has a scenario is an `#[ignore = "fixed by <task>: …"]` end-to-end test in
`apps/craft/tests/e2e/`. `cargo test -p craft --test e2e -- --ignored` lists what's still broken; when a task
lands, delete the `#[ignore]` on its tests.

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
| 1.4 | Tarball root-directory stripping | P0 | [ ] |
| 1.5 | HTTP status checks + friendly registry errors | P0 | [ ] |
| 1.6 | Atomic download and extraction | P0 | [ ] |
| 1.7 | Integrity verification (`dist.integrity` / `shasum`) | P0 | [ ] |
| 1.8 | Symlink-safe filesystem helpers (dangling links) | P0 | [ ] |
| 1.9 | `craft add` fixes | P1 | [ ] |
| 1.10 | Preserve `package.json` formatting and key order | P1 | [ ] |
| 1.11 | `craft remove` fixes | P1 | [ ] |
| 1.12 | `craft run` fixes (PATH, exit code, quoting, pre/post, env) | P1 | [ ] |
| 1.13 | Git / tarball-URL / `file:` / `link:` dependencies | P1 | [ ] |
| 1.14 | Bin linking edge cases | P1 | [ ] |
| **Phase 2 — Store & layout redesign** | | | |
| 2.1 | Global content-addressed store (read-only) | P0 | [ ] |
| 2.2 | Per-project virtual store `node_modules/.craft` | P0 | [ ] |
| 2.3 | Peer dependency resolution | P0 | [ ] |
| 2.4 | Bins inside the virtual store | P1 | [ ] |
| 2.5 | Prune extraneous packages | P1 | [ ] |
| **Phase 3 — Lockfile** | | | |
| 3.1 | Lockfile format (text, deterministic) | P0 | [ ] |
| 3.2 | Write lockfile after install/add/remove | P0 | [ ] |
| 3.3 | Install from lockfile (skip resolution) | P0 | [ ] |
| 3.4 | `--frozen-lockfile` / `craft ci` | P1 | [ ] |
| **Phase 4 — Platform packages & lifecycle scripts** | | | |
| 4.1 | `optionalDependencies` | P0 | [ ] |
| 4.2 | `os` / `cpu` / `libc` filtering | P0 | [ ] |
| 4.3 | Lifecycle scripts with an allowlist | P0 | [ ] |
| **Phase 5 — Network & performance** | | | |
| 5.1 | Abbreviated packuments | P2 | [ ] |
| 5.2 | Memoize packument fetches per package name | P2 | [ ] |
| 5.3 | Global concurrency limits | P2 | [ ] |
| 5.4 | Timeouts, retries, User-Agent | P2 | [ ] |
| 5.5 | Streaming downloads (hash while writing) | P2 | [ ] |
| 5.6 | Metadata disk cache (ETag / 304) | P2 | [ ] |
| 5.7 | Registry configuration (`.npmrc`, scopes, auth) | P1 | [ ] |
| **Phase 6 — CLI & UX** | | | |
| 6.1 | Progress output and install summary | P2 | [ ] |
| 6.2 | Error reporting (no panics, contextual messages) | P2 | [ ] |
| 6.3 | Missing commands and flags | P2 | [ ] |
| 6.4 | Windows support (shims, junctions) | P2 | [ ] |
| 6.5 | Release & docs accuracy | P2 | [ ] |
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

### 1.4 Tarball root-directory stripping · P0 ✅

**Problem** — the linker assumes the archive root is `package/` (`linker_pipe.rs:33`, `163`). `@types/*` tarballs use `node/`, some older packages use other names:

```
node_modules/@types/node -> ~/.craft/packages/@types/node-22.20.5/package   (does not exist)
```

**Fix** — in `tarball::gzip::unzip`, strip the first path component of every entry (iterate `archive.entries()`, rewrite path, `entry.unpack_in`-style safety checks for `..` and absolute paths). Extract to `<store>/<name>@<version>/` directly so the linker never guesses a subdirectory.
Also normalize permissions (some tarballs have `0o000` dirs) — files `0o644`, dirs and bins `0o755`.

**Done when** the `@types`-style fixture links to a dir containing `index.d.ts`.

### 1.5 HTTP status checks + friendly registry errors · P0 ✅

**Problem**
- `Network::fetch` and `Network::download` (`crates/network/src/network.rs:21-37`) never check `response.status()`. A 404/500 body is saved as a `.tgz`, or parsed as a packument.
- Unknown package error today:

```
Caused by: missing field `name` at line 1 column 21
```

**Fix**
- `response.error_for_status()`; map 404 → `PipelineError::PackageNotFound { name }`, 401/403 → auth error hint, 429/5xx → retryable (5.4).
- Error messages name the package and the spec that requested it, plus the parent chain if transitive (`express > body-parser > foo`).

### 1.6 Atomic download and extraction · P0 📖

**Problem**
- `Resolver::download` treats `file_path.exists()` as "cached" (`crates/resolver/src/resolver.rs:67`). `Network::download` creates the final file then writes into it — a killed process leaves a truncated `.tgz` that is reused forever.
- Same for extraction: `if !unzip_dir.exists() { unzip(..) }` (`install_pipe.rs:88`) — half-extracted dirs are reused forever.
- Locks are in-process only (`download_locks`, `unzip_locks`), so two `craft` processes race.

**Fix**
- Download to `<path>.tmp-<pid>-<rand>`, verify integrity (1.7), then `rename` into place (atomic on the same filesystem).
- Extract into `<dir>.tmp-<rand>`, then `rename`; if the rename fails because the target now exists, another process won — delete the temp dir and continue.
- Optional: a marker file (`.craft-complete`) written last, and treat dirs without it as absent.

**Done when** killing `craft` mid-install and re-running produces a working install; two concurrent installs never error.

### 1.7 Integrity verification · P0 📖

**Problem** — `PackageDist` only has `tarball` (`crates/package/src/npm.rs:58-60`); `dist.integrity` / `dist.shasum` are ignored. Nothing stops a corrupted or tampered tarball from being installed.

**Fix**
- Add `integrity: Option<String>` and `shasum: Option<String>` to `PackageDist`.
- Verify SRI (`sha512-<base64>`, also accept `sha256`/`sha1`) using `ssri` or `sha2` + `base64`; fall back to `shasum` (sha1 hex). Hash while streaming (5.5).
- Mismatch → delete the temp file, error `IntegrityMismatch { name, version, expected, actual }`.
- Store integrity on `ResolvedArtifact` (needed by the lockfile).

**Done when** the corrupt-tarball fixture fails with an integrity error and leaves nothing in the cache.

### 1.8 Symlink-safe filesystem helpers · P0 ✅

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

### 1.9 `craft add` fixes · P1 ✅

> Partly done in 1.1: `craft add` saves the version each requested package resolved to (from `Installation.roots`), so `craft add react@18` saves `^18.3.1` and aliases save `npm:<target>@^<version>`; the arbitrary-`find` bug is gone.

**Problems** (`crates/actor/src/add_package_actor.rs`)
- ~~`craft add react@18` installs but does not update `package.json`~~ (fixed in 1.1).
- ~~When several versions of a name are resolved, `find` picks an arbitrary one~~ (fixed in 1.1).
- `package.json` is read **after** installing (line 37); with no `package.json`, all work is done then it fails.
- Only the added packages are installed/linked — existing deps aren't verified, and once the lockfile exists it must be updated too.

**Fix**
- Read `package.json` first; error early if missing (suggest `craft init`, 6.3).
- Save rules (match npm): tag or range spec → `^<resolved>`; exact version spec → `^<resolved>` unless `-E/--exact`; alias → `npm:<target>@^<resolved>`; git/file → the original spec.
- If the package already exists in the other section (`dependencies` vs `devDependencies`), move it.
- After Phase 3: run a full install (all deps) so the lockfile stays consistent.

### 1.10 Preserve `package.json` formatting · P1 ✅

**Problem** — writes go through the typed `PackageJson` struct with a `HashMap` `other` field (`crates/package/src/npm.rs:28-29`) and `serde_json` without `preserve_order`. Result: top-level keys reorder between runs (`private`/`type` swapped in testing), indentation is forced to 2 spaces, trailing newline is dropped → noisy git diffs.

**Fix**
- Enable `serde_json`'s `preserve_order` feature.
- Edit `package.json` as a `serde_json::Value` (only touching `dependencies`/`devDependencies`/…), not by re-serializing the typed struct.
- Detect indentation (tab / N spaces) and the trailing newline from the original file and reproduce them.
- New dependency maps are sorted alphabetically (npm behaviour); existing ones keep order with the new key inserted sorted.
- Keep the typed `PackageJson` for reading registry manifests only.

**Done when** `add` then `remove` of the same package produces a byte-identical `package.json`.

### 1.11 `craft remove` fixes · P1 ✅

**Problems** (`crates/actor/src/remove_package_actor.rs`)
- Dangling symlinks not removed (1.8).
- The package's bins in `node_modules/.bin` are left behind.
- `package.json` is rewritten even when nothing changed; a package not in `package.json` gives no user-facing message.
- Orphaned transitive deps are not pruned (fixed properly by 2.5).

**Fix** — use 1.8 helpers; remove bins whose link target is inside the removed package; only write when changed (1.10); warn `<pkg> is not a dependency`; update lockfile (Phase 3).

### 1.12 `craft run` fixes · P1 ✅

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

### 1.13 Git / tarball / `file:` / `link:` dependencies · P1 📖

**Problem** — `GitResolver` (`crates/resolver/src/git.rs`) returns the git URL as `download_url`, which reqwest then tries to `GET` — git deps can never install. Version is the literal `"git"`.

**Fix** (each behind the `DependencySpec` variants from 1.1)
- **Tarball URL**: download as-is, read `package.json` from the tarball for name/version/deps. NodeId `name@<url-hash>`.
- **GitHub/GitLab/Bitbucket shorthands**: resolve committish via the host API or `git ls-remote`, download `codeload` tarball (no git needed). Generic git: `git clone --depth 1` into a temp dir, then `git archive` / pack. Run `prepare` if present (Phase 4.3).
- **`file:` dir**: copy (or pack) into the store; **`file:*.tgz`**: treat as tarball. **`link:`**: plain symlink, no store, deps not installed.
- Lockfile records the resolved commit / integrity.

Can be split: tarball URL + `file:` + `link:` first (simple), git hosts second.

### 1.14 Bin linking edge cases · P1 📖

**Problems** (`linker_pipe.rs:203-266`)
- String `bin` on a scoped package uses `@scope/name` as the bin name → tries to create `.bin/@scope/name` (ENOENT). Correct name is the part after `/`.
- ~~Bin source path for transitive deps uses `artifact.name`, not the alias link name~~ (fixed in 1.2: bins resolve through `edge.name`).
- `directories.bin` not supported.
- Bin names/paths from manifests are not sanitized (a bin named `../../x` writes outside `.bin`).
- Two packages providing the same bin race in parallel linking (`buffer_unordered(10)` over roots, `linker_pipe.rs:304-311`) → nondeterministic winner / `EEXIST`.

**Fix** — normalize bins in one function (`HashMap<SafeBinName, RelativePath>`), reject names containing `/` or `..`, link bins sequentially (or with a deterministic rule: direct dependency wins over transitive), use relative symlink targets, `chmod 0o755` the target.

---

## Phase 2 — Store & layout redesign

**Why** — today each package's dependency symlinks are written **into the shared global cache**
(`~/.craft/packages/<pkg>/package/node_modules/<dep>`, `linker_pipe.rs:41-90`). Consequences:

- ✅ Concurrent installs in different projects race: 2 of 6 parallel installs failed with `File exists (os error 17)`.
- One project's resolution silently rewrites another project's dependency tree.
- ✅ Peer deps can't work: Node resolves symlinks to their real path in `~/.craft`, so a package can never see the project's `react` (`@tanstack/react-query` → `Cannot find module 'react'`).

Target layout (pnpm-style, proven to work with Node's realpath resolution):

```
~/.craft/store/v1/<name>@<version>/            # immutable extracted package (2.1)

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

### 2.1 Global content-addressed store · P0

- Extracted packages live in `~/.craft/store/v1/` keyed by integrity (or `name@version` + integrity check), written atomically (1.6), never modified afterwards (optionally `chmod -w`).
- Tarballs can be deleted after extraction (or kept under `~/.craft/cache/tarballs` for `cache clean` granularity).
- `craft cache clean` targets the store; add `craft cache dir` / `craft cache prune` (remove entries unused by any lockfile — later).

### 2.2 Per-project virtual store · P0

- Materialize each graph node into `node_modules/.craft/<escaped-id>/node_modules/<name>`:
  macOS `clonefile` (APFS), Linux `ioctl FICLONE` → hardlink → copy fallback; Windows hardlink → copy.
  (`reflink-copy` crate covers clone + fallback.)
- Dependencies of a node are relative symlinks siblings inside the same `node_modules/` (Windows: junctions, 6.4).
- Root edges link `node_modules/<link_name>` → virtual store entry.
- All writes are project-local → no cross-project races, no global locks needed beyond the store.
- `bundleDependencies`: already inside the package's tarball — don't overwrite them.
- Optional later: `public-hoist-pattern` (hoist `*eslint*`, `*prettier*`) and `node-linker=hoisted` for tools that break under symlinks.

**Done when** the concurrent-install scenario passes 20/20 and deleting `~/.craft` doesn't break an already-installed project if materialization used copies/hardlinks.

### 2.3 Peer dependency resolution · P0 ✅

**Problem** — `peerDependencies` are ignored entirely (`install_pipe.rs:95` only reads `dependencies`).

**Fix**
- During resolution, a peer edge resolves to whatever the **parent** provides (the parent's own dependency or the parent's peer, recursively); if nobody provides it, auto-install the peer (npm ≥ 7 behaviour) at the highest satisfying version.
- Respect `peerDependenciesMeta[x].optional` (don't auto-install, don't warn).
- Packages whose peers resolve differently in different parts of the tree get separate virtual-store entries (`name@ver_peer@ver` suffix).
- Warn (not fail) on unmet peer ranges; `--strict-peer-dependencies` to fail.

**Done when** `require('@tanstack/react-query')` works with `react` as a root dependency and the same project builds with two different `react` peers in two subtrees.

### 2.4 Bins inside the virtual store · P1

- Each virtual-store `node_modules/` gets a `.bin` for its node's dependencies, so lifecycle scripts (4.3) and nested tools find their bins.
- Root `node_modules/.bin` only for direct dependencies (npm also exposes transitive bins at root when hoisted — decide and document).

### 2.5 Prune extraneous packages · P1

- After linking, delete root `node_modules/*` entries and `.craft/*` entries not in the current graph (respecting scoped dirs and `.bin`).
- Makes `craft remove` and editing `package.json` + `craft install` correct.

---

## Phase 3 — Lockfile

`crates/lockfile` exists (JSON + bincode, `Lockfile { version, packages }`), but nothing depends on it.
README says "`package-lock.json` support (coming soon)" — decide whether that means *reading* npm's lockfile (Phase 7) or Craft's own.

### 3.1 Lockfile format · P0

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

### 3.2 Write lockfile · P0

- Serialize the `ResolutionGraph` (1.2) after every successful `install`/`add`/`remove`; write atomically; only write when contents changed.

### 3.3 Install from lockfile · P0

- If every importer specifier in `package.json` matches the lockfile, skip all registry metadata requests: download/verify by `resolved` + `integrity` only.
- If some specifiers changed, keep locked versions for unchanged deps and re-resolve only the changed subtrees.
- Fast path: if `node_modules/.craft/lock` equals `craft.lock`, print "Already up to date" and exit (measured warm install today: ~3.3 s, all from re-fetching packuments).

### 3.4 `--frozen-lockfile` / `craft ci` · P1

- `--frozen-lockfile`: fail if the lockfile is missing or out of date with `package.json`. Default on when `CI=true`.
- `craft ci`: remove `node_modules`, then frozen install.

---

## Phase 4 — Platform packages & lifecycle scripts

✅ Today `esbuild` installs but `require('esbuild')` fails: its platform binary ships as an optional dependency (`@esbuild/darwin-arm64`) and its `postinstall` never runs. Same class: `@rollup/rollup-*`, `@swc/core-*`, `lightningcss-*`, `sharp`, `@next/swc-*`, `bcrypt`, `sqlite3`.

### 4.1 `optionalDependencies` · P0

- Add `optional_dependencies` (and `peer_dependencies`, `peer_dependencies_meta`, `os`, `cpu`, `libc`, `has_install_script`, `deprecated`, `engines`) to the manifest struct (`crates/package/src/npm.rs`).
- Resolve optional deps; any failure (resolve, download, platform mismatch, install script) skips that subtree with a warning instead of failing the install.

### 4.2 `os` / `cpu` / `libc` filtering · P0

- Skip packages whose `os`/`cpu` (supports `!` negation) don't match the host (`std::env::consts::{OS, ARCH}` mapped to Node names: `darwin`, `linux`, `win32`; `x64`, `arm64`, `ia32`).
- `libc` (`glibc`/`musl`) detection on Linux.
- Lockfile keeps all platform variants so it's portable across machines; filtering happens at link time.

### 4.3 Lifecycle scripts with an allowlist · P0

- Dependency scripts: `preinstall`, `install`, `postinstall` (and implicit `node-gyp rebuild` when `binding.gyp` exists and there's no `install`/`preinstall`), run after linking in **dependency order** (deps before dependents), with the virtual-store `.bin` on `PATH` and `npm_*` env (reuse 1.12).
- Root project: `preinstall`, `install`, `postinstall`, `prepare`.
- Security: install scripts of dependencies run **only** for packages listed in `package.json` (`"craft": { "trustedDependencies": [...] }`, like Bun/pnpm `onlyBuiltDependencies`); print which packages were skipped and how to allow them. `--ignore-scripts` disables everything.
- Run scripts in parallel where the graph allows, with output prefixed by package name; failure of a non-optional package fails the install.

---

## Phase 5 — Network & performance

Measured: cold install of this repo's `package.json` ≈ 4.8 s, 149 MB RSS; warm ≈ 3.3 s.

### 5.1 Abbreviated packuments · P2

- Send `Accept: application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8` (`network.rs:22`). `typescript` packument: 15.8 MB full vs 8.7 MB abbreviated. Abbreviated manifests still include `bin`, `dist.integrity`, `optionalDependencies`, `os`/`cpu`, `hasInstallScript`.
- `NpmPackage` needs `#[serde(default)]` on optional fields; make manifest fields tolerant (old packages have `keywords` as a string, `dependencies` as an array, etc.) — use `serde_json::Value` or lenient deserializers for fields that aren't needed, so one bad historical version doesn't fail the whole packument.

### 5.2 Memoize packument fetches per name · P2

- Today every `(name, range)` pair fetches the full packument again (`npm.rs:23`). Cache `Arc<Packument>` per name in a `DashMap<String, OnceCell<…>>` (or `futures::Shared`) so concurrent requests for the same name share one fetch.

### 5.3 Global concurrency limits · P2

- `buffer_unordered(10)` nests at every recursion level (`install_pipe.rs:106`) → effectively unbounded parallel requests. Replace with a `tokio::sync::Semaphore` for metadata (e.g. 64) and one for tarballs (e.g. 16), configurable via `network-concurrency`.
- Also cap blocking extraction tasks.

### 5.4 Timeouts, retries, User-Agent · P2

- `reqwest::Client::builder()` with connect + read timeouts, gzip/brotli, HTTP/2, `User-Agent: craft/<version> <os> <arch>`.
- Retry idempotent requests on connect errors / 429 / 5xx with exponential backoff + jitter (3 attempts), honouring `Retry-After`.
- Upgrade `reqwest` 0.11 → 0.12; consider `rustls-tls` to avoid OpenSSL in release builds.

### 5.5 Streaming downloads · P2

- `response.bytes()` buffers whole tarballs in memory (`network.rs:30`). Stream chunks to the temp file while updating the integrity hasher (1.7); or stream straight into `GzDecoder` + `tar` and extract without writing the `.tgz`.

### 5.6 Metadata disk cache · P2

- Cache packuments under `~/.craft/cache/metadata/` with `ETag`/`Last-Modified`; revalidate with `If-None-Match` (304). `--prefer-offline` / `--offline` flags.

### 5.7 Registry configuration · P1

- Registry is hardcoded (`npm.rs:23`). Read `.npmrc` (project, user `~/.npmrc`, env `npm_config_*`): `registry`, `@scope:registry`, `//host/:_authToken`, `_auth`, `always-auth`, `strict-ssl`, `cafile`, proxies (`https-proxy`, `HTTPS_PROXY`).
- URL-encode scoped names: `@scope%2fname`.
- Never log tokens (verbose mode prints URLs).

---

## Phase 6 — CLI & UX

### 6.1 Progress output and summary · P2

- `craft install` prints nothing without `-v`. Wire `crates/progress` (indicatif): resolving / downloading / linking phases with counts.
- Final summary: `+ react 19.2.0`, `- lodash 4.17.21`, `N packages installed in X.Xs`, peer warnings, skipped install scripts, deprecations.
- Respect non-TTY / `CI` (plain output), `--silent`, `NO_COLOR`.

### 6.2 Error reporting · P2

- No panics reachable from user input (1.1, 0.1, `linker_pipe.rs:211` `expect`).
- Print `error: <message>` + cause chain + hint, exit 1 (scripts: exit with their code, 1.12). Install `miette` or format anyhow chains manually.
- Context on every IO error (which path, which package).

### 6.3 Missing commands and flags · P2

- `craft --version` currently errors (`unexpected argument '--version'`) — add `#[command(version)]`, and keep `Cargo.toml` version in sync with release tags (tags are `0.0.8`, crate is `0.1.0`).
- Aliases: `i`/`install <pkg…>` (→ add), `rm`/`uninstall`/`un`, `x`/`exec`.
- New: `init` (`-y`), `update [pkg]` (`--latest`), `outdated`, `why <pkg>`, `list`/`ls`, `dlx`/`x` (temporary install + run).
- Flags: `add -E/--exact`, `-O/--save-optional`, `-P/--save-peer`; `install --production`/`--omit=dev`, `--ignore-scripts`, `--frozen-lockfile`, `--offline`.
- `craft run` with no script: list scripts.

### 6.4 Windows support · P2

- Bins: generate `.cmd` + `.ps1` + sh shims (like `cmd-shim`) instead of `symlink_file` (`linker_pipe.rs:259`).
- Directory links: junctions (no admin/Developer Mode needed) instead of `symlink_dir` (`linker_pipe.rs:90`, `191`).
- `PATH` separator and `Path`/`PATH` casing in 1.12; `cmd /C` quoting.
- Add `windows-latest` to the CI matrix (deferred from 0.4) once the e2e suite passes there.

### 6.5 Release & docs accuracy · P2

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
