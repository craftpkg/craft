#!/bin/sh
# Cross-compiles craft's tests for Windows and runs them under Wine, with the Windows builds of
# Node.js and Git. Arguments go to every test executable, e.g. `docker/windows/test.sh e2e::run`.
# Wine is not Windows: CI's windows-latest job is the final word.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
target=x86_64-pc-windows-gnu

docker build -q -t craft-windows-builder -f "$here/builder.Dockerfile" "$here" >/dev/null
docker build -q --platform linux/amd64 -t craft-windows-runner -f "$here/runner.Dockerfile" "$here" >/dev/null

build_log=$(mktemp)
trap 'rm -f "$build_log"' EXIT
if ! docker run --rm -v "$repo":/work -v craft-cargo-registry:/usr/local/cargo/registry \
    craft-windows-builder \
    cargo test --workspace --no-run --target "$target" --message-format=json-render-diagnostics \
    >"$build_log"; then
    exit 1
fi
executables=$(sed -n 's/.*"executable":"\([^"]*\/deps\/[^"]*\)".*/\1/p' "$build_log")

status=0
for executable in $executables; do
    echo "== ${executable##*/}"
    # Node's stdio only works under Wine when attached to a terminal, hence -t.
    docker run --rm -t --platform linux/amd64 -v "$repo":/work craft-windows-runner \
        wine "$executable" --color never "$@" || status=1
done
exit $status
