# craft add

Add packages to `package.json` and install them.

```text
craft add [OPTIONS] <PACKAGES>...
```

## Options

| Option | Does |
| --- | --- |
| `-D`, `--dev` | Save to `devDependencies` |
| `-E`, `--exact` | Save the exact resolved version instead of a caret range |
| `-O`, `--save-optional` | Save to `optionalDependencies` |
| `-P`, `--save-peer` | Save to `peerDependencies`, and to `devDependencies` so it is installed for development |
| `--ignore-scripts` | Run no lifecycle scripts |

Plus the [global options](./cli#global-options).

## Package specifiers

| You type | Saved as |
| --- | --- |
| `react` | `"react": "^19.2.0"`, a caret range of the version installed |
| `react@18` | `"react": "^18.3.1"` |
| `react@18.3.1` | `"react": "^18.3.1"` (with `-E`: `"18.3.1"`) |
| `typescript@next` | a caret range of what `next` points to |
| `react-18@npm:react@^18` | `"react-18": "npm:react@^18.3.1"` |
| `@acme/ui` | `"@acme/ui": "^2.0.0"` |
| `./packages/ui` | `"ui": "file:packages/ui"`, named after the package found there |
| `github:vercel/ms#2.1.3` | as given |
| `https://example.com/pkg.tgz` | as given |

See [Dependency sources](/guide/dependency-sources) for every form.

## Behaviour

- `craft add` reads `package.json` before contacting the registry, so a missing file fails at once:
  ```text
  error: no package.json found; run `craft init` to create one
  ```
- The packages you name resolve afresh, ignoring versions locked in `craft.lock`; the rest of the project keeps its locked versions.
- A package already in another section moves: `craft add -D react` takes `react` out of `dependencies`.
- Entries are inserted in alphabetical order, and `package.json` keeps its indentation, key order and line endings.
- The whole project is installed, so `node_modules` and `craft.lock` stay consistent.

## Examples

```sh
craft add express
craft add -D typescript vitest @types/node
craft add -E lodash@4.17.21
craft add -O fsevents
craft add -P react@^19
craft add sw@npm:string-width@^4
```

```text
$ craft add express
+ express 5.3.0
65 packages installed in 1.8s
```
