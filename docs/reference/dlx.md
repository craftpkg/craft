# craft dlx

Run a package's bin without adding it to the project.

```text
craft dlx [OPTIONS] <COMMAND> [ARGS]...
```

Alias: `craft x`.

## Options

| Option | Does |
| --- | --- |
| `-p`, `--package <PACKAGE>` | The package to install, when its bin is named differently |

Plus the [global options](./cli#global-options).

## Behaviour

craft installs the package into its own directory under `~/.craft/dlx`, then runs its bin from your current directory, passing the arguments along and exiting with the bin's exit code. Your project's `package.json`, `craft.lock` and `node_modules` aren't touched, and `dlx` works outside any project.

- **Which version:** `craft dlx cowsay` runs the version `latest` points to, checking for a newer one each time. A version or range (`cowsay@1.6.0`, `create-vite@6`) reuses the earlier install, so it also works `--offline`.
- **Which bin:** the package's only bin, or else the one named after the package (without its scope). Name another with `-p`:
  ```sh
  craft dlx -p typescript tsc --version
  ```
- **`PATH`:** the package's own bins come first, then your project's `node_modules/.bin`.
- The install is quiet; warnings and errors still show.

## Examples

```sh
craft dlx create-vite my-app --template react-ts
craft x cowsay hello
craft dlx -p @angular/cli ng new my-app
craft dlx -p typescript@5.6 tsc --init
```

```text
$ craft dlx cowsay@1.6.0 hello
 _______
< hello >
 -------
        \   ^__^
         \  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||
```

A package with several bins and none named after it needs `-p`:

```text
error: toolbox has no bin named toolbox; it provides: tool-a, tool-b
```
