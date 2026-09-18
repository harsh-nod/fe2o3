#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
[[ -z $(find "$archive" -type d -name __pycache__ -print) ]]
for name in audit python-lint python-format shellcheck; do
    for suffix in command started finished exit log; do
        [[ -f "$archive/review/$name.$suffix" ]]
    done
    [[ $(< "$archive/review/$name.exit") == 0 ]]
done
[[ $(find "$archive/review" -type f | wc -l) == 20 ]]
verified=$(python3 -I "$archive/audit.py" --repo "$root")
cmp <(printf '%s\n' "$verified") "$archive/review/audit.log"
for script in "$archive"/*.sh; do bash -n "$script"; done
cd -- "$archive"
# Exclude the manifest itself from its input roster.
# shellcheck disable=SC2094
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
