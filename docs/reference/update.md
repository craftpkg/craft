# craft update

Update dependencies within their specifiers, or to their latest versions with `--latest`.

```text
craft update [OPTIONS] [PACKAGES]...
```

Aliases: `craft up`, `craft upgrade`.

## Arguments and options

| | Does |
| --- | --- |
| `[PACKAGES]...` | The dependencies to update; all of them when none are given |
| `-L`, `--latest` | Move to the latest versions and rewrite the specifiers in `package.json` |

Plus the [global options](./cli#global-options).

## Within the specifiers

Without `--latest`, `package.json` doesn't change. craft re-resolves the named dependencies (or all of them), ignoring their locked versions, to the newest versions their ranges allow, along with their own dependencies. Other dependencies keep what's locked.

```text
$ craft update
~ express 4.21.2 → 4.22.3
~ typescript 5.6.3 → 5.9.3
69 packages installed in 0.9s
```

## To the latest versions

With `--latest`, each dependency moves to the version its `latest` dist-tag points to, and its specifier is rewritten in the same style:

| Before | After |
| --- | --- |
| `"ms": "2.0.0"` | `"ms": "2.1.3"` |
| `"react": "~18.3.0"` | `"react": "~19.2.0"` |
| `"express": "^4.21.2"` | `"express": "^5.3.0"` |
| `"react-18": "npm:react@^18.0.0"` | `"react-18": "npm:react@^19.2.0"` |

```text
$ craft update --latest ms
~ ms 2.0.0 → 2.1.3
69 packages installed in 0.0s
```

Exact versions stay exact and `~` ranges stay `~`; any other range becomes a caret range. `--latest` only rewrites registry dependencies. Git dependencies are re-resolved by any update, so one on a branch or a `semver:` range moves to its newest commit or tag, while one pinned to a tag or commit stays put.

Naming a package that isn't a dependency is an error:

```text
error: lodash is not a dependency of this project
```

See [`craft outdated`](./outdated) to see what an update would change first.
