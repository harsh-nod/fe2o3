#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
names=(benchmark benchmark-boundary benchmark-numa benchmark-settled boundary-script-identity cleanup cleanup-script-identity collect-boundary collect-initial collect-numa collect-settled numa-policy-preflight numa-script-identity numa-script-identity-final occupancy-after-cleanup occupancy-after-initial script-identities settled-script-identity summary summary-boundary summary-checked summary-final summary-numa summary-self-test topology)
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    expected=0
    [[ "$name" != benchmark ]] || expected=1
    [[ $(< "$archive/raw/$name.exit") == "$expected" ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    ((finished >= started))
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
order=(benchmark collect-initial benchmark-settled collect-settled benchmark-boundary collect-boundary benchmark-numa collect-numa cleanup occupancy-after-cleanup)
previous=0
for name in "${order[@]}"; do
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    ((started >= previous))
    previous=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
done
for prefix in initial boundary numa; do
    for file in binaries.sha256 source-files.sha256 source-before.log source-after.log; do
        cmp "$archive/settled-results/$file" "$archive/$prefix-results/$file"
    done
done
cmp "$archive/settled-results/source-before.log" "$archive/settled-results/source-after.log"
[[ $(wc -l < "$archive/settled-results/source-files.sha256") == 5505 ]]
[[ $(wc -l < "$archive/settled-results/binaries.sha256") == 3 ]]
cmp <(python3 -I "$archive/summarize.py") "$archive/raw/summary-final.log"
cmp <(python3 -I "$archive/summarize.py" benchmark-boundary) "$archive/raw/summary-boundary.log"
cmp <(python3 -I "$archive/summarize.py" benchmark-numa) "$archive/raw/summary-numa.log"
python3 -I "$archive/test-summary.py"
grep -Fx 'removed_owned_directory=/home/harsh/fe2o3-copy-diagnostic-20260917.6CF3d70A absent=yes' "$archive/raw/cleanup.log"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
