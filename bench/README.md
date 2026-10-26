# Benchmarks

Races `craft install` against npm, pnpm and Yarn on real projects, timed with [hyperfine](https://github.com/sharkdp/hyperfine).

```sh
make bench                                    # every fixture and scenario, 5 runs each
make bench -- --fixtures small --runs 3       # a quick round
bench/run.sh --pms craft,pnpm --scenarios cold,up-to-date
```

It needs Node.js and hyperfine (`brew install hyperfine`, `apt install hyperfine`). The package managers it races are pinned in `tools/package.json` and installed there on the first run. Results go to `bench/results/<date-time>/`: a `summary.md` table, hyperfine's JSON for each scenario, and the lockfile each package manager wrote.

The **Benchmark** workflow runs it on a GitHub-hosted Ubuntu runner (Actions → Benchmark → Run workflow), puts the table in the run's summary and uploads the results.

## Results

2026-10-25 and 2026-10-26 · macOS, Apple M5 Max, 18 cores, 48 GB · `craft 0.1.0` · `npm 12.2.1` · `pnpm 12.15.0` · `yarn 4.18.1` · `node 24.18.0`

Mean wall time of 5 runs ± standard deviation, with the fastest in bold and how many times slower the others are. Rows were measured again, for all four, as craft changed: warm cache with a lockfile and already installed after craft started cloning whole packages (5.8 in `REMAINING.md`); cold and empty cache after pnpm's store was isolated (5.14); warm and stale cache without a lockfile after craft stopped freeing its metadata at exit (5.15). To refresh, run `make bench` and replace these tables with its `summary.md`.

### small

An Express API server, 186 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | **1.78 s ± 353 ms** | 4.76 s ± 256 ms (2.7×) | 4.46 s ± 1.41 s (2.5×) | 3.09 s ± 171 ms (1.7×) |
| Lockfile, empty cache | **1.06 s ± 74 ms** | 1.36 s ± 322 ms (1.3×) | 4.54 s ± 1.25 s (4.3×) | 2.26 s ± 381 ms (2.1×) |
| Warm cache, no lockfile | 263 ms ± 2.8 ms (1.8×) | 742 ms ± 3.8 ms (5.1×) | **147 ms ± 1.5 ms** | 1.03 s ± 103 ms (7.1×) |
| Warm cache past its max-age, no lockfile | 856 ms ± 78 ms (5.6×) | 2.31 s ± 269 ms (15.0×) | **154 ms ± 1.9 ms** | 1.07 s ± 90 ms (7.0×) |
| Warm cache and lockfile | **135 ms ± 7.8 ms** | 488 ms ± 17 ms (3.6×) | 212 ms ± 48 ms (1.6×) | 490 ms ± 16 ms (3.6×) |
| Already installed | **4.5 ms ± 0.1 ms** | 177 ms ± 1.6 ms (39.2×) | 35 ms ± 1.0 ms (7.8×) | 227 ms ± 3.4 ms (50.1×) |

### medium

A Vite and React single-page app, 293 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 7.93 s ± 3.77 s (1.1×) | 16.95 s ± 3.22 s (2.4×) | **6.95 s ± 491 ms** | 7.23 s ± 408 ms (1.0×) |
| Lockfile, empty cache | **2.40 s ± 43 ms** | 2.78 s ± 356 ms (1.2×) | 11.86 s ± 5.82 s (4.9×) | 4.43 s ± 1.04 s (1.8×) |
| Warm cache, no lockfile | 1.02 s ± 6.1 ms (3.6×) | 2.26 s ± 7.3 ms (7.9×) | **287 ms ± 4.1 ms** | 1.65 s ± 176 ms (5.8×) |
| Warm cache past its max-age, no lockfile | 1.61 s ± 122 ms (5.9×) | 5.33 s ± 847 ms (19.6×) | **271 ms ± 1.9 ms** | 1.73 s ± 114 ms (6.4×) |
| Warm cache and lockfile | **254 ms ± 3.0 ms** | 931 ms ± 7.6 ms (3.7×) | 271 ms ± 5.8 ms (1.1×) | 917 ms ± 4.5 ms (3.6×) |
| Already installed | **4.6 ms ± 0.1 ms** | 196 ms ± 4.1 ms (42.2×) | 34 ms ± 0.7 ms (7.4×) | 302 ms ± 1.4 ms (65.1×) |

### large

A Next.js app with a design system, cloud SDKs, Storybook, Jest, Playwright and a webpack toolchain, 1391 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 31.08 s ± 8.47 s (1.0×) | 63.84 s ± 3.85 s (2.1×) | 43.65 s ± 13.47 s (1.5×) | **29.80 s ± 2.81 s** |
| Lockfile, empty cache | 19.82 s ± 10.01 s (1.6×) | **12.03 s ± 424 ms** | 29.34 s ± 10.30 s (2.4×) | 20.08 s ± 6.06 s (1.7×) |
| Warm cache, no lockfile | 4.47 s ± 15 ms (2.4×) | 12.03 s ± 58 ms (6.5×) | **1.86 s ± 25 ms** | 9.56 s ± 207 ms (5.1×) |
| Warm cache past its max-age, no lockfile | 8.50 s ± 167 ms (4.7×) | 36.97 s ± 3.32 s (20.5×) | **1.81 s ± 13 ms** | 9.52 s ± 153 ms (5.3×) |
| Warm cache and lockfile | 1.97 s ± 10 ms (1.1×) | 7.22 s ± 25 ms (3.9×) | **1.85 s ± 25 ms** | 7.65 s ± 45 ms (4.1×) |
| Already installed | **5.9 ms ± 0.1 ms** | 677 ms ± 2.2 ms (114.5×) | 35 ms ± 0.7 ms (5.9×) | 900 ms ± 5.8 ms (152.2×) |

Peak memory on `large`, which hyperfine 2 records alongside the time:

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | 5.5 GB | 826 MB | **804 MB** | 4.8 GB |
| Lockfile, empty cache | **126 MB** | 613 MB | 513 MB | 4.1 GB |
| Warm cache, no lockfile | 5.5 GB | 2.9 GB | **401 MB** | 2.2 GB |
| Warm cache past its max-age, no lockfile | 5.5 GB | 2.6 GB | **399 MB** | 2.1 GB |
| Warm cache and lockfile | **115 MB** | 2.0 GB | 185 MB | 1.7 GB |
| Already installed | **13 MB** | 250 MB | 49 MB | 333 MB |

### What it shows

- **Already installed** is craft's clear win: it answers from `node_modules/.craft/state` in about 5 ms, 6–9× faster than pnpm and 40–150× faster than npm and Yarn.
- **Warm cache with a lockfile:** craft now wins on `small` and `medium` and is within 7% of pnpm on `large`, since it clones each package from the store with one `clonefile` call (`large`: 9.03 s → 1.97 s).
- **Warm cache without a lockfile** is still 1.8–3.6× behind pnpm: resolution parses full packuments (5.9 in `REMAINING.md`).
- **Past the registry's 5-minute `max-age`**, craft and npm revalidate every packument and fall 5–21× behind pnpm, which resolves from its cache without a single request (5.17). Yarn doesn't slow down either.
- **Cold and empty-cache installs:** craft is fastest on `small` and with a lockfile on `medium`, and level with Yarn on `large`. pnpm's earlier lead there came from the benchmark reaching its existing store through `PNPM_HOME`.
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
| `stale` | older than `max-age` | | | installing again later that day |

Before `cache`, each package manager is set up again right before its own runs, so its metadata stays within the registry's `max-age` throughout. `stale` restores each package manager's home as setup left it, waiting first until the registry's 5-minute `max-age` has passed, so each one decides for itself whether its cached metadata is still good enough.

## Keeping it fair

- Each package manager gets its own `HOME` (and XDG directories, and pnpm its `PNPM_HOME`), so its cache, store and config start empty, and your own `.npmrc` or `.yarnrc.yml` don't apply. Deleting that home is what empties its cache. `npm_config_*`, `PNPM_*`, `YARN_*` and `COREPACK_*` variables from your shell are unset.
- Each one installs the fixture once before timing, and the run fails unless every direct dependency is in `node_modules`. The lockfile from that install is restored before every run that has one.
- No install scripts run: npm, pnpm and Yarn get `--ignore-scripts` (Yarn `enableScripts: false`), and craft runs none because the fixtures trust no package.
- npm skips `audit` and `fund`, which are extra requests after the install. Yarn uses the `node-modules` linker, so everyone writes a `node_modules`.
- `CI=false` is set, so no one switches to frozen or immutable installs.
- Everything else is each package manager's default. Every install hits the real npm registry, so cold and lockfile numbers move with the network; compare them within one run, not across runs.
