#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d \( -name __pycache__ -o -name .ruff_cache \) -print) ]]
[[ $(< "$archive/raw/verify-final-v2.exit") == 0 ]]
[[ -f "$archive/raw/verify-final-v2.finished" ]]
verified=$(python3 -I "$archive/verify.py")
cmp <(printf '%s\n' "$verified") "$archive/raw/verify-final-v2.log"
for script in "$archive"/*.sh; do bash -n "$script"; done
cd -- "$archive"
# shellcheck disable=SC2094
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
