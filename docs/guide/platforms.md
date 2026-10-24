# Platform packages

Native tools often publish one package per platform and list them all as `optionalDependencies`, each restricted with `os`, `cpu` and sometimes `libc`:

```json
{
  "name": "@esbuild/darwin-arm64",
  "os": ["darwin"],
  "cpu": ["arm64"]
}
```

craft installs only the packages that match the machine it runs on, the same way npm does.

## Matching rules

- `os` and `cpu` use Node's names: `darwin`, `linux`, `win32`, …; `x64`, `arm64`, ….
- A list allows those values; entries starting with `!` exclude them (`"os": ["!win32"]`).
- `libc` (`glibc` or `musl`) applies only on Linux. craft detects musl, as on Alpine, from `/lib/ld-musl-*`.

Packages for other platforms are resolved and recorded in `craft.lock`, but never downloaded, linked or given bins, so a lockfile written on a Mac installs the right binaries on Linux and Windows too.

If a dependency that isn't optional doesn't support the platform, the install fails and says what required it:

```text
error: mac-only@1.0.0 does not support linux-x64-glibc, but the project requires it
```

## Installing for another platform

`CRAFT_OS`, `CRAFT_CPU` and `CRAFT_LIBC` override what craft detects. That's useful when you build a Linux container image from a `node_modules` prepared on a Mac:

```sh
CRAFT_OS=linux CRAFT_CPU=x64 CRAFT_LIBC=glibc craft install
```

## Optional dependencies

A failure to install an optional dependency (missing from the registry, failed download, failed build) is a warning, not an error:

```text
warning: skipping optional dependency fsevents of chokidar: …
```

Your own `optionalDependencies` behave the same way. Add one with `craft add -O <package>`.
