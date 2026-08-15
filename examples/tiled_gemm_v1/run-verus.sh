#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/tiled_gemm_host_contract.rs"
lds_proof="$script_dir/verus/lds_tiled_slice1.rs"
kphase_proof="$script_dir/verus/lds_tiled_kphase.rs"
grid_proof="$script_dir/verus/lds_tiled_grid_stride.rs"
edges_proof="$script_dir/verus/lds_tiled_edges_alpha_beta.rs"
a_wrong="$script_dir/verus/negative/a_register_wrong.rs"
b_wrong="$script_dir/verus/negative/b_register_wrong.rs"
accumulator_wrong="$script_dir/verus/negative/accumulator_register_wrong.rs"
xor4_wrong="$script_dir/verus/negative/xor4_wrong.rs"
xor2_permutation_wrong="$script_dir/verus/negative/xor2_permutation_wrong.rs"
lds_epoch_wrong="$script_dir/verus/negative/lds_epoch_wrong.rs"
lds_product_wrong="$script_dir/verus/negative/lds_product_wrong.rs"
kphase_reuse_wrong="$script_dir/verus/negative/lds_kphase_reuse_wrong.rs"
kphase_accumulator_reset_wrong="$script_dir/verus/negative/lds_kphase_accumulator_reset_wrong.rs"
grid_tile_mapping_wrong="$script_dir/verus/negative/lds_grid_tile_mapping_wrong.rs"
grid_stride_wrong="$script_dir/verus/negative/lds_grid_stride_wrong.rs"
grid_c_ownership_wrong="$script_dir/verus/negative/lds_grid_c_ownership_wrong.rs"
edges_lane_skips_barrier_wrong="$script_dir/verus/negative/lds_edges_lane_skips_barrier_wrong.rs"
edges_unguarded_tail_load_wrong="$script_dir/verus/negative/lds_edges_unguarded_tail_load_wrong.rs"
edges_unguarded_tail_store_wrong="$script_dir/verus/negative/lds_edges_unguarded_tail_store_wrong.rs"
edges_alpha_beta_wrong="$script_dir/verus/negative/lds_edges_alpha_beta_wrong.rs"
edges_k_tail_coverage_wrong="$script_dir/verus/negative/lds_edges_k_tail_coverage_wrong.rs"
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
for marker in \
    'pub open spec fn kphase_write_epoch_v1' \
    'pub open spec fn kphase_read_epoch_v1' \
    'pub open spec fn kphase_reuse_epoch_v1' \
    'pub proof fn bounded_kphase_global_loads_v1' \
    'pub proof fn bounded_k_phases_partition_depth_v1' \
    'pub proof fn every_kphase_a_read_is_initialized_v1' \
    'pub proof fn every_kphase_b_read_is_initialized_v1' \
    'pub proof fn kphase_publish_and_reuse_barriers_converge_v1' \
    'pub proof fn no_kphase_overwrite_before_prior_reads_v1' \
    'pub proof fn kphase_inner_accumulator_invariant_v1' \
    'pub proof fn kphase_accumulator_invariant_preserved_v1' \
    'pub proof fn kphase_final_c_stores_are_disjoint_v1' \
    'pub proof fn bounded_kphase_lds_result_is_matrix_product_v1'
do
    require_source "$kphase_proof" "$marker"
done
require_source "$kphase_reuse_wrong" \
    'mutated_missing_reuse_epoch_protects_prior_reads_v1'
require_source "$kphase_accumulator_reset_wrong" \
    'mutated_accumulator_reset_preserves_k_product_v1'
for marker in \
    'pub open spec fn checked_grid_problem_v1' \
    'pub open spec fn grid_a_index_v1' \
    'pub open spec fn grid_b_index_v1' \
    'pub open spec fn grid_c_index_v1' \
    'pub proof fn checked_grid_derivation_is_exact_v1' \
    'pub proof fn workgroup_to_tile_mapping_is_injective_v1' \
    'pub proof fn all_grid_global_a_b_loads_are_in_bounds_v1' \
    'pub proof fn each_grid_lane_four_c_stores_are_in_bounds_v1' \
    'pub proof fn distinct_grid_invocations_own_disjoint_c_v1' \
    'pub proof fn grid_slice1_barrier_converges_for_one_workgroup_v1'
do
    require_source "$grid_proof" "$marker"
done
require_source "$grid_tile_mapping_wrong" \
    'mutated_grid_mapping_is_injective_v1'
require_source "$grid_stride_wrong" \
    'mutated_undersized_lda_keeps_a_load_in_bounds_v1'
require_source "$grid_c_ownership_wrong" \
    'mutated_distinct_grid_owners_have_disjoint_c_v1'
for marker in \
    'pub open spec fn bounded_positive_edges_problem_v1' \
    'pub open spec fn edges_a_load_enabled_v1' \
    'pub open spec fn edges_b_load_enabled_v1' \
    'pub open spec fn edges_c_store_enabled_v1' \
    'pub proof fn each_lane_predicated_global_load_is_bounded_or_zero_filled_v1' \
    'pub proof fn each_lane_predicated_c_access_has_no_oob_store_v1' \
    'pub proof fn distinct_valid_edge_output_owners_are_disjoint_v1' \
    'pub proof fn each_valid_k_depth_has_exactly_one_tiled_position_v1' \
    'pub proof fn valid_k_depth_tiled_position_is_unique_v1' \
    'pub proof fn every_oob_tile_element_is_zero_filled_v1' \
    'pub proof fn barrier_convergence_is_independent_of_predicates_v1' \
    'pub proof fn k_tail_contributes_every_valid_depth_exactly_once_v1' \
    'pub proof fn each_valid_edge_output_has_exact_alpha_beta_v1'
do
    require_source "$edges_proof" "$marker"
done
require_source "$edges_lane_skips_barrier_wrong" \
    'mutated_predicate_off_lane_still_reaches_barrier_v1'
require_source "$edges_unguarded_tail_load_wrong" \
    'mutated_unguarded_tail_load_is_in_bounds_v1'
require_source "$edges_unguarded_tail_store_wrong" \
    'mutated_unguarded_tail_store_is_in_bounds_v1'
require_source "$edges_alpha_beta_wrong" \
    'mutated_wrong_alpha_beta_matches_exact_contract_v1'
require_source "$edges_k_tail_coverage_wrong" \
    'mutated_floor_phases_cover_k_tail_v1'

for file in \
    "$proof" "$a_wrong" "$b_wrong" "$accumulator_wrong" "$xor4_wrong" \
    "$xor2_permutation_wrong" "$kphase_proof" "$kphase_reuse_wrong" \
    "$kphase_accumulator_reset_wrong" "$grid_proof" \
    "$grid_tile_mapping_wrong" "$grid_stride_wrong" "$grid_c_ownership_wrong" \
    "$edges_proof" "$edges_lane_skips_barrier_wrong" \
    "$edges_unguarded_tail_load_wrong" "$edges_unguarded_tail_store_wrong" \
    "$edges_alpha_beta_wrong" "$edges_k_tail_coverage_wrong"
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

lds_positive_log="$tmp_dir/lds-positive.log"
if run_verus "$lds_proof" >"$lds_positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: Slice 1 LDS tiled GEMM proof did not verify (status %s)\n' \
        "$status" >&2
    cat "$lds_positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 93 verified, 0 errors' "$lds_positive_log"; then
    printf 'FAIL: Slice 1 LDS proof emitted an unexpected verification summary\n' >&2
    cat "$lds_positive_log" >&2
    exit 1
fi
printf 'PASS: Slice 1 LDS tiled GEMM model verified (93 verified, 0 errors)\n'

kphase_positive_log="$tmp_dir/kphase-positive.log"
if run_verus "$kphase_proof" >"$kphase_positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: Slice 2 LDS K-phase proof did not verify (status %s)\n' \
        "$status" >&2
    cat "$kphase_positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 196 verified, 0 errors' \
    "$kphase_positive_log"
then
    printf 'FAIL: Slice 2 LDS K-phase proof emitted an unexpected verification summary\n' \
        >&2
    cat "$kphase_positive_log" >&2
    exit 1
fi
printf 'PASS: Slice 2 LDS K-phase model verified (196 verified, 0 errors)\n'

grid_positive_log="$tmp_dir/grid-positive.log"
if run_verus "$grid_proof" >"$grid_positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: Slice 3 LDS grid-stride proof did not verify (status %s)\n' \
        "$status" >&2
    cat "$grid_positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 101 verified, 0 errors' \
    "$grid_positive_log"
then
    printf 'FAIL: Slice 3 LDS grid-stride proof emitted an unexpected verification summary\n' \
        >&2
    cat "$grid_positive_log" >&2
    exit 1
fi
printf 'PASS: Slice 3 LDS grid-stride model verified (101 verified, 0 errors)\n'

edges_positive_log="$tmp_dir/edges-positive.log"
if run_verus "$edges_proof" >"$edges_positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: Slice 4 LDS edge alpha/beta proof did not verify (status %s)\n' \
        "$status" >&2
    cat "$edges_positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 101 verified, 0 errors' \
    "$edges_positive_log"
then
    printf 'FAIL: Slice 4 LDS edge proof emitted an unexpected verification summary\n' \
        >&2
    cat "$edges_positive_log" >&2
    exit 1
fi
printf 'PASS: Slice 4 LDS edge alpha/beta model verified (101 verified, 0 errors)\n'

run_rejected() {
    name=$1
    file=$2
    marker=$3
    expected_verified=${4:-}
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
    if [ -n "$expected_verified" ] \
        && ! grep -Fq "verification results:: $expected_verified verified, 1 errors" "$log"
    then
        printf 'FAIL: %s emitted an unexpected verification summary\n' \
            "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    if [ -z "$expected_verified" ] \
        && ! grep -Eq '^verification results:: [0-9]+ verified, 1 errors$' "$log"
    then
        printf 'FAIL: %s did not report exactly one rejected function\n' \
            "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at the expected proof obligation\n' "$name"
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
run_rejected lds_epoch_wrong \
    "$lds_epoch_wrong" \
    'mutated_cross_epoch_read_is_initialized_v1'
run_rejected lds_product_wrong \
    "$lds_product_wrong" \
    'mutated_lds_result_has_extra_unit_v1'
run_rejected lds_kphase_reuse_wrong \
    "$kphase_reuse_wrong" \
    'mutated_missing_reuse_epoch_protects_prior_reads_v1'
run_rejected lds_kphase_accumulator_reset_wrong \
    "$kphase_accumulator_reset_wrong" \
    'mutated_accumulator_reset_preserves_k_product_v1'
run_rejected lds_grid_tile_mapping_wrong \
    "$grid_tile_mapping_wrong" \
    'mutated_grid_mapping_is_injective_v1'
run_rejected lds_grid_stride_wrong \
    "$grid_stride_wrong" \
    'mutated_undersized_lda_keeps_a_load_in_bounds_v1'
run_rejected lds_grid_c_ownership_wrong \
    "$grid_c_ownership_wrong" \
    'mutated_distinct_grid_owners_have_disjoint_c_v1'
run_rejected lds_edges_lane_skips_barrier_wrong \
    "$edges_lane_skips_barrier_wrong" \
    'mutated_predicate_off_lane_still_reaches_barrier_v1' \
    101
run_rejected lds_edges_unguarded_tail_load_wrong \
    "$edges_unguarded_tail_load_wrong" \
    'mutated_unguarded_tail_load_is_in_bounds_v1' \
    101
run_rejected lds_edges_unguarded_tail_store_wrong \
    "$edges_unguarded_tail_store_wrong" \
    'mutated_unguarded_tail_store_is_in_bounds_v1' \
    101
run_rejected lds_edges_alpha_beta_wrong \
    "$edges_alpha_beta_wrong" \
    'mutated_wrong_alpha_beta_matches_exact_contract_v1' \
    101
run_rejected lds_edges_k_tail_coverage_wrong \
    "$edges_k_tail_coverage_wrong" \
    'mutated_floor_phases_cover_k_tail_v1' \
    101

printf 'Verus fixture run passed: host and LDS models, 17 expected rejections\n'
