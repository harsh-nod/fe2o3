#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d -name __pycache__ -print) ]]
for name in audit lint format shellcheck; do
    [[ $(< "$archive/portable/$name.exit") == 0 ]]
    [[ -f "$archive/portable/$name.finished" ]]
done
verified=$(python3 -B "$archive/audit.py")
cmp <(printf '%s\n' "$verified") "$archive/portable/audit.stdout"
[[ ! -s "$archive/portable/audit.stderr" ]]
for script in "$archive"/*.sh "$archive"/returned/*.sh; do
    bash -n "$script"
done
cd -- "$archive"
# The manifest is explicitly excluded from its own input roster.
# shellcheck disable=SC2094
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
