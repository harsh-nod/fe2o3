#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/tiled_gemm_host_contract.rs"
a_wrong="$script_dir/verus/negative/a_register_wrong.rs"
b_wrong="$script_dir/verus/negative/b_register_wrong.rs"
accumulator_wrong="$script_dir/verus/negative/accumulator_register_wrong.rs"
xor4_wrong="$script_dir/verus/negative/xor4_wrong.rs"
xor2_permutation_wrong="$script_dir/verus/negative/xor2_permutation_wrong.rs"
version_file="$script_dir/verus/VERUS_VERSION"
sha256_file="$script_dir/verus/VERUS_SHA256"

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

require_source() {
    file=$1
    marker=$2
    if ! grep -Fq "$marker" "$file"; then
        printf 'FAIL: %s is missing source marker %s\n' "$file" "$marker" >&2
        exit 1
    fi
}

forbid_source() {
    file=$1
    shortcut=$2
    if grep -Fq "$shortcut" "$file"; then
        printf 'FAIL: %s contains forbidden proof shortcut %s\n' \
            "$file" "$shortcut" >&2
        exit 1
    fi
}

for marker in \
    'pub open spec fn a_register_row_v1' \
    'pub open spec fn a_register_depth_v1' \
    'pub open spec fn b_register_depth_v1' \
    'pub open spec fn b_register_col_v1' \
    'pub open spec fn accumulator_row_v1' \
    'pub open spec fn accumulator_col_v1' \
    'pub open spec fn xor4_lds_col_v1' \
    'pub proof fn lane_component_register_maps_are_injective_v1' \
    'pub proof fn xor4_physical_layout_is_permutation_v1' \
    'pub proof fn distinct_lane_components_have_disjoint_a_lds_v1' \
    'pub proof fn distinct_lane_components_have_disjoint_b_lds_v1' \
    'pub proof fn all_unequal_invocations_own_disjoint_global_c_v1' \
    'pub proof fn a_phase_load_is_in_bounds_v1' \
    'pub proof fn b_phase_load_is_in_bounds_v1' \
    'pub proof fn k_phases_partition_every_depth_v1' \
    'pub proof fn checked_matrix_addresses_fit_u64_v1'
do
    require_source "$proof" "$marker"
done

require_source "$a_wrong" 'mutated_a_matches_official_table_v1'
require_source "$b_wrong" 'mutated_b_matches_official_table_v1'
require_source "$accumulator_wrong" 'mutated_accumulator_matches_official_table_v1'
require_source "$xor4_wrong" 'mutated_xor4_matches_official_storage_v1'
require_source "$xor2_permutation_wrong" \
    'mutated_two_bit_permutation_matches_official_xor2_v1'

for file in \
    "$proof" "$a_wrong" "$b_wrong" "$accumulator_wrong" "$xor4_wrong" \
    "$xor2_permutation_wrong"
do
    for shortcut in 'admit(' 'assume(' '#[verifier::external_body]'; do
        forbid_source "$file" "$shortcut"
    done
done

if [ ! -s "$version_file" ]; then
    printf 'FAIL: missing pinned Verus version file %s\n' "$version_file" >&2
    exit 1
fi
expected_version=$(sed -n '1p' "$version_file")
case "$expected_version" in
    ''|*[!0-9A-Za-z.-]*)
        printf 'FAIL: invalid pinned Verus version %s\n' "$expected_version" >&2
        exit 1
        ;;
esac

if [ ! -s "$sha256_file" ]; then
    printf 'FAIL: missing pinned Verus SHA-256 file %s\n' "$sha256_file" >&2
    exit 1
fi
expected_sha256=$(sed -n '1p' "$sha256_file")
case "$expected_sha256" in
    *[!0-9a-f]*|'')
        printf 'FAIL: invalid pinned Verus SHA-256 %s\n' "$expected_sha256" >&2
        exit 1
        ;;
esac
if [ "${#expected_sha256}" -ne 64 ]; then
    printf 'FAIL: pinned Verus SHA-256 must contain exactly 64 hex digits\n' >&2
    exit 1
fi

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'FAIL: Verus is unavailable; set VERUS=/absolute/path/to/verus\n' >&2
    exit 1
fi

sha256_path=$(command -v sha256sum 2>/dev/null || true)
if [ -z "$sha256_path" ]; then
    printf 'FAIL: sha256sum is required to authenticate Verus bytes\n' >&2
    exit 1
fi
actual_sha256=$("$sha256_path" "$verus_path" | awk '{ print $1 }')
if [ "$actual_sha256" != "$expected_sha256" ]; then
    printf 'FAIL: Verus executable SHA-256 %s does not match pinned %s\n' \
        "${actual_sha256:-unknown}" "$expected_sha256" >&2
    exit 1
fi

actual_version=$(
    "$verus_path" --version \
        | awk '/^[[:space:]]*Version:/ { print $2; exit }'
)
if [ "$actual_version" != "$expected_version" ]; then
    printf 'FAIL: Verus version %s does not match pinned %s\n' \
        "${actual_version:-unknown}" "$expected_version" >&2
    exit 1
fi

timeout_path=$(command -v timeout 2>/dev/null || true)
if [ -z "$timeout_path" ]; then
    printf 'FAIL: timeout is required to bound Verus execution\n' >&2
    exit 1
fi

timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-tiled-gemm-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    file=$1
    "$timeout_path" --foreground --signal=TERM --kill-after=5 \
        "$timeout_seconds" \
        "$verus_path" --crate-type lib --triggers-mode silent "$file"
}

positive_log="$tmp_dir/positive.log"
if run_verus "$proof" >"$positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: positive tiled GEMM host proof did not verify (status %s)\n' \
        "$status" >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 73 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: positive proof emitted an unexpected verification summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
printf 'PASS: tiled GEMM host contract verified (73 verified, 0 errors)\n'

run_rejected() {
    name=$1
    file=$2
    marker=$3
    log="$tmp_dir/$name.log"
    if run_verus "$file" >"$log" 2>&1; then
        printf 'FAIL: %s unexpectedly verified\n' "$name" >&2
        exit 1
    else
        status=$?
    fi
    if [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
        printf 'FAIL: %s exceeded the %ss Verus time limit\n' \
            "$name" "$timeout_seconds" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log"; then
        printf 'FAIL: %s failed without marker %s\n' "$name" "$marker" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq 'error: postcondition not satisfied' "$log"; then
        printf 'FAIL: %s failed without the expected proof diagnostic\n' \
            "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Eq '^verification results:: [0-9]+ verified, 1 errors$' "$log"; then
        printf 'FAIL: %s did not report exactly one rejected function\n' \
            "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at the official-layout correspondence check\n' "$name"
}

run_rejected a_register_wrong "$a_wrong" 'mutated_a_matches_official_table_v1'
run_rejected b_register_wrong "$b_wrong" 'mutated_b_matches_official_table_v1'
run_rejected accumulator_register_wrong \
    "$accumulator_wrong" \
    'mutated_accumulator_matches_official_table_v1'
run_rejected xor4_wrong "$xor4_wrong" 'mutated_xor4_matches_official_storage_v1'
run_rejected xor2_permutation_wrong \
    "$xor2_permutation_wrong" \
    'mutated_two_bit_permutation_matches_official_xor2_v1'

printf 'Verus fixture run passed: 23 public theorems, 5 expected rejections\n'
