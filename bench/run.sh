#!/usr/bin/env bash
# Races craft against npm, pnpm and Yarn on the fixtures in bench/fixtures.
set -euo pipefail

BENCH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$BENCH")"
CRAFT="${CRAFT:-$ROOT/target/release/craft}"
TOOLS="$BENCH/tools/node_modules/.bin"
TEMP="${TMPDIR:-/tmp}"
WORK="${BENCH_WORK:-${TEMP%/}/craft-bench}"
OUT="$BENCH/results/$(date +%Y%m%d-%H%M%S)"
FIXTURES="small,medium,large"
SCENARIOS="cold,lockfile,cache,cache+lockfile,up-to-date,stale"
PMS="craft,npm,pnpm,yarn"
RUNS=5

usage() {
  cat <<EOF
Usage: bench/run.sh [options]

  --fixtures LIST   fixtures to install (default: $FIXTURES)
  --scenarios LIST  scenarios to time (default: $SCENARIOS)
  --pms LIST        package managers to race (default: $PMS)
  --runs N          timed runs per scenario (default: $RUNS)
  --out DIR         where results are written (default: bench/results/<date-time>)

Scenarios say what an install starts from:
  cold            nothing: no cache, no lockfile, no node_modules
  lockfile        a lockfile, with an empty cache
  cache           a warm cache, without a lockfile
  cache+lockfile  a warm cache and a lockfile
  up-to-date      everything already installed
  stale           the cache as setup left it, past the registry's max-age, without a lockfile

Environment: CRAFT (craft binary, default target/release/craft),
BENCH_WORK (scratch directory, default \$TMPDIR/craft-bench).
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --fixtures) FIXTURES="$2"; shift 2 ;;
    --scenarios) SCENARIOS="$2"; shift 2 ;;
    --pms) PMS="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

# Every package manager sees a plain, non-CI terminal session, so none of them
# switches to frozen-lockfile or immutable installs.
export CI=false

# Package manager settings from the caller's environment would reach past the
# per-package-manager HOME, as PNPM_HOME does with pnpm's store.
while read -r name; do
  unset "$name"
done < <(env | grep -ioE '^(npm_config_|pnpm_|yarn_|corepack_)[a-z0-9_]*=' | tr -d =)

# How long the registry lets a cached packument be used without asking again.
MAX_AGE=300

require() {
  command -v "$1" >/dev/null || { echo "error: $1 is not installed${2:+ ($2)}" >&2; exit 1; }
}

lockfile_of() {
  case "$1" in
    craft) echo craft.lock ;;
    npm) echo package-lock.json ;;
    pnpm) echo pnpm-lock.yaml ;;
    yarn) echo yarn.lock ;;
  esac
}

# The install command of a package manager. Each one gets its own HOME, so its
# cache, store and config start from nothing and are cleared by deleting it.
install_command() {
  local pm="$1" home="$2"
  local env="HOME='$home' XDG_CACHE_HOME='$home/.cache' XDG_DATA_HOME='$home/.local/share' XDG_STATE_HOME='$home/.local/state' XDG_CONFIG_HOME='$home/.config'"
  case "$pm" in
    craft) echo "$env CRAFT_HOME='$home/.craft' '$CRAFT' install" ;;
    npm) echo "$env npm_config_cache='$home/.npm' '$TOOLS/npm' install --ignore-scripts --no-audit --no-fund --no-update-notifier" ;;
    pnpm) echo "$env PNPM_HOME='$home/.pnpm' '$TOOLS/pnpm' install --ignore-scripts" ;;
    yarn) echo "$env YARN_NODE_LINKER=node-modules YARN_ENABLE_SCRIPTS=false YARN_ENABLE_TELEMETRY=false '$TOOLS/yarn' install" ;;
  esac
}

# What a scenario removes before every run. Yarn keeps its install state in
# .yarn rather than node_modules, so it goes with node_modules.
prepare_command() {
  local scenario="$1" project="$2" home="$3" lockfile="$4" saved="$5"
  local modules="rm -rf '$project/node_modules' '$project/.yarn'"
  local restore="cp '$saved' '$project/$lockfile'"
  case "$scenario" in
    cold) echo "$modules '$project/$lockfile' '$home'" ;;
    lockfile) echo "$modules '$home' && $restore" ;;
    cache) echo "$modules '$project/$lockfile'" ;;
    cache+lockfile) echo "$modules && $restore" ;;
    up-to-date) echo "true" ;;
    stale) echo "$modules '$project/$lockfile' '$home' && $(copy_tree "$home.setup" "$home")" ;;
  esac
}

needs_warmup() {
  case "$1" in
    cold|lockfile) return 1 ;;
    *) return 0 ;;
  esac
}

# Copies a directory keeping its timestamps, cloning files where the filesystem can.
copy_tree() {
  local source="$1" dest="$2"
  case "$(uname)" in
    Darwin) echo "cp -cpR '$source' '$dest'" ;;
    *) echo "cp -a --reflink=auto '$source' '$dest'" ;;
  esac
}

has_scenario() {
  [[ ",$SCENARIOS," == *",$1,"* ]]
}

# Waits until every cache setup wrote is older than the registry's max-age, so a
# package manager that honours it has to ask the registry again.
wait_for_stale_caches() {
  local fixture="$1"
  local ready=$(($(cat "$WORK/$fixture/setup-done") + MAX_AGE + 1))
  local now
  now="$(date +%s)"
  if [ "$now" -lt "$ready" ]; then
    echo "Waiting $((ready - now)) s for the caches to pass the registry's max-age"
    sleep $((ready - now))
  fi
}

# Installs a fixture once, keeps the lockfile it wrote, and checks every direct
# dependency landed, so a package manager that silently did nothing can't win.
setup() {
  local fixture="$1" pm="$2"
  local dir="$WORK/$fixture/$pm" lockfile
  lockfile="$(lockfile_of "$pm")"
  rm -rf "$dir"
  mkdir -p "$dir/project"
  cp "$BENCH/fixtures/$fixture/package.json" "$dir/project/"
  if ! (cd "$dir/project" && eval "$(install_command "$pm" "$dir/home")") >"$dir/setup.log" 2>&1; then
    echo "error: $pm failed to install $fixture, see $dir/setup.log" >&2
    tail -20 "$dir/setup.log" >&2
    exit 1
  fi
  cp "$dir/project/$lockfile" "$dir/lockfile"
  mkdir -p "$OUT/$fixture"
  cp "$dir/lockfile" "$OUT/$fixture/$lockfile"
  node "$BENCH/verify.mjs" "$dir/project"
  if has_scenario stale; then
    eval "$(copy_tree "$dir/home" "$dir/home.setup")"
  fi
}

require node
require hyperfine "brew install hyperfine, or see https://github.com/sharkdp/hyperfine"
[ -x "$CRAFT" ] || { echo "error: no craft binary at $CRAFT; run cargo build --release -p craft" >&2; exit 1; }
if [ ! -x "$TOOLS/pnpm" ]; then
  echo "Installing the pinned package managers into bench/tools"
  (cd "$BENCH/tools" && "$CRAFT" install --frozen-lockfile --silent)
fi

IFS=, read -r -a fixtures <<<"$FIXTURES"
IFS=, read -r -a scenarios <<<"$SCENARIOS"
IFS=, read -r -a pms <<<"$PMS"
for pm in "${pms[@]}"; do
  [ -n "$(lockfile_of "$pm")" ] || { echo "error: no package manager named $pm" >&2; exit 2; }
done
for scenario in "${scenarios[@]}"; do
  [ -n "$(prepare_command "$scenario" - - - -)" ] || { echo "error: no scenario named $scenario" >&2; exit 2; }
done
for fixture in "${fixtures[@]}"; do
  [ -f "$BENCH/fixtures/$fixture/package.json" ] || { echo "error: no fixture named $fixture" >&2; exit 2; }
done

mkdir -p "$OUT"
{
  for pm in "${pms[@]}"; do
    case "$pm" in
      craft) echo "craft $("$CRAFT" --version | awk '{print $NF}')" ;;
      *) echo "$pm $("$TOOLS/$pm" --version)" ;;
    esac
  done
  echo "node $(node --version | tr -d v)"
} >"$OUT/versions.txt"

for fixture in "${fixtures[@]}"; do
  for pm in "${pms[@]}"; do
    echo "Setting up $fixture with $pm"
    setup "$fixture" "$pm"
  done
  date +%s >"$WORK/$fixture/setup-done"

  for scenario in "${scenarios[@]}"; do
    [ "$scenario" = stale ] && wait_for_stale_caches "$fixture"
    args=(--shell=bash --runs "$RUNS" --export-json "$OUT/$fixture/$scenario.json" --export-markdown "$OUT/$fixture/$scenario.md")
    needs_warmup "$scenario" && args+=(--warmup 1)
    for pm in "${pms[@]}"; do
      dir="$WORK/$fixture/$pm"
      project="$dir/project"
      prepare="$(prepare_command "$scenario" "$project" "$dir/home" "$(lockfile_of "$pm")" "$dir/lockfile")"
      args+=(--prepare "$prepare" --command-name "$pm" "cd '$project' && $(install_command "$pm" "$dir/home")")
    done
    echo
    echo "=== $fixture: $scenario"
    hyperfine "${args[@]}"
  done
done

node "$BENCH/report.mjs" "$OUT" | tee "$OUT/summary.md"
