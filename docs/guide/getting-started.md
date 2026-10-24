# Quick start

This walks through a new project from `package.json` to running code. If you already have a project, `cd` into it and start at [Install dependencies](#install-dependencies).

## Create a project

```sh
mkdir my-app && cd my-app
craft init
```

```text
Created package.json
```

`craft init` writes a minimal `package.json` named after the directory. It never overwrites an existing one.

```json
{
  "name": "my-app",
  "version": "1.0.0"
}
```

## Add dependencies

```sh
craft add express
craft add -D typescript
```

```text
+ express 5.3.0
65 packages installed in 1.8s
+ typescript 7.0.2 (dev)
67 packages installed in 2.8s
```

`craft add` saves a caret range of the version it installed (`"express": "^5.3.0"`), writes `craft.lock`, and installs. Use `-D` for `devDependencies`, `-E` to save the exact version, `-O` for `optionalDependencies` and `-P` for `peerDependencies`. See [`craft add`](/reference/add).

## Install dependencies

```sh
craft install
```

On a fresh checkout this installs everything in `package.json`, following `craft.lock` when there is one. When nothing changed, craft notices without touching the network:

```text
Already up to date
```

`craft i` is the same command.

## Run scripts and bins

```json
{
  "scripts": {
    "build": "tsc",
    "start": "node dist/server.js"
  }
}
```

```sh
craft run build          # runs "tsc", with prebuild and postbuild around it
craft start              # shortcut for craft run start; craft test for "test"
craft tsc --version      # a dependency's bin, when no script has that name
craft run                # lists the scripts
```

Arguments after the script name are passed to it, quoted so they arrive unchanged: `craft run build --watch`. See [`craft run`](/reference/run).

To run a tool without adding it to the project, use [`craft dlx`](/reference/dlx):

```sh
craft dlx create-vite my-site --template react-ts
```

## Look around

```sh
craft list                # direct dependencies
craft list --depth 1      # and theirs
craft why debug           # every chain that installs debug
craft outdated            # what has newer versions
```

```text
$ craft why debug
debug 4.4.3
  dependencies: express 5.3.0 → body-parser 2.4.0 → debug 4.4.3
  dependencies: express 5.3.0 → debug 4.4.3
  dependencies: express 5.3.0 → finalhandler 2.1.1 → debug 4.4.3
  …
```

## Keep up to date

```sh
craft update              # newest versions your ranges allow
craft update --latest     # move to the latest versions, rewriting the ranges
craft remove express      # remove a dependency and what only it needed
```

## Commit the lockfile

Commit `package.json` and `craft.lock`, and add `node_modules` to `.gitignore`. In CI, run `craft ci` or `craft install --frozen-lockfile` (the default when `CI` is set) so the build installs exactly what the lockfile says. See [Continuous integration](./ci).
