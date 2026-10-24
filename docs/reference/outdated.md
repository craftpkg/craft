# craft outdated

Show direct dependencies with newer versions available.

```text
craft outdated [OPTIONS]
```

Takes the [global options](./cli#global-options).

## Output

```text
$ craft outdated
Package     Current  Wanted  Latest  Type
express     4.21.2   4.22.3  5.3.0   dependencies
ms          2.0.0    2.0.0   2.1.3   dependencies
typescript  5.6.3    5.9.3   7.0.2   devDependencies
```

| Column | Meaning |
| --- | --- |
| Current | The version installed, from `craft.lock` |
| Wanted | The newest version the specifier in `package.json` allows: what [`craft update`](./update) installs |
| Latest | The version the `latest` dist-tag points to: what `craft update --latest` installs |
| Type | The `package.json` section |

Only dependencies that are behind are listed, and only registry dependencies are checked. When nothing is behind:

```text
All dependencies are up to date
```

`craft outdated` exits with `0` either way.
