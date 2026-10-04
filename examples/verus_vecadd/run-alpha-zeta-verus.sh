#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
verus_bin=$(printenv VERUS 2>/dev/null || printf verus)
case "$verus_bin" in
    */*) verus_path=$verus_bin ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ] || [ ! -x "$verus_path" ]; then
    printf 'FAIL: Verus is unavailable (set VERUS=/absolute/path/to/verus)\n' >&2
    exit 1
fi

timeout_path=$(command -v timeout 2>/dev/null || true)
if [ -z "$timeout_path" ]; then
    printf 'FAIL: timeout is required\n' >&2
    exit 1
fi
timeout_seconds=$(printenv VERUS_TIMEOUT_SECONDS 2>/dev/null || printf 120)
case "$timeout_seconds" in
    ''|*[!0-9]*)
        printf 'FAIL: VERUS_TIMEOUT_SECONDS must be an integer from 1 through 300\n' >&2
        exit 2
        ;;
esac
if [ "$timeout_seconds" -lt 1 ] || [ "$timeout_seconds" -gt 300 ]; then
    printf 'FAIL: VERUS_TIMEOUT_SECONDS must be an integer from 1 through 300\n' >&2
    exit 2
fi

tmp_dir=$(mktemp -d "/tmp/fe2o3-alpha-zeta-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    "$timeout_path" --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

positive="$script_dir/verus/two_kernel.rs"
positive_log="$tmp_dir/positive.log"
if ! run_verus "$positive" >"$positive_log" 2>&1; then
    printf 'FAIL: alpha/zeta positive harness did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Eq '^verification results:: [0-9]+ verified, 0 errors$' "$positive_log"; then
    printf 'FAIL: alpha/zeta positive harness omitted a zero-error summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
printf 'PASS: alpha/zeta positive source model verified\n'

run_mutation() {
    name=$1
    file=$2
    marker=$3
    diagnostic=$4
    log="$tmp_dir/$name.log"
    if run_verus "$file" >"$log" 2>&1; then
        printf 'FAIL: %s unexpectedly verified\n' "$name" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log"; then
        printf 'FAIL: %s omitted marker %s\n' "$name" "$marker" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Eq "$diagnostic" "$log"; then
        printf 'FAIL: %s missed its intended proof obligation\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Eq '^verification results:: [0-9]+ verified, 1 errors$' "$log"; then
        printf 'FAIL: %s did not report exactly one proof error\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at its intended proof obligation\n' "$name"
}

run_mutation functional_result \
    "$script_dir/verus/negative/two_kernel_wrong_scalar.rs" \
    'mutated_alpha_uses_wrong_scalar_result' \
    'postcondition not satisfied'
run_mutation guarded_bounds \
    "$script_dir/verus/negative/two_kernel_guard_bypass.rs" \
    'mutated_alpha_bypasses_output_guard' \
    'precondition not met: index in bounds'
run_mutation injective_writes \
    "$script_dir/verus/negative/two_kernel_overlapping_output.rs" \
    'mutated_overlapping_output_ownership_is_race_free' \
    'postcondition not satisfied'
run_mutation initialized_inputs \
    "$script_dir/verus/negative/two_kernel_uninitialized_input.rs" \
    'mutated_uninitialized_input_is_readable' \
    'postcondition not satisfied'
run_mutation address_overflow \
    "$script_dir/verus/negative/two_kernel_address_overflow.rs" \
    'mutated_f32_address_overflow_is_representable' \
    'postcondition not satisfied'

printf 'Alpha/zeta Verus run passed: 1 positive harness, 5 expected rejections\n'
