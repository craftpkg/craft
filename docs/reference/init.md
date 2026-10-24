# craft init

Create a `package.json` in the current directory.

```text
craft init [OPTIONS]
```

Takes the [global options](./cli#global-options).

## Behaviour

Writes a minimal `package.json`, named after the directory:

```text
$ mkdir my-app && cd my-app && craft init
Created package.json
```

```json
{
  "name": "my-app",
  "version": "1.0.0"
}
```

The directory name is turned into a valid package name: `My App` becomes `my-app`.

`craft init` never overwrites an existing file:

```text
error: package.json already exists
```
