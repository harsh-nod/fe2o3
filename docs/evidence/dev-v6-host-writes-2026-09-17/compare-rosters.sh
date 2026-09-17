#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" ]]
for target in gnu musl; do
    [[ ! -e "$archive/$target.roster" ]]
    awk '
        /Running unittests/ {
            package = $NF
            gsub(/[()]/, "", package)
            sub(/^.*\//, "", package)
            sub(/-[0-9a-f]+$/, "", package)
        }
        /^test [^ ]+ \.\.\. (ok|ignored)/ { print package, $0 }
    ' "$archive/raw/$target.log" | LC_ALL=C sort > "$archive/$target.roster"
done
test -s "$archive/gnu.roster"
cmp "$archive/gnu.roster" "$archive/musl.roster"
wc -l "$archive/gnu.roster" "$archive/musl.roster"
sha256sum "$archive/gnu.roster" "$archive/musl.roster"
