# Migrating from npm or pnpm

craft works with the `package.json` you have. To try it on a project:

```sh
rm -rf node_modules
craft install
craft test
```

The first install resolves from `package.json` and writes `craft.lock`. craft doesn't read `package-lock.json`, `yarn.lock` or `pnpm-lock.yaml`, so versions within your ranges may differ from what those lockfiles pinned. Once it works, commit `craft.lock` and delete the old lockfile.

## Command equivalents

| npm | pnpm | craft |
| --- | --- | --- |
| `npm install` | `pnpm install` | `craft install` |
| `npm ci` | `pnpm install --frozen-lockfile` | `craft ci` |
| `npm install react` | `pnpm add react` | `craft add react` |
| `npm install -D vitest` | `pnpm add -D vitest` | `craft add -D vitest` |
| `npm uninstall react` | `pnpm remove react` | `craft remove react` |
| `npm update` | `pnpm update` | `craft update` |
| `npm outdated` | `pnpm outdated` | `craft outdated` |
| `npm ls` | `pnpm list` | `craft list` |
| `npm explain ms` | `pnpm why ms` | `craft why ms` |
| `npm run build` | `pnpm build` | `craft run build`, or `craft build` |
| `npx tsc` | `pnpm exec tsc` | `craft exec tsc`, or `craft tsc` |
| `npx create-vite` | `pnpm dlx create-vite` | `craft dlx create-vite` |
| `npm init -y` | `pnpm init` | `craft init` |
| `npm cache clean --force` | `pnpm store prune` | `craft cache clean --force` |

## What changes

**Undeclared dependencies stop resolving.** npm hoists every package to the top of `node_modules`, so code can accidentally `require` a dependency of a dependency. craft's root `node_modules` holds only what `package.json` lists, as with pnpm. If something breaks with `Cannot find module`, add the missing package to `package.json`. Dependencies that do this themselves keep working through [hidden hoisting](./how-it-works#hidden-hoisting).

**Dependencies' install scripts don't run until you trust them.** craft lists the packages it skipped; most work anyway, and the ones that need to compile, such as `bcrypt` or `sqlite3`, need `craft trust <name>`. See [Install scripts](./install-scripts). pnpm users can move the names from `onlyBuiltDependencies` to `trustedDependencies`.

**`.npmrc` keeps working** for registries, auth, proxies and certificates. See [Configuration](./configuration).

## Not supported yet

- **Workspaces** (`workspaces` in `package.json`, `pnpm-workspace.yaml`, `workspace:` specifiers). A monorepo can't use craft yet.
- **`overrides` / `resolutions` / `pnpm.overrides`** are ignored.
- **Importing an existing lockfile.**
- **Global installs** (`npm install -g`). Use [`craft dlx`](/reference/dlx) to run tools.
- **`publish`, `pack`, `audit`, `link`.** Keep npm around for publishing.
