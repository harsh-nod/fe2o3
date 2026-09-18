#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
baseline="$archive/../dev-v4j3-reader-commits-2026-09-17"
parser="$archive/../dev-v6-generated-read-leases-2026-09-17/roster.awk"
[[ ! -e "$archive/SHA256SUMS" ]]
for target in gnu musl; do
    [[ ! -e "$archive/$target.complete-roster" ]]
    awk -v target="$target" -f "$parser" "$archive/raw/$target.log" |
        LC_ALL=C sort > "$archive/$target.complete-roster"
done
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
[[ $(wc -l < "$archive/gnu.complete-roster") == 2147 ]]
[[ -z $(LC_ALL=C comm -23 "$baseline/gnu.complete-roster" "$archive/gnu.complete-roster") ]]
LC_ALL=C comm -13 "$baseline/gnu.complete-roster" "$archive/gnu.complete-roster" > "$archive/added-tests.list"
[[ $(wc -l < "$archive/added-tests.list") == 1 ]]
grep -F 'whole_reader_state_survives_base_lifecycle_and_slot_reuse' "$archive/added-tests.list"
wc -l "$archive/gnu.complete-roster" "$archive/musl.complete-roster" "$archive/added-tests.list"
sha256sum "$archive/gnu.complete-roster" "$archive/musl.complete-roster" "$parser"
