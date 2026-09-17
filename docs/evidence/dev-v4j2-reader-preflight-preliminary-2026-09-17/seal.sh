#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
names=(source-base rustc-version cargo-version source-before gnu musl rosters binaries clippy fmt no-default unsafe-policy doctests reader-self-test reader-campaign global-verus source-after binaries-after)
previous=0
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    expected=0
    [[ "$name" != global-verus ]] || expected=1
    [[ $(< "$archive/raw/$name.exit") == "$expected" ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    (( started >= previous && finished >= started ))
    previous=$finished
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
grep -Fx 'READ_PREFLIGHT_OK obligations=103 inherited=69 new=34 executable_mutations=21' "$archive/raw/reader-campaign.log"
grep -Fx 'FAIL: verification runner does not match its complete-source audit' "$archive/raw/global-verus.log"
sha256sum --check "$archive/source-files.sha256"
sha256sum --check "$archive/raw/binaries.log"
mapfile -t sources < "$archive/source-files.list"
cmp "$archive/source.patch" <(git diff --binary ed5b5d64bf95116c21e9bf350c30132ff2bbb524 -- "${sources[@]}")
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check SHA256SUMS
