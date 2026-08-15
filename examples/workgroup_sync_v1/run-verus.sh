#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$root/verus/workgroup_sync_v1.rs"
negative_dir="$root/verus/negative"
expected_version=$(sed -n '1p' "$root/verus/VERUS_VERSION")
expected_sha256=$(sed -n '1p' "$root/verus/VERUS_SHA256")

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'SKIP: pinned Verus is unavailable; set VERUS=/absolute/path/to/verus\n' >&2
    exit 77
fi

actual_sha256=$(sha256sum "$verus_path" | awk '{ print $1 }')
if [ "$actual_sha256" != "$expected_sha256" ]; then
    printf 'SKIP: Verus executable SHA-256 %s does not match pinned %s\n' \
        "$actual_sha256" "$expected_sha256" >&2
    exit 77
fi

verus_root=$(CDPATH='' cd -- "$(dirname -- "$verus_path")" && pwd)
actual_version=$(VERUS_Z3_PATH="$verus_root/z3" "$verus_path" --version \
    | awk '/^[[:space:]]*Version:/ { print $2; exit }')
if [ "$actual_version" != "$expected_version" ]; then
    printf 'FAIL: Verus version %s does not match pinned %s\n' \
        "${actual_version:-unknown}" "$expected_version" >&2
    exit 1
fi

for source in "$proof" "$negative_dir"/*.rs; do
    if grep -En '\b(admit|assume|external_body)\b' "$source" >/dev/null; then
        printf 'FAIL: forbidden proof escape in %s\n' "$source" >&2
        exit 1
    fi
done

timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
case "$timeout_seconds" in
    ''|*[!0-9]*) printf 'FAIL: invalid VERUS_TIMEOUT_SECONDS\n' >&2; exit 2 ;;
esac
if [ "$timeout_seconds" -lt 1 ] || [ "$timeout_seconds" -gt 300 ]; then
    printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2
    exit 2
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-workgroup-sync-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    timeout --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
        env VERUS_Z3_PATH="$verus_root/z3" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

positive_log="$tmp_dir/positive.log"
if ! run_verus "$proof" >"$positive_log" 2>&1; then
    printf 'FAIL: positive workgroup synchronization proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq '0 errors' "$positive_log"; then
    printf 'FAIL: positive proof emitted an unexpected summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
cat "$positive_log"

check_negative() {
    name=$1
    marker=$2
    log="$tmp_dir/$name.log"
    if run_verus "$negative_dir/$name.rs" >"$log" 2>&1; then
        printf 'FAIL: expected-negative proof unexpectedly verified: %s\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log"; then
        printf 'FAIL: expected-negative proof failed at an unexpected surface: %s\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at %s\n' "$name" "$marker"
}

check_negative initialization_wrong_slot mutated_last_lane_still_initializes_in_bounds_v1
check_negative convergence_divergent_barrier mutated_distinct_lanes_reach_same_barrier_v1
check_negative epoch_reuse_missing_barrier mutated_reuse_precedes_next_publish_v1
check_negative ownership_duplicate_writer mutated_two_output_owners_are_equal_v1
check_negative sum_drops_last_lane mutated_reduction_still_equals_exact_sum_v1
check_negative atomic_ineligible_contributes mutated_atomic_sum_respects_eligibility_v1

printf 'PASS: pinned workgroup synchronization proofs and mutations checked\n'
