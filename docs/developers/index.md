# Contributing

craft is open source under the MIT License, and contributions are welcome: bug reports, documentation fixes and code.

## Getting set up

You need [Rust](https://www.rust-lang.org/tools/install) (stable) and [Node.js](https://nodejs.org) 22 or later, which the end-to-end tests use to load installed packages.

```sh
git clone https://github.com/craftpkg/craft.git
cd craft
make                       # format check, clippy, build and every test
cargo run -p craft -- install   # run your build of craft
```

`make` must pass before a change is merged; CI runs it on Linux, macOS and Windows.

## Making a change

1. Open an issue first for anything bigger than a fix, so the approach can be agreed on.
2. Write a test that fails without your change: an end-to-end test against the mock registry for anything a user can observe, a unit test for internal logic. See [Testing](./testing).
3. Keep `make` green. Clippy runs with `-D warnings`, and `unwrap` / `expect` are denied outside tests.
4. Open a pull request against `main`.

`REMAINING.md` at the repository root tracks what's planned, with the reasoning behind each task. To find your way around the code, start with [Anatomy](./anatomy), then the crate map in [Architecture](./architecture).

## Documentation

This site lives in `docs/` and is built with [VitePress](https://vitepress.dev), whose dependencies craft itself installs:

```sh
cd docs
craft install
craft run docs:dev          # http://localhost:5173/craft/
```

Every page has an "Edit this page on GitHub" link at the bottom.
