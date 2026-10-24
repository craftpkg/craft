# craft exec

Run a bin from the project's dependencies.

```text
craft exec [OPTIONS] <COMMAND> [ARGS]...
```

Takes the [global options](./cli#global-options).

## Behaviour

`craft exec <bin>` runs the bin from the nearest `node_modules/.bin`, searching the current directory and its parents, with the [same environment as scripts](./run#environment). Unlike [`craft run`](./run) and `craft <name>`, it never runs a `package.json` script, even one with the same name.

Arguments after the bin's name are passed to it unchanged, and its exit code becomes craft's.

```sh
craft exec tsc --noEmit
craft exec eslint . --fix
craft exec vitest run --reporter=dot
```

A bin that isn't installed fails, suggesting [`craft dlx`](./dlx):

```text
error: no prettier in node_modules/.bin; add the package that provides it, or try `craft dlx prettier`
```
