# Benchmarks

Races `craft install` against npm, pnpm and Yarn on real projects, timed with [hyperfine](https://github.com/sharkdp/hyperfine).

```sh
make bench                                    # every fixture and scenario, 5 runs each
make bench -- --fixtures small --runs 3       # a quick round
bench/run.sh --pms craft,pnpm --scenarios cold,up-to-date
```

It needs Node.js and hyperfine (`brew install hyperfine`, `apt install hyperfine`). The package managers it races are pinned in `tools/package.json` and installed there on the first run. Results go to `bench/results/<date-time>/`: a `summary.md` table, hyperfine's JSON and Markdown for each scenario, and the lockfile each package manager wrote.

The **Benchmark** workflow runs it on a GitHub-hosted Ubuntu runner (Actions → Benchmark → Run workflow), puts the table in the run's summary and uploads the results.

## Results

2026-10-25 · macOS, Apple M5 Max, 18 cores, 48 GB · `craft 0.1.0` · `npm 12.2.1` · `pnpm 12.15.0` · `yarn 4.18.1` · `node 24.18.0`

Mean wall time of 5 runs ± standard deviation, with the fastest in bold and how many times slower the others are. To refresh, run `make bench` and replace these tables with its `summary.md`.

### small

An Express API server, 186 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 3.38 s ± 1.05 s (1.4×) | 4.98 s ± 1.45 s (2.0×) | **2.46 s ± 391 ms** | 2.62 s ± 213 ms (1.1×) |
| Lockfile, empty cache | 1.08 s ± 66 ms (1.0×) | **1.06 s ± 58 ms** | 3.53 s ± 677 ms (3.3×) | 2.08 s ± 348 ms (2.0×) |
| Warm cache, no lockfile | 477 ms ± 14 ms (2.8×) | 744 ms ± 7.8 ms (4.4×) | **168 ms ± 2.9 ms** | 1.12 s ± 139 ms (6.7×) |
| Warm cache and lockfile | 286 ms ± 3.2 ms (1.9×) | 448 ms ± 20 ms (3.0×) | **149 ms ± 11 ms** | 454 ms ± 6.4 ms (3.0×) |
| Already installed | **4.3 ms ± 0.1 ms** | 177 ms ± 4.6 ms (41.1×) | 35 ms ± 0.4 ms (8.0×) | 230 ms ± 4.4 ms (53.6×) |

### medium

A Vite and React single-page app, 293 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 6.33 s ± 202 ms (1.1×) | 16.47 s ± 2.98 s (2.9×) | **5.69 s ± 409 ms** | 6.58 s ± 300 ms (1.2×) |
| Lockfile, empty cache | 4.52 s ± 1.54 s (1.1×) | 5.28 s ± 1.37 s (1.3×) | 5.99 s ± 2.84 s (1.5×) | **4.11 s ± 553 ms** |
| Warm cache, no lockfile | 1.80 s ± 8.4 ms (5.5×) | 2.36 s ± 74 ms (7.2×) | **330 ms ± 6.9 ms** | 1.71 s ± 78 ms (5.2×) |
| Warm cache and lockfile | 756 ms ± 8.5 ms (2.8×) | 999 ms ± 46 ms (3.8×) | **265 ms ± 13 ms** | 938 ms ± 12 ms (3.5×) |
| Already installed | **4.6 ms ± 0.2 ms** | 206 ms ± 14 ms (44.6×) | 40 ms ± 0.7 ms (8.7×) | 307 ms ± 9.0 ms (66.7×) |

### large

A Next.js app with a design system, cloud SDKs, Storybook, Jest, Playwright and a webpack toolchain, 1391 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 37.26 s ± 10.64 s (2.6×) | 67.34 s ± 2.78 s (4.7×) | **14.26 s ± 1.96 s** | 37.10 s ± 12.61 s (2.6×) |
| Lockfile, empty cache | 22.46 s ± 8.38 s (1.7×) | **13.55 s ± 3.54 s** | 49.60 s ± 46.33 s (3.7×) | 33.44 s ± 11.39 s (2.5×) |
| Warm cache, no lockfile | 12.78 s ± 204 ms (6.3×) | 12.07 s ± 51 ms (5.9×) | **2.03 s ± 22 ms** | 9.89 s ± 161 ms (4.9×) |
| Warm cache and lockfile | 9.03 s ± 50 ms (4.9×) | 7.30 s ± 19 ms (4.0×) | **1.85 s ± 21 ms** | 7.62 s ± 102 ms (4.1×) |
| Already installed | **6.5 ms ± 0.4 ms** | 682 ms ± 15 ms (104.3×) | 38 ms ± 3.6 ms (5.7×) | 893 ms ± 1.9 ms (136.7×) |

Peak memory on `large`, which hyperfine 2 records alongside the time:

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 5.5 GB | 851 MB | **663 MB** | 4.7 GB |
| Lockfile, empty cache | **126 MB** | 616 MB | 590 MB | 3.8 GB |
| Warm cache, no lockfile | 5.5 GB | 2.9 GB | **364 MB** | 2.1 GB |
| Warm cache and lockfile | **116 MB** | 2.0 GB | 184 MB | 1.7 GB |
| Already installed | **13 MB** | 243 MB | 49 MB | 333 MB |

### What it shows

- **Already installed** is craft's clear win: it answers from `node_modules/.craft/state` in about 5 ms, 6–9× faster than pnpm and 40–140× faster than npm and Yarn.
- **Warm cache** is craft's weakest spot: 2–6× slower than pnpm, and close to npm and Yarn. With a lockfile there is no resolution, so that time goes to importing files from the store into `node_modules`.
- **Resolving without a lockfile** costs craft the most memory, growing with the graph to 5.5 GB on `large`. With a lockfile it stays the leanest of the four.
- **Cold and empty-cache** runs are bound by the registry and the network: their deviations reach 30–90%, so differences under about 1.5× there aren't meaningful.

## Fixtures

| | |
|---|---|
| `small` | An Express API server |
| `medium` | A Vite and React single-page app, with ESLint, Vitest and Tailwind |
| `large` | A Next.js app with MUI, Firebase and the AWS SDK, plus Storybook, Jest, Playwright, Cypress and webpack |

A fixture is a directory holding a `package.json`; add one and pass its name to `--fixtures`. Every package manager must be able to install it without flags such as `--legacy-peer-deps`.

## Scenarios

| | Cache | Lockfile | node_modules | Like |
|---|:-:|:-:|:-:|---|
| `cold` | | | | a first install on a new machine |
| `lockfile` | | ✓ | | CI without a cache |
| `cache` | ✓ | | | adding a project on a machine that has installed before |
| `cache+lockfile` | ✓ | ✓ | | CI with a cache, or a fresh clone |
| `up-to-date` | ✓ | ✓ | ✓ | running install again |

## Keeping it fair

- Each package manager gets its own `HOME` (and XDG directories), so its cache, store and config start empty, and your own `.npmrc` or `.yarnrc.yml` don't apply. Deleting that home is what empties its cache.
- Each one installs the fixture once before timing, and the run fails unless every direct dependency is in `node_modules`. The lockfile from that install is restored before every run that has one.
- No install scripts run: npm, pnpm and Yarn get `--ignore-scripts` (Yarn `enableScripts: false`), and craft runs none because the fixtures trust no package.
- npm skips `audit` and `fund`, which are extra requests after the install. Yarn uses the `node-modules` linker, so everyone writes a `node_modules`.
- `CI=false` is set, so no one switches to frozen or immutable installs.
- Everything else is each package manager's default. Every install hits the real npm registry, so cold and lockfile numbers move with the network; compare them within one run, not across runs.
