#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/source-base.txt" && ! -e "$archive/SHA256SUMS" ]]
base=c29f4f46aed32a0317d5d1cfd39dfdbaa495cf57
git cat-file -e "$base^{commit}"
[[ -z $(git ls-files --others --exclude-standard -- crates scripts) ]]
git diff --check "$base" -- crates scripts
mapfile -t sources < <(git diff --name-only "$base" -- crates scripts)
[[ ${#sources[@]} == 4 ]]
printf '%s\n' "$base" > "$archive/source-base.txt"
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary "$base" -- crates scripts > "$archive/source.patch"
sha256sum "$archive/source.patch" "$archive/source-files.sha256"
