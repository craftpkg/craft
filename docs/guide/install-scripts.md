# Install scripts

Packages can declare `preinstall`, `install` and `postinstall` scripts, which npm runs on every install. They are how native modules compile, and how malicious packages get onto developer machines. craft runs them only for packages you trust.

## Trusting a package

When an install skips scripts, craft says so:

```text
warning: skipped install scripts of 1 package not in trustedDependencies: bcrypt@6.0.0
  to allow them, add their names to "trustedDependencies" in package.json and run `craft install`
```

Trust it with [`craft trust`](/reference/trust), which adds the name to `package.json` and installs, running the scripts:

```sh
craft trust bcrypt
```

```text
Trusted bcrypt
Ran install scripts of bcrypt@6.0.0
72 packages installed in 0.6s
```

or edit `package.json` yourself and run `craft install`:

```json
{
  "trustedDependencies": ["bcrypt", "esbuild"]
}
```

Trust is by package name and applies wherever the package appears in your tree. Changing the list counts as a change, so the next `craft install` builds newly trusted packages even when nothing else changed.

## How builds run

- A trusted package's `preinstall`, `install` and `postinstall` run in that order, in the package's directory. A package with a `binding.gyp` and no `install` script gets an implicit `node-gyp rebuild`, as with npm.
- Packages are built after everything is linked, dependencies before dependents, up to eight at a time.
- Before its first build, a package's files are copied rather than linked from the store, so a build can never change the store.
- Each package builds once per project. A failed build is removed, so the next install tries again.
- Output is captured and shown when a build fails, or with `--debug`.
- If the build of a package that's only reachable through `optionalDependencies` fails, craft warns and carries on. Any other failed build fails the install with the script's output.

Scripts get the same environment as [`craft run`](/reference/run#environment): every `node_modules/.bin` from the package upwards on `PATH`, `npm_package_*` variables and `npm_config_user_agent`.

## Packages that work untrusted

Many packages with install scripts don't need them. esbuild, SWC and Lightning CSS, for example, ship their native binary as a [platform package](./platforms) and use their `postinstall` only as a fallback; they work without being trusted.

## Your project's own scripts

The scripts in your own `package.json` are always trusted. `craft install` and `craft ci` run the project's `preinstall`, `install`, `postinstall` and `prepare`, in that order, after its dependencies are installed.

## Turning scripts off

`--ignore-scripts` runs no lifecycle scripts at all, neither the project's nor its dependencies':

```sh
craft install --ignore-scripts
craft add sharp --ignore-scripts
craft ci --ignore-scripts
```
