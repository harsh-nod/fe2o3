#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/source-base.txt" && ! -e "$archive/SHA256SUMS" ]]
git rev-parse HEAD > "$archive/source-base.txt"
git diff --cached --name-only -- crates > "$archive/source-files.list"
mapfile -t sources < "$archive/source-files.list"
[[ ${#sources[@]} == 10 ]]
sha256sum "${sources[@]}" > "$archive/source-files.sha256"
git diff --binary HEAD -- "${sources[@]}" > "$archive/source.patch"
