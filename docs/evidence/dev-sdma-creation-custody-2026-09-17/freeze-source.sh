#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=5db1450574a12053d46d890a0bfdc2a8c4f17e9c
test "$(git rev-parse HEAD)" = "$base"
git diff --quiet -- crates/fe2o3-kfd/src
for name in source-base.txt source.patch source-files.sha256; do
    test ! -e "$archive/$name"
done
printf '%s\n' "$base" > "$archive/source-base.txt"
git diff "$base" --binary -- crates/fe2o3-kfd/src > "$archive/source.patch"
git diff "$base" --name-only -z -- crates/fe2o3-kfd/src |
    LC_ALL=C sort -z | xargs -0 sha256sum > "$archive/source-files.sha256"
test "$(wc -l < "$archive/source-files.sha256")" = 8
sha256sum --check "$archive/source-files.sha256"
