# Installation

craft is a single binary with no runtime dependencies. It needs [Node.js](https://nodejs.org) only to run your scripts and packages.

## Install script (macOS and Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
```

The script:

- picks the binary for your system, including a statically linked build on musl systems such as Alpine;
- downloads it with its published SHA-256 checksum and refuses to install a binary that doesn't match;
- installs it to `/usr/local/bin` when that is writable, and to `~/.local/bin` otherwise, without `sudo`;
- prints a reminder if the directory isn't on your `PATH`, then runs `craft --version`.

It reads three environment variables:

| Variable | Effect |
| --- | --- |
| `CRAFT_VERSION` | Install that release instead of the latest, e.g. `CRAFT_VERSION=0.1.0` |
| `CRAFT_INSTALL_DIR` | Install into this directory |
| `CRAFT_DOWNLOAD_URL` | Download release assets from this base URL, such as an internal mirror |

```sh
curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | CRAFT_INSTALL_DIR="$HOME/bin" bash
```

## Release binaries

Every [release](https://github.com/craftpkg/craft/releases) publishes these binaries, each with a `.sha256` file beside it:

| Platform | Asset |
| --- | --- |
| Linux x64 (glibc) | `craft-linux-amd64` |
| Linux arm64 (glibc) | `craft-linux-arm64` |
| Linux x64 (musl, static) | `craft-linux-amd64-musl` |
| Linux arm64 (musl, static) | `craft-linux-arm64-musl` |
| macOS Intel | `craft-darwin-amd64` |
| macOS Apple Silicon | `craft-darwin-arm64` |
| Windows x64 | `craft-windows-amd64.exe` |

To verify a download by hand:

```sh
sha256sum -c craft-linux-amd64.sha256      # or: shasum -a 256 -c …
```

## Windows

Download `craft-windows-amd64.exe` from the releases page, rename it to `craft.exe`, and put it in a directory on your `PATH`. craft needs neither administrator rights nor Developer Mode. See [Windows](./windows) for how it lays out `node_modules` there.

## From source

You need [Rust](https://www.rust-lang.org/tools/install):

```sh
git clone https://github.com/craftpkg/craft.git
cd craft
cargo install --path apps/craft
```

## Check the installation

```sh
craft --version      # or -v
craft --help
```

craft keeps its store and caches in `~/.craft`. Set `CRAFT_HOME` to put them elsewhere; see [Configuration](./configuration#environment-variables).
