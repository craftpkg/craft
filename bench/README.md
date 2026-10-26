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

Mean wall time of 5 runs ± standard deviation, with the fastest in bold and how many times slower the others are. Rows were measured again, for all four, as craft changed: empty cache after pnpm's store was isolated (5.14 in `REMAINING.md`); warm cache with a lockfile and already installed after craft started linking while it imports (5.16); cold, warm cache and stale cache after craft started reading abbreviated metadata (5.9). To refresh, run `make bench` and replace these tables with its `summary.md`.

### small

An Express API server, 186 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | **1.33 s ± 120 ms** | 2.97 s ± 444 ms (2.2×) | 2.77 s ± 59 ms (2.1×) | 2.40 s ± 210 ms (1.8×) |
| Lockfile, empty cache | **1.06 s ± 74 ms** | 1.36 s ± 322 ms (1.3×) | 4.54 s ± 1.25 s (4.3×) | 2.26 s ± 381 ms (2.1×) |
| Warm cache, no lockfile | **107 ms ± 1.8 ms** | 758 ms ± 7.1 ms (7.1×) | 147 ms ± 1.7 ms (1.4×) | 1.07 s ± 109 ms (10.0×) |
| Warm cache past its max-age, no lockfile | 633 ms ± 47 ms (4.0×) | 1.89 s ± 226 ms (12.0×) | **157 ms ± 8.3 ms** | 1.01 s ± 95 ms (6.4×) |
| Warm cache and lockfile | **103 ms ± 5.0 ms** | 442 ms ± 4.9 ms (4.3×) | 128 ms ± 2.4 ms (1.2×) | 459 ms ± 5.0 ms (4.4×) |
| Already installed | **4.3 ms ± 0.2 ms** | 171 ms ± 1.1 ms (39.9×) | 35 ms ± 1.3 ms (8.2×) | 226 ms ± 0.9 ms (52.8×) |

### medium

A Vite and React single-page app, 293 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | **4.83 s ± 274 ms** | 11.28 s ± 780 ms (2.3×) | 6.13 s ± 203 ms (1.3×) | 6.36 s ± 319 ms (1.3×) |
| Lockfile, empty cache | **2.40 s ± 43 ms** | 2.78 s ± 356 ms (1.2×) | 11.86 s ± 5.82 s (4.9×) | 4.43 s ± 1.04 s (1.8×) |
| Warm cache, no lockfile | **229 ms ± 4.9 ms** | 2.28 s ± 23 ms (10.0×) | 282 ms ± 3.7 ms (1.2×) | 1.67 s ± 76 ms (7.3×) |
| Warm cache past its max-age, no lockfile | 917 ms ± 44 ms (3.4×) | 4.93 s ± 816 ms (18.4×) | **268 ms ± 3.1 ms** | 1.62 s ± 82 ms (6.1×) |
| Warm cache and lockfile | **217 ms ± 13 ms** | 946 ms ± 3.0 ms (4.4×) | 246 ms ± 8.1 ms (1.1×) | 935 ms ± 22 ms (4.3×) |
| Already installed | **4.5 ms ± 0.1 ms** | 197 ms ± 1.3 ms (43.6×) | 35 ms ± 0.8 ms (7.8×) | 299 ms ± 0.8 ms (66.4×) |

### large

A Next.js app with a design system, cloud SDKs, Storybook, Jest, Playwright and a webpack toolchain, 1391 packages.

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | **18.38 s ± 1.73 s** | 62.98 s ± 14.08 s (3.4×) | 28.82 s ± 3.42 s (1.6×) | 29.97 s ± 7.11 s (1.6×) |
| Lockfile, empty cache | 19.82 s ± 10.01 s (1.6×) | **12.03 s ± 424 ms** | 29.34 s ± 10.30 s (2.4×) | 20.08 s ± 6.06 s (1.7×) |
| Warm cache, no lockfile | **1.78 s ± 13 ms** | 12.38 s ± 236 ms (6.9×) | 1.91 s ± 66 ms (1.1×) | 9.58 s ± 138 ms (5.4×) |
| Warm cache past its max-age, no lockfile | 7.35 s ± 655 ms (4.1×) | 41.53 s ± 3.01 s (23.1×) | **1.80 s ± 9.1 ms** | 9.64 s ± 149 ms (5.4×) |
| Warm cache and lockfile | **1.72 s ± 12 ms** | 7.26 s ± 57 ms (4.2×) | 1.80 s ± 34 ms (1.0×) | 7.67 s ± 26 ms (4.5×) |
| Already installed | **5.9 ms ± 0.3 ms** | 673 ms ± 1.8 ms (113.8×) | 35 ms ± 0.6 ms (6.0×) | 900 ms ± 3.0 ms (152.2×) |

Peak memory on `large`, which hyperfine 2 records alongside the time:

| | craft | npm | pnpm | yarn |
|---|---:|---:|---:|---:|
| Cold: no cache, no lockfile | **418 MB** | 841 MB | 792 MB | 4.9 GB |
| Lockfile, empty cache | **126 MB** | 613 MB | 513 MB | 4.1 GB |
| Warm cache, no lockfile | **71 MB** | 2.9 GB | 401 MB | 2.2 GB |
| Warm cache past its max-age, no lockfile | **67 MB** | 2.6 GB | 398 MB | 2.1 GB |
| Warm cache and lockfile | **93 MB** | 2.0 GB | 219 MB | 1.7 GB |
| Already installed | **13 MB** | 254 MB | 49 MB | 334 MB |

### What it shows

- **Already installed** is craft's clear win: it answers from `node_modules/.craft/state` in about 5 ms, 6–9× faster than pnpm and 40–150× faster than npm and Yarn.
- **Warm cache with a lockfile:** craft now wins on all three. It clones each package from the store with one `clonefile` call and links while the clones run (`large`: 9.03 s → 1.72 s, pnpm 1.80 s).
- **Warm cache without a lockfile:** craft now wins on all three too. It reads the registry's abbreviated metadata and keeps it on disk indexed by version, parsing only the manifest it picks (`large`: 4.29 s → 1.78 s and 5.5 GB → 71 MB, pnpm 1.91 s).
- **Past the registry's 5-minute `max-age`**, craft and npm revalidate every packument and fall 3–23× behind pnpm, which resolves from its cache without a single request (5.17). Yarn doesn't slow down either.
- **Cold installs:** craft is fastest on all three (`large`: 18.4 s, pnpm 28.8 s, Yarn 30.0 s, npm 63.0 s), helped by abbreviated metadata being a fraction of the full packuments.
- **Lockfile, empty cache:** craft is fastest on `small` and `medium`; on `large` npm leads (12.0 s to craft's 19.8 s).
- **Memory:** craft is the leanest of the four in every scenario, peaking at 418 MB for a cold `large` install, where Yarn reaches 4.9 GB.
- **Cold and empty-cache** runs are bound by the registry and the network: their deviations reach 50%, so differences under about 1.5× there aren't meaningful.

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
