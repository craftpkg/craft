# craft why

Show every chain of dependencies that installs a package.

```text
craft why [OPTIONS] <PACKAGE>
```

Takes the [global options](./cli#global-options).

## Output

Chains are grouped by the version they reach, each starting from the `package.json` section that pulls it in:

```text
$ craft why ms
ms 2.0.0
  dependencies: express 4.21.2 → body-parser 1.20.3 → debug 2.6.9 → ms 2.0.0
  dependencies: express 4.21.2 → debug 2.6.9 → ms 2.0.0
  …
  dependencies: ms 2.0.0
ms 2.1.3
  dependencies: express 4.21.2 → send 0.19.0 → ms 2.1.3
  …
```

Up to ten chains are shown per version, followed by `… and N more`. A package matches by its name or by the alias it's installed under.

```text
$ craft why left-pad
left-pad is not installed
```

Like [`craft list`](./list), `craft why` reads only `craft.lock`.
