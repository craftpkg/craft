<p align="center">
  <img src="docs/assets/logo.png" alt="Craft Logo" width="200" />
</p>

# 📦 Craft Package Manager

![CI](https://github.com/craftpkg/craft/actions/workflows/ci.yml/badge.svg)
![Release](https://github.com/craftpkg/craft/actions/workflows/release.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

**Craft** is a fast, strict package manager for Node.js, written in Rust. It installs from the npm registry (or any npm-compatible one) into a pnpm-style `node_modules`, keeps a deterministic `craft.lock`, and runs dependency install scripts only for packages you trust.

## ✨ Features

- 🚀 **Fast**: parallel resolution and downloads, packuments cached on disk and revalidated with ETags, files cloned (copy-on-write) from a global store. An unchanged project installs in milliseconds.
- 🔒 **Deterministic**: `craft.lock` records every resolved version, tarball and integrity; `craft install --frozen-lockfile` (the default in CI) installs exactly that.
- 📦 **Strict, isolated layout**: every package sees only the dependencies it declares, each project has its own `node_modules/.craft`, and the global store is never modified after extraction.
- 🧩 **npm-compatible**: semver ranges, dist-tags, `npm:` aliases, git and tarball URLs, `file:` and `link:` dependencies, peer and optional dependencies, `os`/`cpu`/`libc` platform packages, `.npmrc` registries and auth.
- 🛡️ **Safe by default**: tarballs are verified against their integrity hash, and dependencies' install scripts run only when listed in `trustedDependencies`.

## 🚀 Installation

### One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
```

The script downloads the binary for your platform (a static build on musl/Alpine), verifies its SHA-256 checksum, and installs it to `/usr/local/bin` when writable or `~/.local/bin` otherwise — no `sudo`. Set `CRAFT_INSTALL_DIR` to choose the directory and `CRAFT_VERSION` to pin a release.

### From a release

Download a binary and its `.sha256` from the [Releases](https://github.com/craftpkg/craft/releases) page: Linux (x64 and arm64, glibc and musl), macOS (Intel and Apple Silicon). Windows binaries are published too, but Windows support is still experimental.

### From source

```bash
git clone https://github.com/craftpkg/craft.git
cd craft
cargo install --path apps/craft
```

## 📖 Usage

```bash
craft init                    # create a package.json
craft install                 # install everything in package.json (alias: craft i)
craft add react react-dom     # add dependencies
craft add -D typescript       # …to devDependencies (-E exact, -O optional, -P peer)
craft remove react            # remove dependencies (aliases: rm, uninstall, un)
craft update [pkg…]           # update within the ranges in package.json; --latest to move ranges
craft outdated                # show what has newer versions
craft list --depth 1          # show the dependency tree (alias: ls)
craft why ms                  # show why a package is installed
craft run build               # run a script, with its pre/post scripts (craft run lists them)
craft start / craft test      # shortcuts for craft run start / test
craft tsc --noEmit            # run a dependency's bin directly
craft exec eslint .           # run a dependency's bin, never a script
craft dlx create-vite my-app  # run a package without adding it (alias: craft x)
craft trust esbuild           # allow a dependency's install scripts, then install
craft ci                      # clean, frozen install for CI
craft cache clean --force     # remove the global store and metadata cache
```

Global flags: `--offline` (never touch the network), `--prefer-offline` (use cached metadata without revalidating), `--silent`, `--debug`. `craft -v` prints the version. Every command has `--help`.

## 🧠 How it works

- **Global store** — each package is extracted once into `~/.craft/store/v1/<name>@<version>-<hash>/` after its integrity is verified, and never written to afterwards.
- **Per-project virtual store** — every package lives in `node_modules/.craft/<name>@<version>/node_modules/<name>`, imported from the store by clone, hardlink or copy (whichever the filesystem supports), with its dependencies linked beside it. Your `node_modules/<dep>` entries are links into `.craft`. Packages that `require` something they never declared still find it through `node_modules/.craft/node_modules`, while your own code only sees what `package.json` lists.
- **Peer dependencies** resolve to what the parent provides; a package used with different peers gets one copy per peer set (`ui@1.0.0_react@18.3.1`).
- **`craft.lock`** is sorted JSON meant to be committed. With it, installs make no registry metadata requests at all; `craft add`, `craft remove` and `craft update` keep it up to date. Anything removed from `package.json` is pruned from `node_modules` on the next install.

## 🛡️ Install scripts

Dependencies' `preinstall` / `install` / `postinstall` scripts (and implicit `node-gyp` builds) run only for packages listed in `package.json`:

```json
{
  "trustedDependencies": ["bcrypt", "esbuild"]
}
```

Craft lists every package whose scripts it skipped; `craft trust <pkg>` adds it and builds it. Your project's own lifecycle scripts always run, and `--ignore-scripts` turns all of them off.

## ⚙️ Configuration

Craft reads `.npmrc` from your home directory and the project, plus `npm_config_*` variables:

```ini
registry=https://registry.npmjs.org/
@acme:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
https-proxy=http://proxy.internal:8080
fetch-retries=3
network-concurrency=16
```

Supported keys: `registry`, `@scope:registry`, `//host/:_authToken`, `_auth`, `username` + `_password`, `https-proxy`, `proxy`, `noproxy`, `strict-ssl`, `cafile`, `fetch-retries`, `network-concurrency`, `offline`, `prefer-offline`. Credentials are only ever sent to the host they're configured for.

| Environment variable | Purpose |
| --- | --- |
| `CRAFT_HOME` | Where the store and caches live (default `~/.craft`) |
| `CRAFT_REGISTRY` | Override the default registry |
| `CRAFT_OS` / `CRAFT_CPU` / `CRAFT_LIBC` | Install platform packages for another platform (e.g. building a Linux image on a Mac) |
| `CI` | When set, installs require an up-to-date `craft.lock` |

## 🤝 Contributing

Contributions are welcome! `make` runs formatting, lints and the full test suite (the end-to-end tests need Node.js on `PATH`).

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
