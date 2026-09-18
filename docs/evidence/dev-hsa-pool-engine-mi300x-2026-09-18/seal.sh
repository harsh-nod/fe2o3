#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d -name __pycache__ -print) ]]
names=(create stage prepare preflight stage-cleanup inspect-before benchmark inspect-after hash-binaries collect summary-test summary cleanup occupancy-after-cleanup audit lint)
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    ((finished >= started))
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
cmp <(python3 -I "$archive/summarize.py") "$archive/raw/summary.log"
cmp <(python3 -I "$archive/audit.py") "$archive/raw/audit.log"
python3 -I "$archive/test-summary.py" -v
bash -n "$archive/record.sh" "$archive/create.sh" "$archive/prepare.sh" "$archive/guard.sh" "$archive/run.sh" "$archive/inspect.sh" "$archive/cleanup.sh" "$archive/seal.sh"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
