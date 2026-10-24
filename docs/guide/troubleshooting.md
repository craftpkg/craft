# Troubleshooting

## Reading errors

Every error is one `error:` line, a `caused by:` line for each underlying cause, and, when the fix is a common one, a `hint:`:

```text
error: could not create directory /srv/app/node_modules: Permission denied (os error 13)
  hint: check that you may write to the path above, or run craft as its owner
```

The exit code is non-zero. When a script fails, craft exits with the script's own exit code and adds one line:

```text
error: test exited with code 2
```

Run any command with `--debug` to see what craft is doing: each request, link and script. Credentials are redacted.

## Common problems

### `package … not found in https://registry.npmjs.org`

```text
error: failed to install left-pda
  caused by: package left-pda not found in https://registry.npmjs.org
```

The name is misspelled, or the package lives in another registry. For a scoped package from a private registry, configure `@scope:registry`; see [Configuration](./configuration#scoped-registries). When the missing package is a dependency of a dependency, the error names the chain that requires it:

```text
error: failed to install ghost@^1.0.0 (required by express@4.21.2 > body-parser@1.20.3)
```

### `… denied access to … (HTTP 401)`

The registry needs credentials craft doesn't have, or has the wrong ones. Check the `_authToken` (or `_auth`) line for that registry's URL prefix in `.npmrc`, and that any `${VAR}` it uses is set.

### `could not reach registry.npmjs.org`

```text
caused by: could not reach registry.npmjs.org (Connection refused (os error 61)); check your network connection and proxy settings, or use --offline
```

The network or a proxy is in the way. Behind a corporate proxy, set `https-proxy` in `.npmrc` or `HTTPS_PROXY`. With a warm store, `--offline` installs without the network.

### `could not verify the certificate of …`

A private registry, or a proxy that inspects TLS, presents a certificate signed by a CA craft doesn't trust. Point `cafile` at that CA's certificate. See [Certificates](./configuration#certificates).

### `craft.lock is out of date with package.json`

A frozen install (`craft ci`, `--frozen-lockfile`, or any install with `CI` set) found `package.json` changed without `craft.lock`. Run `craft install` locally and commit the updated lockfile.

### `Cannot find module 'x'` after switching to craft

Your code uses a package it doesn't declare, and npm's flat `node_modules` was hiding that. Add it: `craft add x`. See [Migrating](./migrating#what-changes).

### A native module doesn't load

Its install script didn't run. craft printed `skipped install scripts of … not in trustedDependencies` during the install; trust the package with `craft trust <name>`. See [Install scripts](./install-scripts).

### `script "x" not found`

```text
error: script "deploy" not found; available scripts: build, test
```

There's no such script in `package.json`, and no bin of that name in `node_modules/.bin`.

### Disk space

The store keeps every version any project ever installed. `craft cache clean --force` removes it, along with the metadata cache; projects keep working, and the next install downloads what it needs. A full disk produces:

```text
hint: the disk is full; `craft cache clean --force` frees the global store
```

## Reporting a bug

Run the failing command again with `--debug`, and open an issue at [github.com/craftpkg/craft](https://github.com/craftpkg/craft/issues) with the output, `craft --version`, and your operating system.
