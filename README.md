<p align="center">
  <img src="docs/public/logo.png" alt="Craft Logo" width="200" />
</p>

# 📦 Craft Package Manager

![CI](https://github.com/craftpkg/craft/actions/workflows/ci.yml/badge.svg)
![Release](https://github.com/craftpkg/craft/actions/workflows/release.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

**Craft** is a fast, strict package manager for Node.js, written in Rust. It installs from the npm registry (or any npm-compatible one) into a pnpm-style `node_modules`, keeps a deterministic `craft.lock`, and runs dependency install scripts only for packages you trust.

**📖 Documentation: [craftpkg.github.io/craft](https://craftpkg.github.io/craft/)**

## ✨ Features

- 🚀 **Fast**: parallel downloads, a revalidated metadata cache, and files cloned from a global store. An unchanged project installs in milliseconds.
- 🔒 **Deterministic**: `craft.lock` pins every version, source and integrity hash; installs in CI are frozen by default.
- 📦 **Strict**: every package sees only the dependencies it declares, so undeclared imports fail on your machine, not in production.
- 🛡️ **Safe by default**: tarballs are integrity-checked, and dependencies' install scripts run only when listed in `trustedDependencies`.
- 🧩 **npm-compatible**: ranges, dist-tags, aliases, git, tarball, `file:` and `link:` dependencies, peers, platform packages, and `.npmrc` registries and auth.
- 🖥️ **Everywhere**: native binaries for Linux (glibc and musl), macOS and Windows.

## 🚀 Installation

```bash
curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
```

Or download a binary from the [Releases](https://github.com/craftpkg/craft/releases) page. See [Installation](https://craftpkg.github.io/craft/guide/installation) for Windows, checksums and building from source.

## 📖 Usage

```bash
craft init                    # create a package.json
craft install                 # install everything in package.json
craft add react               # add a dependency (-D dev, -E exact, -O optional, -P peer)
craft remove react            # remove one
craft update                  # update within your ranges; --latest to move them
craft run build               # run a script; craft tsc runs a dependency's bin
craft dlx create-vite my-app  # run a package without adding it
craft ci                      # clean, frozen install for CI
```

Every command is documented in the [CLI reference](https://craftpkg.github.io/craft/reference/cli).

## 🤝 Contributing

Contributions are welcome! `make` runs formatting, lints and the full test suite. See [Contributing](https://craftpkg.github.io/craft/contributing/) for the architecture, the test suites and how to work on the docs.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
