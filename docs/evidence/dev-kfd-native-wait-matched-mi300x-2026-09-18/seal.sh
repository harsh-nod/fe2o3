#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d -name __pycache__ -print) ]]
[[ $(< "$archive/raw/audit.exit") == 0 ]]
checked=$(python3 -I "$archive/audit.py")
cmp <(printf '%s\n' "$checked") "$archive/raw/audit.log"
for script in "$archive"/*.sh; do bash -n "$script"; done
cd -- "$archive"
# The output manifest is explicitly excluded from the input roster.
# shellcheck disable=SC2094
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
