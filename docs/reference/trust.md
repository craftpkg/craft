# craft trust

Allow packages to run install scripts, then install.

```text
craft trust [OPTIONS] <PACKAGES>...
```

Takes the [global options](./cli#global-options).

## Behaviour

Adds the packages to `trustedDependencies` in `package.json` (kept sorted, without duplicates), then runs [`craft install`](./install), which builds them.

```text
$ craft trust bcrypt
Trusted bcrypt
Ran install scripts of bcrypt@6.0.0
72 packages installed in 0.6s
```

A package is trusted wherever it appears in the dependency tree. To stop trusting one, remove it from `trustedDependencies`.

See [Install scripts](/guide/install-scripts) for how builds run.
