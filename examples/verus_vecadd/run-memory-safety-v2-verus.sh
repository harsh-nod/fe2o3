#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
positive="$script_dir/verus/memory_safety_v2.rs"
negative_dir="$script_dir/verus/negative"

for marker in \
    'pub proof fn nested_range_stays_in_bounds' \
    'pub proof fn stale_generation_cannot_match' \
    'pub proof fn nested_loan_lifetime_is_live_with_allocation' \
    'pub proof fn ordered_exclusive_ranges_are_compatible' \
    'pub proof fn valid_write_initializes_same_typed_read' \
    'pub proof fn same_allocation_element_distance_is_integral' \
    'pub proof fn gfx942_profile_fixes_pointer_widths_and_alignments' \
    'pub proof fn exclusive_bound_is_not_a_materialized_workgroup_pointer' \
    'pub proof fn zero_size_storage_never_overlaps' \
    'pub proof fn admitted_live_storage_makes_copy_ranges_disjoint' \
    'pub proof fn adjacent_validity_ranges_are_not_canonical' \
    'pub proof fn full_domain_and_nonzero_ranges_require_named_encodings'
do
    if ! grep -Fq "$marker" "$positive"; then
        printf 'FAIL: missing proof marker %s\n' "$marker" >&2
        exit 1
    fi
done

for forbidden in 'admit(' 'assume(false' '#[verifier::external_body]'; do
    if grep -Fq "$forbidden" "$positive"; then
        printf 'FAIL: positive proof contains forbidden shortcut %s\n' "$forbidden" >&2
        exit 1
    fi
done

if [ "${1:-}" = "--source-only" ]; then
    printf 'PASS: memory-safety V2 Verus source shape\n'
    exit 0
fi
if [ "$#" -ne 0 ]; then
    printf 'usage: %s [--source-only]\n' "$0" >&2
    exit 2
fi

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) verus_path=$verus_bin ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ] || [ ! -x "$verus_path" ]; then
    printf 'FAIL: Verus is unavailable; set VERUS=/path/to/verus\n' >&2
    exit 1
fi
timeout_path=$(command -v timeout 2>/dev/null || true)
if [ -z "$timeout_path" ]; then
    printf 'FAIL: timeout is required\n' >&2
    exit 1
fi

limit=${VERUS_TIMEOUT_SECONDS:-120}
case "$limit" in
    ''|*[!0-9]*) printf 'FAIL: invalid VERUS_TIMEOUT_SECONDS\n' >&2; exit 2 ;;
esac
if [ "$limit" -lt 1 ] || [ "$limit" -gt 300 ]; then
    printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2
    exit 2
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-memory-v2-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    "$timeout_path" --foreground --signal=TERM --kill-after=5 "$limit" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

if ! run_verus "$positive" >"$tmp_dir/positive.log" 2>&1; then
    printf 'FAIL: positive memory-safety proof did not verify\n' >&2
    cat "$tmp_dir/positive.log" >&2
    exit 1
fi
printf 'PASS: positive memory-safety proof verified\n'

run_rejected() {
    name=$1
    marker=$2
    file="$negative_dir/$name.rs"
    log="$tmp_dir/$name.log"
    if run_verus "$file" >"$log" 2>&1; then
        printf 'FAIL: %s unexpectedly verified\n' "$name" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$file" || ! grep -Fq 'postcondition not satisfied' "$log"; then
        printf 'FAIL: %s failed for an unexpected reason\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at its memory obligation\n' "$name"
}

run_rejected memory_safety_v2_oob mutated_out_of_bounds_is_accepted
run_rejected memory_safety_v2_stale mutated_stale_generation_is_accepted
run_rejected memory_safety_v2_alias mutated_overlapping_exclusive_loans_are_compatible
run_rejected memory_safety_v2_pointer_width mutated_one_past_workgroup_pointer_is_representable
run_rejected memory_safety_v2_physical_alias mutated_distinct_allocation_ids_imply_physical_disjointness
run_rejected memory_safety_v2_validity_canonical mutated_adjacent_validity_ranges_are_canonical
run_rejected memory_safety_v2_target_layout mutated_64_bit_workgroup_layout_is_gfx942
