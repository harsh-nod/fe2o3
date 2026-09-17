#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=ace978213dd4e5e1ff10cb00d875bf24581aae83
[[ $(git rev-parse HEAD) == "$base" ]]
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/source-files.sha256" ]]
scope=(crates/fe2o3-runtime crates/fe2o3-runtime-model crates/fe2o3-host docs/runtime-a1-a2-swarm-current.md docs/runtime-context-version-journal-generated-v1.md docs/runtime-context-generated-read-leases-v1.md)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
mapfile -t sources < <(git diff --name-only "$base" -- "${scope[@]}")
[[ ${#sources[@]} -gt 0 ]]
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- "${sources[@]}" > "$archive/source.patch"
