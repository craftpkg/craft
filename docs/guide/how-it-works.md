# How it works

An install has three stages: **resolve** the dependency graph, **fetch** each package into the global store, and **link** the project's `node_modules` from the store.

## The global store

Every package is downloaded once, checked against its integrity hash while it streams to disk, and extracted into the store:

```text
~/.craft/
├── store/v1/
│   ├── express@5.3.0-78ac17c20dd7/     <name>@<version>-<hash>
│   ├── @types+node@22.10.0-c806…/      scopes use + for /
│   ├── git-1a2b3c4d5e6f/               git: repository + commit
│   └── tarball-6f5e4d3c2b1a/           tarballs, by content
├── cache/metadata/                     registry responses
└── dlx/                                craft dlx installs
```

A store entry is complete before it appears (it is extracted beside its final name and renamed into place) and is never written to afterwards, so any number of projects, and any number of concurrent installs, can share it. Set `CRAFT_HOME` to move it; `craft cache clean --force` deletes it.

## The virtual store

Each project gets its own `node_modules/.craft`, with one directory per package instance:

```text
node_modules/
├── express -> .craft/express@5.3.0/node_modules/express
├── .bin/                        bins of direct dependencies
└── .craft/
    ├── express@5.3.0/node_modules/
    │   ├── express/             files imported from the store
    │   ├── body-parser -> ../../body-parser@2.4.0/node_modules/…
    │   └── debug -> ../../debug@4.4.3/node_modules/debug
    ├── debug@4.4.3/node_modules/
    │   ├── debug/
    │   └── ms -> ../../ms@2.1.3/node_modules/ms
    ├── ms@2.1.3/node_modules/ms/
    └── node_modules/            hidden hoisting, see below
```

A package's files sit in `node_modules/<name>` inside its own entry, with its dependencies linked **beside** it. Node's module resolution walks up from the package and finds exactly those dependencies, and nothing else.

Your project's `node_modules` holds only links to its direct dependencies. If your code `require`s something `package.json` doesn't list, it fails, on your machine and in production alike.

Files are imported from the store by the fastest method the filesystem supports: a copy-on-write clone (APFS, Btrfs, XFS, ReFS), then a hard link, then a plain copy. A package's files are copied rather than linked before its install scripts run, so a build can never modify the store.

### Hidden hoisting

Some packages `require` modules they never declared. To keep them working, every package name is also linked once into `node_modules/.craft/node_modules`: the project's own dependency when it has one, otherwise the highest version. Node looks there after a package's own dependencies, while your project's root stays strict.

### Bins

`node_modules/.bin` holds the bins of your direct dependencies, which is what `craft run` and `craft <bin>` find. Each virtual store entry has its own `.bin` with the bins of that package's dependencies, so a package's scripts can call its own tools. When two direct dependencies declare the same bin, the one first in alphabetical order keeps it, and craft warns:

```text
warning: bin shared of b-tool conflicts with a-tool; keeping a-tool's
```

## Peer dependencies

A package's `peerDependencies` resolve to what its parent provides, then to the project's own dependencies, and otherwise to a version craft installs for them, as npm 7 and later do. An optional peer (`peerDependenciesMeta`) is linked only when provided.

A package used with different peers gets one instance per distinct peer set, named after them:

```text
node_modules/.craft/
├── react-redux@9.2.0_react@18.3.1/
└── react-redux@9.2.0_react@19.1.0/
```

When a peer's version is outside the declared range, craft installs anyway and warns:

```text
warning: react-redux@9.2.0 wants peer react@^18.0 || ^19 but gets 17.0.2
```

## Pruning

After every install, `add`, `update` and `remove`, craft removes what the project no longer uses: links to dependencies that left `package.json`, virtual store entries nothing reaches, and dangling bins. Directories craft didn't create, such as `node_modules/.cache`, are left alone.

## Skipping work

craft records what it installed in `node_modules/.craft/state`. When `package.json`, `craft.lock` and that record agree, `craft install` prints `Already up to date` without reading the registry, typically in a few milliseconds. `file:` directories are the exception: they are copied again on every install, since their source may have changed.

## The network

- Registry metadata (packuments) is fetched once per package name per install, cached on disk, and revalidated with `If-None-Match` / `If-Modified-Since`, honouring the registry's `max-age`.
- Requests are limited to `network-concurrency` at a time (16 by default) and retried `fetch-retries` times (2 by default) on connection errors, timeouts and HTTP 408, 429 and 5xx, with exponential backoff or the server's `Retry-After`.
- Tarballs are hashed as they stream to disk, so even a large install needs little memory.

See [Configuration](./configuration) for the settings.

For how craft does all this internally, and why it's fast, see [Anatomy](../developers/anatomy) in the developer docs.
