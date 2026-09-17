#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
test ! -e "$archive/audit.log"
test ! -e "$archive/SHA256SUMS"
bash "$archive/audit.sh" > "$archive/audit.log"
(
    cd -- "$archive"
    find . -type f ! -name SHA256SUMS -printf '%P\0' |
        LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
)
bash "$archive/audit.sh" | cmp - "$archive/audit.log"
