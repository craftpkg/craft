# CLI overview

```text
craft [OPTIONS] [COMMAND]
```

## Commands

| Command | Aliases | Does |
| --- | --- | --- |
| [`install`](./install) | `i` | Install the project's dependencies; with package names, add them like `craft add` |
| [`add`](./add) | | Add packages to `package.json` and install them |
| [`remove`](./remove) | `rm`, `uninstall`, `un` | Remove packages from `package.json` and `node_modules` |
| [`update`](./update) | `up`, `upgrade` | Update dependencies within their specifiers, or to their latest versions with `--latest` |
| [`outdated`](./outdated) | | Show direct dependencies with newer versions available |
| [`list`](./list) | `ls` | List the project's dependencies, and theirs with `--depth` |
| [`why`](./why) | | Show every chain of dependencies that installs a package |
| [`run`](./run) | | Run a `package.json` script, or list them when no name is given |
| [`start`](./run#shortcuts) | | Run the `start` script |
| [`test`](./run#shortcuts) | `t` | Run the `test` script |
| [`exec`](./exec) | | Run a bin from the project's dependencies |
| [`dlx`](./dlx) | `x` | Run a package's bin without adding it to the project |
| [`trust`](./trust) | | Allow packages to run install scripts, then install |
| [`init`](./init) | | Create a `package.json` in the current directory |
| [`ci`](./ci) | | Remove `node_modules` and install exactly what `craft.lock` records |
| [`cache clean`](./cache) | | Remove every stored package and cached registry response |

Any other word runs the script of that name, or else the dependency bin of that name: `craft build`, `craft vitest --run`. See [`craft run`](./run#scripts-and-bins).

With no command, craft runs `craft install`.

## Global options

These work with every command, before or after it.

| Option | Does |
| --- | --- |
| `-v`, `--version` | Print the version |
| `-h`, `--help` | Print help; `craft <command> --help` for a command |
| `--debug` | Print debug logs of what craft is doing. Credentials are redacted. |
| `-s`, `--silent` | Print nothing but warnings and errors |
| `--offline` | Use only cached registry metadata and stored packages; never touch the network |
| `--prefer-offline` | Use cached registry metadata without revalidating it; fetch only what is missing |

`--debug` and `--silent` can't be combined, and neither can `--offline` and `--prefer-offline`.

## Output

On a terminal, craft shows one live status line while it works (`Resolving dependencies… 143 packages`, `Linking 218 packages`), then a summary of what changed in your direct dependencies:

```text
+ react 19.2.0
+ vitest 3.2.4 (dev)
- lodash 4.17.21
~ typescript 5.6.3 → 5.9.3
218 packages installed in 1.9s
```

When stderr isn't a terminal, in CI, or with `--debug`, the status line is replaced by plain output. Warnings and errors always go to stderr.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | craft failed; the message says why |
| other | A script or bin failed with this code, which craft passes through. A script killed by a signal reports 128 + the signal number. |
