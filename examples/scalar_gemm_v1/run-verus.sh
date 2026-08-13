#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
proof="$repo_root/crates/fe2o3-verifier/verus/scalar_gemm_v1.rs"
require_verus=0

if [ "${1:-}" = "--require" ]; then
    require_verus=1
    shift
fi
if [ "$#" -ne 0 ]; then
    printf 'usage: %s [--require]\n' "$0" >&2
    exit 2
fi

for marker in \
    'pub proof fn active_invocation_has_unique_coordinates' \
    'pub proof fn active_accesses_are_in_bounds' \
    'pub proof fn active_input_reads_are_initialized' \
    'pub proof fn distinct_active_invocations_have_distinct_output_indices' \
    'pub proof fn every_output_has_unique_canonical_invocation' \
    'pub proof fn exact_dot_has_fixed_sequential_recurrence'
do
    if ! grep -Fq "$marker" "$proof"; then
        printf 'FAIL: scalar GEMM proof is missing %s\n' "$marker" >&2
        exit 1
    fi
done

for shortcut in 'admit(' 'assume(false' '#[verifier::external_body]'; do
    if grep -Fq "$shortcut" "$proof"; then
        printf 'FAIL: scalar GEMM proof contains forbidden shortcut %s\n' "$shortcut" >&2
        exit 1
    fi
done

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'SKIP: Verus is unavailable (set VERUS=/path/to/verus)\n'
    [ "$require_verus" -eq 0 ] && exit 0
    exit 1
fi

timeout_seconds=${VERUS_TIMEOUT_SECONDS:-60}
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

timeout --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
    "$verus_path" --crate-type lib --triggers-mode silent "$proof"
