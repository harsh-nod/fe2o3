#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
git rev-parse HEAD > "$archive/source-base.txt"
git diff --name-only HEAD -- crates/fe2o3-runtime/src | LC_ALL=C sort > "$archive/source-files.list"
git diff --binary HEAD -- crates/fe2o3-runtime/src > "$archive/source.patch"
xargs -d '\n' sha256sum < "$archive/source-files.list" > "$archive/source-files.sha256"
