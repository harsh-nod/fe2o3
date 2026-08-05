#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
require_verus=0
if [ "${1:-}" = "--require" ]; then
    require_verus=1
    shift
fi
if [ "$#" -ne 0 ]; then
    printf 'usage: %s [--require]\n' "$0" >&2
    exit 2
fi

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*)
        if [ ! -x "$verus_bin" ]; then
            verus_path=
        else
            verus_path=$verus_bin
        fi
        ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac

if [ -z "$verus_path" ]; then
    printf 'SKIP: Verus is unavailable (set VERUS=/path/to/verus)\n'
    if [ "$require_verus" -eq 1 ]; then
        exit 1
    fi
    exit 0
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-verus-fill.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

failures=0
run_pass() {
    name=$1
    file=$2
    log="$tmp_dir/$name.log"
    if "$verus_path" --crate-type lib --triggers-mode silent "$file" >"$log" 2>&1; then
        printf 'PASS:  %s verified\n' "$name"
    else
        printf 'FAIL:  %s was expected to verify\n' "$name" >&2
        cat "$log" >&2
        failures=$((failures + 1))
    fi
}

run_rejected() {
    name=$1
    file=$2
    marker=$3
    diagnostic=$4
    log="$tmp_dir/$name.log"
    if "$verus_path" --crate-type lib --triggers-mode silent "$file" >"$log" 2>&1; then
        printf 'FAIL:  %s unexpectedly verified\n' "$name" >&2
        failures=$((failures + 1))
    elif ! grep -Fq "$marker" "$log"; then
        printf 'FAIL:  %s failed without marker %s\n' "$name" "$marker" >&2
        cat "$log" >&2
        failures=$((failures + 1))
    elif grep -Eiq "$diagnostic" "$log"; then
        printf 'XFAIL: %s rejected for the expected proof obligation\n' "$name"
    else
        printf 'FAIL:  %s failed without the expected proof diagnostic\n' "$name" >&2
        cat "$log" >&2
        failures=$((failures + 1))
    fi
}

run_pass vecadd "$script_dir/verus/vecadd.rs"
run_pass fill "$script_dir/verus/fill.rs"
run_rejected fill_missing_bounds \
    "$script_dir/verus/negative/fill_missing_bounds.rs" \
    'mutated_fill_index_is_in_bounds' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected fill_non_injective \
    "$script_dir/verus/negative/fill_non_injective.rs" \
    'mutated_distinct_threads_have_disjoint_outputs' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected fill_incorrect_postcondition \
    "$script_dir/verus/negative/fill_incorrect_postcondition.rs" \
    'mutated_one_write_fills_every_element' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected permission_overlapping_output_writes \
    "$script_dir/verus/negative/permission_overlapping_output_writes.rs" \
    'mutated_overlapping_output_writes_are_disjoint' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected permission_write_read_alias \
    "$script_dir/verus/negative/permission_write_read_alias.rs" \
    'mutated_write_read_alias_is_compatible' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected permission_out_of_bounds_region \
    "$script_dir/verus/negative/permission_out_of_bounds_region.rs" \
    'mutated_unbounded_region_is_in_bounds' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected same_source_wrong_bounds \
    "$script_dir/verus/negative/same_source_wrong_bounds.rs" \
    'thread.linear < domain.length' \
    'precondition.*not satisfied|precondition failure'
run_rejected same_source_output_alias \
    "$script_dir/verus/negative/same_source_output_alias.rs" \
    'rejects_output_input_alias' \
    'precondition.*not satisfied|precondition failure'
run_rejected same_source_functional_error \
    "$script_dir/verus/negative/same_source_functional_error.rs" \
    'mutated_same_source_claims_wrong_sum' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected real_kernel_wrong_bounds \
    "$script_dir/verus/negative/real_kernel_wrong_bounds.rs" \
    'rejects_missing_real_kernel_thread_bound' \
    'precondition.*not satisfied|precondition failure'
run_rejected real_kernel_wrong_index \
    "$script_dir/verus/negative/real_kernel_wrong_index.rs" \
    'mutated_real_kernel_index_is_injective' \
    'postcondition.*not satisfied|postcondition failure'
run_rejected real_kernel_output_alias \
    "$script_dir/verus/negative/real_kernel_output_alias.rs" \
    'rejects_real_output_input_alias' \
    'precondition.*not satisfied|precondition failure'

if [ "$failures" -ne 0 ]; then
    printf 'Verus fixture run failed: %s unexpected result(s)\n' "$failures" >&2
    exit 1
fi
printf 'Verus fixture run passed: 2 proof harnesses, 12 expected rejections\n'
