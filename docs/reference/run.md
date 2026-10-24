# craft run

Run a `package.json` script, or list them when no name is given.

```text
craft run [OPTIONS] [SCRIPT] [ARGS]...
```

Takes the [global options](./cli#global-options).

## Scripts and bins

`craft run <name>` runs, in order of preference:

1. the script `<name>` in `package.json`;
2. otherwise, the bin `<name>` from the nearest `node_modules/.bin`, searching the current directory and its parents.

Otherwise it fails, listing the scripts there are:

```text
error: script "deploy" not found; available scripts: build, test
```

`craft <name>`, with any name that isn't a craft command, does the same: `craft build`, `craft tsc --noEmit`. To run only a bin, never a script, use [`craft exec`](./exec).

## Pre and post scripts

`craft run build` runs `prebuild`, `build` and `postbuild`, the ones that exist, stopping at the first that fails. Only `build` receives the arguments.

## Arguments

Everything after the script name goes to the script, quoted so it reaches it as written, with no shell expansion:

```sh
craft run test --watch
craft run greet "Ada Lovelace"      # the script gets one argument: Ada Lovelace
craft run show 'a; echo hacked'      # one argument; the echo never runs
```

## Environment

Scripts run in a shell (`sh -c` on macOS and Linux, `cmd /d /s /c` on [Windows](/guide/windows#scripts-run-in-cmd)) in the project directory, with:

| Variable | Value |
| --- | --- |
| `PATH` | `node_modules/.bin` of the project and every parent directory, then your `PATH` |
| `npm_lifecycle_event` | The script's name, e.g. `prebuild` |
| `npm_lifecycle_script` | The script's command |
| `npm_package_name`, `npm_package_version` | From `package.json` |
| `npm_package_json` | The path of `package.json` |
| `npm_config_user_agent` | `craft/<version> <os> <arch>`, so tools such as create-vite know craft ran them |
| `INIT_CWD` | The directory craft was run from |

## Exit codes and signals

When a script fails, craft stops and exits with the script's exit code (128 + the signal number if it was killed), after one line:

```text
error: build exited with code 2
```

Ctrl-C reaches the script, and craft waits for it to exit.

## Listing scripts

```text
$ craft run
Scripts in my-app:
  build
    tsc
  start
    node dist/server.js
```

## Shortcuts

| Command | Same as |
| --- | --- |
| `craft start` | `craft run start` |
| `craft test`, `craft t` | `craft run test` |
