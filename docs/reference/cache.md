# craft cache

Manage the global package store and metadata cache.

```text
craft cache clean [OPTIONS]
```

## craft cache clean

Remove every stored package and cached registry response.

| Option | Does |
| --- | --- |
| `-f`, `--force` | Confirm the removal |

Plus the [global options](./cli#global-options).

Without `--force`, nothing is removed and craft explains how to confirm. With it, craft deletes the store (`~/.craft/store`) and the metadata cache (`~/.craft/cache/metadata`), or the same directories under `CRAFT_HOME`. Installs made by [`craft dlx`](./dlx) in `~/.craft/dlx` are kept.

Projects keep working afterwards: their `node_modules` hold their own copies of the files. The next install anywhere downloads again what it needs.

```sh
craft cache clean --force
```
