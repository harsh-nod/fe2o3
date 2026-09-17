#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" ]]
for target in gnu musl; do
    [[ ! -e "$archive/$target.complete-roster" ]]
    awk -v target="$target" -f "$archive/roster.awk" "$archive/raw/$target.log" |
        LC_ALL=C sort > "$archive/$target.complete-roster"
done
test -s "$archive/gnu.complete-roster"
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
wc -l "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
sha256sum "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
