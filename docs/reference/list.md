# craft list

List the project's dependencies, and theirs with `--depth`.

```text
craft list [OPTIONS]
```

Alias: `craft ls`.

## Options

| Option | Does |
| --- | --- |
| `--depth <DEPTH>` | How many levels of dependencies to show below the project's own. Default: `0` |

Plus the [global options](./cli#global-options).

## Output

```text
$ craft list
dependencies:
express 5.3.0

devDependencies:
typescript 7.0.2
```

```text
$ craft list --depth 1
dependencies:
express 5.3.0
├── accepts 2.0.0
├── body-parser 2.4.0
├── content-disposition 1.0.0
…
```

Dependencies are grouped by `package.json` section and sorted by name. In the tree:

- peers and optional dependencies are marked `(peer)` and `(optional)`;
- a package that depends back on one of its ancestors is marked `(cycle)` and not expanded again;
- an alias shows the package it stands for: `react-18 (npm:react@18.3.1)`.

`craft list` reads only `craft.lock`; it doesn't contact the registry or look at `node_modules`. Without a lockfile:

```text
error: craft.lock is missing; run `craft install` to create it
```
