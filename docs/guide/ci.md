# Continuous integration

In CI you want an install that's reproducible and fails loudly when the lockfile is stale. craft does that by default when the `CI` environment variable is set, which every major CI provider does.

## Install

```sh
craft ci
```

[`craft ci`](/reference/ci) removes `node_modules` and installs exactly what `craft.lock` records. It fails, before downloading anything, when `craft.lock` is missing or doesn't match `package.json`, listing every difference. `craft install` behaves the same way under `CI`, without removing `node_modules` first.

Production images can leave out `devDependencies`:

```sh
craft install --frozen-lockfile --production
```

## Cache the store

`craft ci` needs no registry metadata, only tarballs, and finds those in the store when it's warm. Cache `~/.craft/store` (or `$CRAFT_HOME/store`) keyed on the lockfile.

## GitHub Actions

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install craft
        run: |
          curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"

      - uses: actions/cache@v4
        with:
          path: ~/.craft/store
          key: craft-${{ runner.os }}-${{ hashFiles('craft.lock') }}
          restore-keys: craft-${{ runner.os }}-

      - run: craft ci
      - run: craft test
```

## GitLab CI

```yaml
test:
  image: node:22
  variables:
    CRAFT_HOME: "$CI_PROJECT_DIR/.craft"
  cache:
    key:
      files: [craft.lock]
    paths: [.craft/store]
  before_script:
    - curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
    - export PATH="$HOME/.local/bin:$PATH"
  script:
    - craft ci
    - craft test
```

## Docker

```dockerfile
FROM node:22-slim AS deps
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
 && curl -fsSL https://raw.githubusercontent.com/craftpkg/craft/main/install.sh | bash
WORKDIR /app
COPY package.json craft.lock ./
RUN craft install --frozen-lockfile --production

FROM node:22-slim
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
CMD ["node", "server.js"]
```

On Alpine images, the install script picks craft's static musl build.

## Output

On a terminal craft shows a single live status line. In CI, when output isn't a terminal, or with `--debug`, it prints plain lines instead, and `--silent` prints only warnings and errors. Exit codes are non-zero on any failure; a failing script's own exit code passes through.
