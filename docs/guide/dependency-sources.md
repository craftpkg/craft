# Dependency sources

craft accepts the specifiers npm does, in `package.json` and on the `craft add` command line.

## Registry packages

```json
{
  "dependencies": {
    "react": "^19.0.0",
    "lodash": "4.17.21",
    "typescript": "next",
    "debug": "*"
  }
}
```

- **Ranges** follow npm's semver rules. craft picks the version the `latest` dist-tag points to when it satisfies the range, and otherwise the highest satisfying stable version.
- **Exact versions** install that version.
- **Dist-tags** such as `latest` or `next` install the tagged version.
- **`*` and `""`** mean any version.

`craft add react` saves a caret range of the version it installs (`^19.2.0`); `craft add -E react` saves the exact version.

Packages under a scope can come from their own registry. See [Configuration](./configuration#scoped-registries).

## Aliases

Install a package under another name with `npm:`:

```json
{
  "dependencies": {
    "react-18": "npm:react@^18.3.0"
  }
}
```

```sh
craft add sw@npm:string-width@^4
```

The package is linked at `node_modules/react-18`, and `require('react-18')` loads it. Aliases inside dependencies work the same way.

## Git repositories

```json
{
  "dependencies": {
    "ms": "github:vercel/ms#2.1.3",
    "dequal": "git+https://github.com/lukeed/dequal.git#semver:^2.0.0",
    "my-lib": "git+ssh://git@github.com:me/my-lib.git#main"
  }
}
```

Accepted forms:

| Form | Example |
| --- | --- |
| Hosted shorthand | `github:owner/repo`, `gitlab:owner/repo`, `bitbucket:owner/repo`, `gist:id` |
| GitHub shorthand | `owner/repo` |
| Git URLs | `git+https://…`, `git+ssh://…`, `git+file://…`, `git://…` |
| scp-style | `git@github.com:owner/repo.git` |

After `#`, give a branch, tag or commit, or `semver:<range>` to pick the highest tag that satisfies the range. Without a ref, craft uses the default branch.

craft pins the ref to a commit with `git ls-remote` (so `git` must be installed) and records the commit in `craft.lock`; later installs stay on it. Public repositories on GitHub, GitLab and Bitbucket are downloaded as the host's tarball of that commit, with no clone. Other repositories, and hosted ones whose tarball isn't reachable (private repositories), are fetched with `git`, using your usual git credentials.

::: warning Repositories that need a build
craft doesn't run a git dependency's `prepare` script yet. A repository that only contains its built files after `prepare` (many TypeScript libraries) installs without them. Depend on the published package, or on its release tarball, instead.
:::

## Tarball URLs

```json
{
  "dependencies": {
    "ms": "https://registry.npmjs.org/ms/-/ms-2.1.3.tgz"
  }
}
```

Each URL is downloaded once, extracted into the store by content, and locked with its `sha512` integrity.

## Local packages

```json
{
  "dependencies": {
    "ui": "file:./packages/ui",
    "fixtures": "file:./vendor/fixtures-1.0.0.tgz",
    "sdk": "link:../sdk"
  }
}
```

| Specifier | What craft does |
| --- | --- |
| `file:<directory>` | Copies the directory into the project (without its `node_modules` and `.git`) on **every** install, and installs its registry dependencies. Edits show up on the next install. |
| `file:<tarball>` | Extracts the tarball into the store by its content hash. |
| `link:<directory>` | Links the directory in place. Its dependencies are **not** installed; it uses its own `node_modules`. Edits show up immediately. |

Relative paths resolve against the directory of the `package.json` that declares them, and `~/` against your home directory.

`craft add ./packages/ui` names the dependency after the package it finds there and saves `"ui": "file:packages/ui"`. URLs and `file:` / `link:` specifiers are saved as given.

## Not supported yet

`workspace:` specifiers fail with `workspace dependencies are not supported yet`, since craft doesn't support workspaces yet.
