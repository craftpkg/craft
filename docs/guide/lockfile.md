# Lockfile

craft writes `craft.lock` next to `package.json` after every install, `add`, `update` and `remove`. Commit it: with a lockfile, every machine installs the same versions from the same sources, without asking the registry which versions exist.

## Format

`craft.lock` is JSON with sorted keys and a trailing newline, so diffs are small and the file is identical on every platform.

```json
{
  "lockfileVersion": 1,
  "importers": {
    ".": {
      "dependencies": {
        "express": { "specifier": "^5.3.0", "version": "express@5.3.0" }
      },
      "devDependencies": {
        "typescript": { "specifier": "^7.0.2", "version": "typescript@7.0.2" }
      }
    }
  },
  "packages": {
    "express@5.3.0": {
      "name": "express",
      "version": "5.3.0",
      "resolution": {
        "type": "registry",
        "tarball": "https://registry.npmjs.org/express/-/express-5.3.0.tgz",
        "integrity": "sha512-…"
      },
      "dependencies": {
        "body-parser": "body-parser@2.4.0",
        "debug": "debug@4.4.3"
      }
    }
  }
}
```

- **`importers["."]`** lists the project's own `dependencies`, `devDependencies` and `optionalDependencies`, each with the specifier from `package.json` and the package it resolved to.
- **`packages`** has one entry per package instance, keyed like the virtual store: `name@version`, with the resolved peers appended for packages that have them (`react-redux@9.2.0_react@19.1.0`).
- Each package lists its `dependencies`, `peerDependencies` and `optionalDependencies` as package keys, and its `os`, `cpu` and `libc` constraints when it has any.

### Resolutions

| `type` | Fields | Used for |
| --- | --- | --- |
| `registry` | `tarball`, `integrity` | packages from a registry |
| `tarball` | `url`, `integrity` | `https://…/pkg.tgz` dependencies |
| `localTarball` | `path`, `integrity` | `file:` paths to `.tgz` files |
| `directory` | `path` | `file:` paths to directories |
| `link` | `path` | `link:` dependencies |
| `git` | `url`, `commit` | git dependencies, pinned to the commit |

Local paths are relative to the project and always use `/`.

Packages for other platforms, such as the Linux build of a native module when you're on a Mac, are locked too, so one lockfile installs everywhere. See [Platform packages](./platforms).

## How installs use it

- **Locked versions win.** A dependency whose specifier hasn't changed keeps its locked version, even when newer matching versions exist. Git dependencies stay on their locked commit.
- **Changed specifiers re-resolve.** Edit a range in `package.json` and only that dependency resolves afresh; its own dependencies still prefer what's locked.
- **`craft add` resolves the packages you name afresh**, so `craft add react` gets the latest even if an older `react` is locked.
- **`craft update`** re-resolves within the specifiers, ignoring the locked versions of what it updates. See [`craft update`](/reference/update).
- **`craft remove`** edits the lockfile directly, dropping the dependency and every package only it needed, without contacting the registry.

## Frozen installs

```sh
craft install --frozen-lockfile
```

A frozen install uses only the lockfile. It fails before any download when:

- `craft.lock` is missing:
  ```text
  error: craft.lock is missing; run `craft install` to create it
  ```
- it disagrees with `package.json`, listing every difference:
  ```text
  error: craft.lock is out of date with package.json: debug (^4.4.0 in package.json, ^4.0.0 in craft.lock), ms (not in craft.lock); run `craft install` to update it
  ```
- something it needs isn't locked:
  ```text
  error: ms@^2.1.3 is not in craft.lock; run `craft install` to update it
  ```

and it never rewrites the lockfile. Installs are frozen by default when the `CI` environment variable is set (to anything but `0` or `false`). [`craft ci`](/reference/ci) removes `node_modules` first, then runs a frozen install.

## Versions

`lockfileVersion` is `1`. A lockfile written by a newer craft is rejected instead of misread:

```text
error: craft.lock has lockfileVersion 2, but this craft reads up to 1; upgrade craft
```

craft can't import `package-lock.json`, `yarn.lock` or `pnpm-lock.yaml` yet. The first `craft install` in a project resolves from `package.json` and writes `craft.lock`.
