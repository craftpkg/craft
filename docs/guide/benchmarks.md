<script setup>
import benchmarks from '../.vitepress/theme/benchmarks.json'
</script>

# Benchmarks

craft, npm, pnpm and Yarn install the same three projects from six starting points, from a first install on a new machine to running `install` again in a project that is already installed. Pick a project, and switch between time and memory:

<BenchmarkChart controls />

<p class="benchmark-meta">
  Measured on {{ benchmarks.machine }} with
  craft {{ benchmarks.versions.craft }}, npm {{ benchmarks.versions.npm }},
  pnpm {{ benchmarks.versions.pnpm }}, Yarn {{ benchmarks.versions.yarn }} and
  Node.js {{ benchmarks.versions.node }}. Each bar is the mean of 5 runs.
</p>

## Highlights

- **Already installed:** craft answers in 4–6 ms from the state it keeps in `node_modules/.craft`, 6–8× faster than pnpm and 40–150× faster than npm and Yarn.
- **Warm cache:** craft is fastest on all three projects, with or without a lockfile. The large app installs in 1.72 s with a lockfile and 1.78 s without, against pnpm's 1.80 s and 1.91 s, and npm and Yarn's 7–12 s.
- **Cold:** a first install of the large app takes craft 18.4 s, pnpm 28.8 s, Yarn 30.0 s and npm 63.0 s.
- **Memory:** craft uses the least memory of the four in every scenario. The large app peaks at 71 MB with a warm cache, where pnpm needs 401 MB and npm 2.9 GB.
- **Where craft is behind:** once cached metadata is past the registry's 5-minute `max-age`, craft asks the registry whether each package changed, while pnpm trusts its cache. That costs craft 7.35 s on the large app to pnpm's 1.80 s. With a lockfile and an empty cache, npm installs the large app in 12.0 s to craft's 19.8 s.

## What each scenario starts from

| Scenario | Cache | Lockfile | `node_modules` | Like |
| --- | :-: | :-: | :-: | --- |
| Cold | | | | a first install on a new machine |
| Lockfile, empty cache | | ✓ | | CI without a cache |
| Warm cache, no lockfile | ✓ | | | adding a project on a machine that has installed before |
| Warm cache past its max-age | older than 5 minutes | | | installing again later that day |
| Warm cache and lockfile | ✓ | ✓ | | CI with a cache, or a fresh clone |
| Already installed | ✓ | ✓ | ✓ | running `install` again |

The npm registry lets clients reuse cached metadata for 5 minutes (`max-age=300`). Past that, a package manager either asks the registry again or decides for itself that its cache is good enough, which is what the "past its max-age" scenario shows.

## The projects

| Project | Packages | What it is |
| --- | --: | --- |
| `small` | 186 | An Express API server |
| `medium` | 293 | A Vite and React single-page app, with ESLint, Vitest and Tailwind |
| `large` | 1,391 | A Next.js app with MUI, Firebase and the AWS SDK, plus Storybook, Jest, Playwright, Cypress and webpack |

## Keeping it fair

- Each package manager gets its own home directory, cache and store, so none of them reuses another's downloads or your own configuration.
- Each one installs the project once before timing, and the run fails unless every direct dependency landed in `node_modules`.
- No install scripts run: npm, pnpm and Yarn get `--ignore-scripts`, and the projects trust no package in craft.
- npm skips `audit` and `fund`, and Yarn uses its `node-modules` linker, so everyone writes a `node_modules`.
- Before the warm-cache runs, each package manager is set up again right before its own runs, so its metadata stays within the registry's `max-age` throughout.
- Every install hits the real npm registry, so cold and empty-cache numbers move with the network.

## Run it yourself

The benchmark lives in [`bench/`](https://github.com/craftpkg/craft/tree/main/bench) and needs Node.js and [hyperfine](https://github.com/sharkdp/hyperfine):

```sh
make bench                                  # every project and scenario, 5 runs each
make bench -- --fixtures small --runs 3     # a quick round
```

The **Benchmark** workflow runs the same thing on a GitHub-hosted Ubuntu runner. Full tables, including the medium and small projects' memory, are in [`bench/README.md`](https://github.com/craftpkg/craft/blob/main/bench/README.md).

<style>
.benchmark-meta {
  font-size: 13px;
  color: var(--vp-c-text-2);
}
</style>
