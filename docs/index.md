---
layout: home

hero:
  name: craft
  text: A fast, strict package manager for Node.js
  tagline: Written in Rust. Installs from npm into an isolated node_modules, keeps a deterministic lockfile, and runs install scripts only for packages you trust.
  image:
    src: /logo.png
    alt: craft
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Why craft?
      link: /guide/introduction
    - theme: alt
      text: CLI reference
      link: /reference/cli

features:
  - icon: ⚡
    title: Fast
    details: Parallel resolution and downloads, registry metadata cached on disk and revalidated with ETags, and packages cloned from a global store. An unchanged project installs in milliseconds.
    link: /guide/benchmarks
    linkText: See the benchmarks
  - icon: 🔒
    title: Deterministic
    details: craft.lock records every resolved version, tarball and integrity hash. Frozen installs, the default in CI, install exactly that.
  - icon: 📦
    title: Strict and isolated
    details: Every package sees only the dependencies it declares, each project gets its own node_modules/.craft, and the global store is never modified after extraction.
  - icon: 🛡️
    title: Safe by default
    details: Tarballs are checked against their integrity hash, and dependencies' install scripts run only for packages listed in trustedDependencies.
  - icon: 🧩
    title: npm-compatible
    details: Semver ranges, dist-tags, npm aliases, git and tarball URLs, file and link dependencies, peer and optional dependencies, platform packages, and .npmrc registries and auth.
  - icon: 🖥️
    title: Everywhere
    details: Native binaries for Linux (glibc and musl, x64 and arm64), macOS and Windows, with no runtime dependencies.
---

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
```

Then, in a project:

```sh
craft install          # install everything in package.json
craft add react        # add a dependency
craft run build        # run a script
```

See [Installation](/guide/installation) for release binaries, Windows and building from source.

## Benchmarks

craft against npm, pnpm and Yarn on the largest project in the benchmark:

<BenchmarkChart fixture="large" :scenarios="['cache+lockfile', 'cache', 'cold', 'up-to-date']" />

Every project, scenario and peak memory, and how it's kept fair: [Benchmarks](/guide/benchmarks).
