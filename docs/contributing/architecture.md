# Architecture

craft is a Cargo workspace. The binary in `apps/craft` is thin: it parses the command line, builds a `Context`, and hands an action to the manager.

```text
apps/craft          main: CLI → Context → CraftManager, error printing
crates/
├── cli             clap definitions; turns arguments into an Action
├── manager         dispatches each Action to its actor
├── actor           one actor per command: install, add, remove, update, outdated,
│                   list/why, run/exec, dlx, trust, init, ci, cache
├── pipeline        the install pipeline: resolve → peers → link → prune → scripts → lockfile
├── resolver        registry and git resolution, the dependency graph, lockfile preferences
├── network         HTTP client: retries, concurrency limits, metadata cache, integrity
├── tarball         safe, atomic tarball extraction
├── package         package.json types, dependency specifiers, platform constraints
├── lockfile        craft.lock types
├── process         spawning scripts and bins, their environment and argument quoting
├── fsutil          links, junctions, bin shims, store imports, atomic writes
├── progress        the status line and summaries
├── contract        shared types: Context, errors, .npmrc configuration
├── debug           --debug logging
└── testkit         the mock registry and package fixtures used by the tests
```

## An install, step by step

1. **`InstallActor`** reads `package.json` and `craft.lock`, and stops early with `Already up to date` when the installed state matches.
2. **`InstallPipe`** resolves every dependency into a `ResolutionGraph` of nodes (`name@version`) and edges. Registry requests share one packument fetch per name; locked versions are preferred; local and git sources are materialized while resolving, since their metadata comes from their own `package.json`.
3. **The peer pass** rewrites the graph so each package instance knows its resolved peers (`ui@1.0.0_react@19.2.0`).
4. **`fetch`** downloads and extracts missing registry packages into the global store.
5. **`LinkerPipe`** imports store entries into `node_modules/.craft`, links dependencies, hoists, links bins, and prunes.
6. **`LifecyclePipe`** runs install scripts of trusted packages.
7. **The lockfile** is serialized from the graph and written if it changed.

## Conventions

- Errors are `thiserror` types in `contract::errors`, with context added at IO boundaries (`fsutil` names the path for every failure), and printed by `main` as `error:`, `caused by:` and `hint:` lines.
- Filesystem changes go through `fsutil`, which handles dangling links, concurrent writers and the differences between Unix and Windows.
- Platform-specific code lives behind `cfg(unix)` / `cfg(windows)` in small functions, with the shared logic compiled and tested everywhere.
