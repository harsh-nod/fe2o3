#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=0a58128b74282aaae59194d6edb91aa424b814e5
[[ $(git rev-parse HEAD) == "$base" ]]
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/source-files.sha256" ]]
mapfile -t sources < <(git diff --name-only "$base" -- crates/fe2o3-runtime docs/runtime-a1-a2-swarm-current.md docs/runtime-context-version-journal-async-v1.md docs/runtime-context-version-journal-disposal-v1.md)
[[ ${#sources[@]} -gt 0 ]]
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- "${sources[@]}" > "$archive/source.patch"
