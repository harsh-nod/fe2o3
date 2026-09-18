#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d -name __pycache__ -print) ]]
[[ $(find "$archive/raw" -type f | wc -l) == 95 ]]
for suffix in command started log finished exit; do
    [[ -f "$archive/raw/verify.$suffix" ]]
    [[ -f "$archive/raw/verify-corrected.$suffix" ]]
    [[ -f "$archive/raw/verify-final.$suffix" ]]
done
[[ $(< "$archive/raw/verify.exit") == 1 ]]
[[ $(< "$archive/raw/verify-corrected.exit") == 1 ]]
[[ $(< "$archive/raw/verify-final.exit") == 0 ]]
cmp <(python3 -I "$archive/verify.py") "$archive/raw/verify-final.log"
cmp <(python3 -I "$archive/legacy-isolation.py") "$archive/raw/legacy-isolation.log"
bash -n "$archive/record.sh" "$archive/seal.sh"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
