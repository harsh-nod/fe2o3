#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=ed5b5d64bf95116c21e9bf350c30132ff2bbb524
[[ $(git rev-parse HEAD) == "$base" ]]
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/source-files.sha256" ]]
scope=(crates/fe2o3-runtime-model docs/runtime-a1-a2-swarm-current.md docs/runtime-context-generated-read-leases-v1.md docs/runtime-context-read-preflight-v1.md)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
mapfile -t sources < <(git diff --name-only "$base" -- "${scope[@]}")
[[ ${#sources[@]} == 10 ]]
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- "${sources[@]}" > "$archive/source.patch"
