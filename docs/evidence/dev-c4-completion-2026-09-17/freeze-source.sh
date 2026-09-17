#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/source-base.txt" && ! -e "$archive/SHA256SUMS" ]]
base=$(git rev-parse HEAD)
[[ $base == 13c5e5b8126dd461c5d7eb8972613d3ff6f4019b ]]
[[ -z $(git ls-files --others --exclude-standard -- crates scripts) ]]
git diff --cached --binary "$base" -- crates scripts > "$archive/prerequisite.patch"
[[ $(sha256sum "$archive/prerequisite.patch" | cut -d ' ' -f 1) == fd3f82ee6330210ec91157acfdbd9815ef913e1e0378fcbef7337a850be14c3b ]]
mapfile -t sources < <(git diff --name-only "$base" -- crates scripts)
[[ ${#sources[@]} == 38 ]]
printf '%s\n' "$base" > "$archive/source-base.txt"
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- crates scripts > "$archive/source.patch"
git diff --binary -- crates scripts > "$archive/integration.patch"
sha256sum "$archive"/*.patch "$archive/source-files.sha256"
