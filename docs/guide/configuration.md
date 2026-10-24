# Configuration

craft reads npm's configuration, so an existing `.npmrc` keeps working.

## Where settings come from

Later sources override earlier ones:

1. `~/.npmrc`, or the file `NPM_CONFIG_USERCONFIG` names;
2. the project's `.npmrc`, next to `package.json`;
3. `npm_config_*` environment variables, with `_` standing for `-` (`npm_config_fetch_retries=5`);
4. command-line flags such as `--offline`.

Values can use `${VAR}` to read an environment variable, and may be quoted.

```ini
registry=https://registry.npmjs.org/
@acme:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
fetch-retries=3
```

## Registries

| Key | Meaning |
| --- | --- |
| `registry` | The default registry. `CRAFT_REGISTRY` overrides it. |
| `@scope:registry` | The registry for packages in `@scope`. |

### Scoped registries

```ini
@acme:registry=https://npm.pkg.github.com
@jsr:registry=https://npm.jsr.io
```

With the second line, `craft add @jsr/std__path` installs [JSR](https://jsr.io) packages through JSR's npm-compatible registry.

## Authentication

Credentials are configured per registry URL prefix (npm's "nerf dart" form) and are sent only to URLs under that prefix, choosing the longest match. A token for your private registry never reaches the public one, and credentials print as `<redacted>` in `--debug` output.

| Key | Meaning |
| --- | --- |
| `//host/path/:_authToken` | A bearer token |
| `//host/path/:_auth` | Base64 of `username:password`, sent as basic auth |
| `//host/path/:username` and `//host/path/:_password` | Basic auth; `_password` is base64-encoded, as with npm |
| `_authToken` | Legacy: a token for the default registry |

```ini
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
//registry.internal.example/:username=ci
//registry.internal.example/:_password=${REGISTRY_PASSWORD_BASE64}
```

A registry that refuses craft's credentials gives:

```text
error: failed to install @acme/ui@^2.0.0
  caused by: https://npm.pkg.github.com denied access to @acme/ui (HTTP 401); check your registry credentials
```

## Network

| Key | Default | Meaning |
| --- | --- | --- |
| `fetch-retries` | `2` | How many times to retry a request after a connection error, timeout or HTTP 408, 429 or 5xx |
| `network-concurrency` | `16` | Requests in flight at once |
| `https-proxy` | | Proxy for `https://` URLs |
| `proxy` | | Proxy for `http://` URLs |
| `noproxy` (or `no-proxy`) | | Comma-separated hosts to reach directly |
| `strict-ssl` | `true` | `false` skips certificate verification |
| `cafile` | | A PEM file of extra CA certificates to trust |
| `offline` | `false` | Same as `--offline` |
| `prefer-offline` | `false` | Same as `--prefer-offline` |

Without `proxy` / `https-proxy`, craft uses the `HTTPS_PROXY`, `HTTP_PROXY` and `NO_PROXY` environment variables. Connections time out after 10 seconds, and a response that stalls for 60 seconds is retried.

### Certificates

craft trusts a bundled set of public root certificates (Mozilla's), like Node.js does, rather than the operating system's certificate store. A private registry or a TLS-inspecting corporate proxy with its own CA needs `cafile`:

```ini
cafile=/etc/ssl/certs/corp-root-ca.pem
```

Otherwise the install stops with:

```text
caused by: could not verify the certificate of registry.internal.example (invalid peer certificate: UnknownIssuer); point cafile in .npmrc at your registry's CA certificate, or set strict-ssl=false
```

### Offline modes

| Flag | Behaviour |
| --- | --- |
| `--prefer-offline` | Use cached registry metadata without revalidating it; fetch only what is missing. |
| `--offline` | Never touch the network: use only cached metadata and packages already in the store. |

```text
$ craft --offline add left-pad
error: failed to install left-pad
  caused by: failed to fetch left-pad from https://registry.npmjs.org
  caused by: https://registry.npmjs.org/left-pad is not cached and craft is offline
```

A project with a lockfile whose packages are all in the store installs offline, which is handy on a plane or in a sandboxed build.

## Environment variables

| Variable | Meaning |
| --- | --- |
| `CRAFT_HOME` | Where the store and caches live. Default: `~/.craft` |
| `CRAFT_REGISTRY` | Overrides the default registry |
| `CRAFT_OS`, `CRAFT_CPU`, `CRAFT_LIBC` | Install platform packages for another platform. See [Platform packages](./platforms#installing-for-another-platform) |
| `CI` | When set (to anything but `0` or `false`), installs are [frozen](./lockfile#frozen-installs) and progress is printed as plain lines |
| `NPM_CONFIG_USERCONFIG` | Use this file instead of `~/.npmrc` |
| `npm_config_*` | Any `.npmrc` key |

## Not supported yet

`always-auth`, client certificates (`certfile` / `keyfile`), and `.npmrc` sections are ignored.
