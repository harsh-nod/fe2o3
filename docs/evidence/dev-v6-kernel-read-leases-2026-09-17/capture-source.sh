#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=13ef09c9ab79ecf2278c0fe99c8266279a0fda04
[[ $(git rev-parse HEAD) == "$base" ]]
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/source-files.sha256" ]]
scope=(crates/fe2o3-runtime crates/fe2o3-runtime-model crates/fe2o3-host docs/runtime-a1-a2-swarm-current.md docs/runtime-context-version-journal-async-v1.md docs/runtime-context-copy-read-leases-v1.md docs/runtime-context-kernel-read-leases-v1.md)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
mapfile -t sources < <(git diff --name-only "$base" -- "${scope[@]}")
[[ ${#sources[@]} -gt 0 ]]
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- "${sources[@]}" > "$archive/source.patch"
