#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
baseline="$archive/../dev-v6-copy-read-leases-2026-09-17/raw"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-roster-check.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
for target in gnu musl; do
    awk -v target="$target" -f "$archive/roster.awk" "$baseline/$target.log" > /dev/null
done
mutations=(missing_package duplicate_summary duplicate_row wrong_count measured filtered unknown_status wrong_package wrong_target missing_row)
for mutation in "${mutations[@]}"; do
    awk -v mutation="$mutation" '
        /Running unittests/ { package=$NF }
        mutation == "missing_package" && package ~ /fe2o3_host-/ { next }
        mutation == "duplicate_summary" && /^test result:/ && !changed++ { print }
        mutation == "duplicate_row" && /^test .* \.\.\. ok$/ && !changed++ { print }
        mutation == "wrong_count" && /^running [0-9]+ tests$/ && !changed++ { $2++ }
        mutation == "measured" && /^test result:/ && !changed++ { sub(/0 measured;/, "1 measured;") }
        mutation == "filtered" && /^test result:/ && !changed++ { sub(/0 filtered/, "1 filtered") }
        mutation == "unknown_status" && /^test .* \.\.\. ok$/ && !changed++ { sub(/ok$/, "FAILED") }
        mutation == "wrong_package" && /Running unittests/ && !changed++ { sub(/fe2o3_host-/, "unrelated-") }
        mutation == "wrong_target" && /Running unittests/ && !changed++ { sub(/target\/debug/, "target/x86_64-unknown-linux-musl/debug") }
        mutation == "missing_row" && /^test .* \.\.\. ok$/ && !changed++ { next }
        { print }
    ' "$baseline/gnu.log" > "$scratch/$mutation.log"
    if awk -v target=gnu -f "$archive/roster.awk" "$scratch/$mutation.log" > /dev/null 2>&1; then
        printf 'mutation unexpectedly accepted: %s\n' "$mutation" >&2
        exit 1
    fi
    printf 'rejected %s\n' "$mutation"
done
printf 'accepted both complete baseline rosters; rejected all 10 mutations\n'
