#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=0c1bb9a51d177524ef95a961261137f8ec97410b
[[ $(git rev-parse HEAD) == "$base" ]]
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/source-files.sha256" ]]
mapfile -t sources < <(git diff --name-only "$base" -- crates/fe2o3-runtime crates/fe2o3-runtime-model docs/runtime-a1-a2-swarm-current.md docs/runtime-context-version-journal-async-v1.md docs/runtime-context-copy-read-leases-v1.md)
[[ ${#sources[@]} -gt 0 ]]
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- "${sources[@]}" > "$archive/source.patch"
