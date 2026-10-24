# craft remove

Remove packages from `package.json` and `node_modules`.

```text
craft remove [OPTIONS] <PACKAGES>...
```

Aliases: `craft rm`, `craft uninstall`, `craft un`.

Takes the [global options](./cli#global-options).

## Behaviour

- Removes each package from `dependencies`, `devDependencies`, `optionalDependencies` and `peerDependencies`, and drops a section left empty.
- Removes the package's link and its bins from `node_modules`, then every package nothing else needs any more. Shared dependencies stay.
- Edits `craft.lock` directly, without contacting the registry.
- A package that isn't a dependency produces a warning, and nothing changes:
  ```text
  warning: lodash is not a dependency of this project
  ```

## Example

```text
$ craft remove express
- express 5.3.0
```
