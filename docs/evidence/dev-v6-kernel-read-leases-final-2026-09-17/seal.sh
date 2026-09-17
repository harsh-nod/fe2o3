#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
names=(source-base rustc-version cargo-version source-before parser-policy gnu musl rosters binaries clippy fmt no-default unsafe-policy doctests source-after binaries-after)
previous=0
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    (( started >= previous && finished >= started ))
    previous=$finished
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
sha256sum --check "$archive/source-files.sha256"
sha256sum --check "$archive/raw/binaries.log"
mapfile -t sources < "$archive/source-files.list"
scope=(crates/fe2o3-runtime crates/fe2o3-runtime-model crates/fe2o3-host docs/runtime-a1-a2-swarm-current.md docs/runtime-context-version-journal-async-v1.md docs/runtime-context-copy-read-leases-v1.md docs/runtime-context-kernel-read-leases-v1.md)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
cmp "$archive/source-files.list" <(git diff --name-only 13ef09c9ab79ecf2278c0fe99c8266279a0fda04 -- "${scope[@]}")
cmp "$archive/source.patch" <(git diff --binary 13ef09c9ab79ecf2278c0fe99c8266279a0fda04 -- "${sources[@]}")
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check SHA256SUMS
