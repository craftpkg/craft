# craft ci

Remove `node_modules` and install exactly what `craft.lock` records.

```text
craft ci [OPTIONS]
```

## Options

| Option | Does |
| --- | --- |
| `--ignore-scripts` | Run no lifecycle scripts |

Plus the [global options](./cli#global-options).

## Behaviour

1. Checks that `craft.lock` exists and matches `package.json`, failing before any download otherwise:
   ```text
   error: craft.lock is out of date with package.json: ms (not in craft.lock); run `craft install` to update it
   ```
2. Removes `node_modules`.
3. Installs from the lockfile alone, with no registry metadata requests, taking tarballs from the global store when it has them.
4. Runs the install scripts of [trusted dependencies](/guide/install-scripts) and the project's own `preinstall`, `install`, `postinstall` and `prepare`.

`craft.lock` is never rewritten. To install from the lockfile without removing `node_modules` first, use `craft install --frozen-lockfile`.

See [Continuous integration](/guide/ci) for complete CI setups.
