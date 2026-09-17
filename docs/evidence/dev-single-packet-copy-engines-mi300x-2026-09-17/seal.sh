#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
names=(allocate prepare benchmark collect summary-self-test summary cleanup summary-self-test-corrected summary-checked occupancy-after-cleanup)
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    expected=0
    [[ "$name" != summary-self-test ]] || expected=1
    [[ $(< "$archive/raw/$name.exit") == "$expected" ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    ((finished >= started))
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
previous=0
for name in allocate prepare benchmark collect cleanup occupancy-after-cleanup; do
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    ((started >= previous))
    previous=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
done
cmp "$archive/results/source-before.log" "$archive/results/source-after.log"
[[ $(wc -l < "$archive/results/source-files.sha256") == 5434 ]]
[[ $(wc -l < "$archive/results/binaries.sha256") == 2 ]]
cmp "$archive/raw/summary.log" "$archive/raw/summary-checked.log"
cmp <(python3 -I "$archive/summarize.py") "$archive/raw/summary-checked.log"
python3 -I "$archive/summarize.py" --self-test
grep -Fx 'removed_owned_path=/home/harsh/fe2o3-engine-diagnostic-20260917.vNTF2iSX' "$archive/raw/cleanup.log"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
