# craft install

Install the project's dependencies; with package names, add them like [`craft add`](./add).

```text
craft install [OPTIONS] [PACKAGES]...
```

Alias: `craft i`. `craft` with no command does the same.

## Options

| Option | Does |
| --- | --- |
| `--frozen-lockfile` | Install exactly what `craft.lock` records; fail if it is missing or out of date. On by default when the `CI` environment variable is set. |
| `--production`, `--prod` | Leave `devDependencies` out |
| `--ignore-scripts` | Run no lifecycle scripts |
| `-D`, `--dev` | With package names: save to `devDependencies` |
| `-E`, `--exact` | With package names: save the exact resolved version instead of a caret range |
| `-O`, `--save-optional` | With package names: save to `optionalDependencies` |
| `-P`, `--save-peer` | With package names: save to `peerDependencies`, and to `devDependencies` so it is installed for development |

Plus the [global options](./cli#global-options).

## What it does

1. Reads `package.json` and `craft.lock`. If they match what's already installed, prints `Already up to date` and stops.
2. Resolves the dependency graph, preferring the versions in `craft.lock`. Dependencies whose specifier changed in `package.json` resolve afresh. See [Lockfile](/guide/lockfile#how-installs-use-it).
3. Downloads what the global store doesn't have yet, checking each tarball's integrity.
4. Lays out `node_modules` from the store and removes what's no longer used. See [How it works](/guide/how-it-works).
5. Runs install scripts of [trusted dependencies](/guide/install-scripts), then the project's own `preinstall`, `install`, `postinstall` and `prepare`.
6. Writes `craft.lock` if it changed, and prints what changed.

## Examples

```sh
craft install                          # everything in package.json
craft install --frozen-lockfile        # exactly what craft.lock says
craft install --production             # without devDependencies
craft i react react-dom                # same as craft add react react-dom
craft i -D vitest                      # same as craft add -D vitest
```

A production install leaves `craft.lock` unchanged, and a later `craft install` adds the development dependencies back.
