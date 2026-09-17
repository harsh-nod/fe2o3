#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ $(git rev-parse HEAD) == 5ed58ea896e492f5bbc135e9f8449eabd51165bd ]]
[[ $(< "$archive/raw/benchmark-suite.exit") == 1 ]]
[[ ! -e "$archive/fixture-files.sha256" && ! -e "$archive/fixture.patch" ]]
fixtures=(benchmarks/runtime_gfx942/test_r26_host_guard.py benchmarks/runtime_gfx942/test_run_r60_pipeline.py)
git diff --quiet -- "${fixtures[@]}"
printf '%s\n' "${fixtures[@]}" > "$archive/fixture-files.list"
sha256sum -- "${fixtures[@]}" > "$archive/fixture-files.sha256"
git diff HEAD --binary -- "${fixtures[@]}" > "$archive/fixture.patch"
sha256sum "$archive/fixture-files.sha256" "$archive/fixture.patch"
