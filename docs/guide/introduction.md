# Introduction

craft is a package manager for Node.js projects, written in Rust. It reads the `package.json` you already have, installs from the npm registry (or any npm-compatible registry), and runs your scripts. You can use it in place of `npm`, `pnpm` or `yarn` for most day-to-day work.

## What craft does differently

**A strict `node_modules`.** Like pnpm, craft keeps every package in a virtual store, `node_modules/.craft`, and links each package to exactly the dependencies it declares. Your code can only `require` what your `package.json` lists, so a missing dependency fails on your machine instead of in production. See [How it works](./how-it-works).

**One copy of each package on disk.** Packages are extracted once into a global store under `~/.craft` and cloned (copy-on-write) or hard-linked into projects, so a second project with the same dependencies costs almost no disk space.

**Install scripts are opt-in.** A dependency's `preinstall`, `install` and `postinstall` scripts run only if you list the package in `trustedDependencies`. Everything else is reported, never run. See [Install scripts](./install-scripts).

**A deterministic lockfile.** `craft.lock` is sorted JSON that records every resolved version, source and integrity hash, so the same lockfile produces the same `node_modules` on every machine and platform. See [Lockfile](./lockfile).

**Clear errors.** Every error prints what failed, why, and when the fix is common, what to do about it. See [Troubleshooting](./troubleshooting).

## Compared with npm and pnpm

| | npm | pnpm | craft |
| --- | --- | --- | --- |
| Layout | flat, hoisted | isolated, symlinked | isolated, symlinked |
| Global content store | cache only | yes | yes |
| Dependencies' install scripts | run by default | opt-in since v10 | opt-in via `trustedDependencies` |
| Lockfile | `package-lock.json` | `pnpm-lock.yaml` | `craft.lock` |
| Workspaces | yes | yes | not yet |
| `overrides` / `resolutions` | yes | yes | not yet |
| Written in | JavaScript | JavaScript | Rust |

## Status

craft handles real projects today: Express servers, React with Redux and React Query, Vite and TypeScript builds, native packages such as esbuild, SWC, Lightning CSS and bcrypt, and git, tarball, JSR and local dependencies. These scenarios run against the real npm registry in craft's test suite.

Not supported yet: workspaces and the `workspace:` protocol, `overrides` / `resolutions`, importing `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml`, global installs, `craft publish` and `craft audit`. See [Migrating](./migrating) for what that means for an existing project.
