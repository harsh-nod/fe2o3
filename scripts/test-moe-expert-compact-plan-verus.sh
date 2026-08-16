#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
proof="$repo_root/crates/fe2o3-verifier/verus/moe_expert_compact_plan_v1.rs"
negative_dir="$repo_root/crates/fe2o3-verifier/verus/moe_expert_compact_plan_v1_negative"
pin_dir="$repo_root/crates/fe2o3-verifier/verus/moe_expert_compact_plan_v1"
negative_manifest="$pin_dir/NEGATIVE_SHA256"
source_checker="$repo_root/examples/wave64_collectives_v1/check-proof-source.py"
closure_checker="$repo_root/examples/row_softmax_v1/verify-verus-closure.sh"
closure_manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

read_pin() {
    value=$(sed -n '1p' "$1")
    case "$value" in
        *[!0-9a-f]*|'') printf 'FAIL: invalid SHA-256 pin in %s\n' "$1" >&2; exit 1 ;;
    esac
    if [ "${#value}" -ne 64 ]; then
        printf 'FAIL: SHA-256 pin in %s must contain 64 hex digits\n' "$1" >&2
        exit 1
    fi
    printf '%s\n' "$value"
}

expected_model=$(read_pin "$pin_dir/MODEL_SHA256")
expected_verus=$(read_pin "$pin_dir/VERUS_SHA256")
expected_closure=$(read_pin "$pin_dir/VERUS_CLOSURE_MANIFEST_SHA256")
expected_transcript=$(read_pin "$pin_dir/TRANSCRIPT_SHA256")
expected_version=$(sed -n '1p' "$pin_dir/VERUS_VERSION")
case "$expected_version" in
    ''|*[!0-9A-Za-z.-]*) printf 'FAIL: invalid pinned Verus version\n' >&2; exit 1 ;;
esac

sha256_path=$(command -v sha256sum 2>/dev/null || true)
timeout_path=$(command -v timeout 2>/dev/null || true)
readlink_path=$(command -v readlink 2>/dev/null || true)
if [ -z "$sha256_path" ] || [ -z "$timeout_path" ] || [ -z "$readlink_path" ]; then
    printf 'FAIL: sha256sum, timeout, and readlink are required\n' >&2
    exit 1
fi

check_digest() {
    actual=$("$sha256_path" "$2" | awk '{ print $1 }')
    if [ "$actual" != "$1" ]; then
        printf 'FAIL: SHA-256 substitution for %s\n' "$2" >&2
        exit 1
    fi
}

check_digest "$expected_model" "$proof"
check_digest "$expected_closure" "$closure_manifest"
check_digest 'a3071ad8f0025a59d0c70dcf2427c1eb43b0c02fe484474a84d0377d99ccb887' "$source_checker"
check_digest 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c' "$closure_checker"

negative_count=$(wc -l <"$negative_manifest" | tr -d '[:space:]')
if [ "$negative_count" -ne 7 ]; then
    printf 'FAIL: expected exactly seven pinned compact-plan mutations\n' >&2
    exit 1
fi
(cd "$pin_dir" && "$sha256_path" -c NEGATIVE_SHA256 >/dev/null)
"$source_checker" "$proof" "$negative_dir"/*.rs

for marker in \
    'pub proof fn exact_compact_shape_is_closed_v1' \
    'pub proof fn valid_offsets_are_route_bounded_v1' \
    'pub proof fn every_expert_count_is_capacity_bounded_v1' \
    'pub proof fn each_source_range_lies_inside_its_expert_tile_v1' \
    'pub proof fn each_source_coordinate_lies_inside_its_expert_tile_v1' \
    'pub proof fn each_destination_range_lies_inside_compact_tile_v1' \
    'pub proof fn each_compact_destination_coordinate_is_bounded_v1' \
    'pub proof fn nonempty_destination_ranges_are_pairwise_disjoint_and_ordered_v1' \
    'pub proof fn destination_union_is_exactly_the_accepted_prefix_v1' \
    'pub proof fn zero_fill_defines_every_unused_tail_value_v1' \
    'pub proof fn compact_plan_assurance_boundary_is_inert_v1'
do
    if ! grep -Fq "$marker" "$proof"; then
        printf 'FAIL: pinned compact-plan proof marker is missing: %s\n' "$marker" >&2
        exit 1
    fi
done

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'FAIL: Verus is unavailable; set VERUS=/absolute/path/to/verus\n' >&2
    exit 1
fi
verus_path=$("$readlink_path" -f "$verus_path")
if [ "$(basename "$verus_path")" != verus ]; then
    printf 'FAIL: pinned Verus executable must be named verus\n' >&2
    exit 1
fi
check_digest "$expected_verus" "$verus_path"
verus_root=$(CDPATH='' cd -- "$(dirname -- "$verus_path")" && pwd)
"$closure_checker" "$verus_root" "$closure_manifest"

runner_home=${HOME:-/nonexistent}
runner_path=${PATH:-/usr/local/bin:/usr/bin:/bin}
runner_rustup_home=${RUSTUP_HOME:-"$runner_home/.rustup"}
runner_cargo_home=${CARGO_HOME:-"$runner_home/.cargo"}
actual_version=$(
    env -i \
        "HOME=$runner_home" \
        "PATH=$runner_path" \
        "RUSTUP_HOME=$runner_rustup_home" \
        "CARGO_HOME=$runner_cargo_home" \
        "VERUS_Z3_PATH=$verus_root/z3" \
        "$verus_path" --version \
        | awk '/^[[:space:]]*Version:/ { print $2; exit }'
)
if [ "$actual_version" != "$expected_version" ]; then
    printf 'FAIL: Verus version does not match the pin\n' >&2
    exit 1
fi

timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
case "$timeout_seconds" in
    ''|*[!0-9]*) printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2; exit 2 ;;
esac
if [ "$timeout_seconds" -lt 1 ] || [ "$timeout_seconds" -gt 300 ]; then
    printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2
    exit 2
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-moe-expert-compact-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    "$timeout_path" --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
        env -i \
        "HOME=$runner_home" \
        "PATH=$runner_path" \
        "RUSTUP_HOME=$runner_rustup_home" \
        "CARGO_HOME=$runner_cargo_home" \
        "VERUS_Z3_PATH=$verus_root/z3" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

positive_log="$tmp_dir/positive.log"
if ! run_verus "$proof" >"$positive_log" 2>&1; then
    printf 'FAIL: positive exact MoE expert compact-plan proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 19 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: positive compact-plan proof emitted an unexpected verification summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
cat "$positive_log"

check_negative() {
    name=$1
    marker=$2
    log="$tmp_dir/$name.log"
    if run_verus "$negative_dir/$name.rs" >"$log" 2>&1; then
        printf 'FAIL: expected-negative compact-plan proof unexpectedly verified: %s\n' "$name" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log" || \
        ! grep -Fq 'error: postcondition not satisfied' "$log" || \
        ! grep -Fq 'verification results:: 0 verified, 1 errors' "$log"; then
        printf 'FAIL: compact-plan mutation failed at an unexpected surface: %s\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at %s\n' "$name" "$marker"
}

check_negative source_range_escapes_expert_tile mutated_source_range_escapes_expert_tile_v1
check_negative destination_index_out_of_bounds mutated_destination_index_out_of_bounds_v1
check_negative destination_ranges_overlap mutated_closed_destination_ranges_overlap_v1
check_negative destination_ranges_reordered mutated_destination_ranges_are_reordered_v1
check_negative destination_union_has_gap mutated_destination_union_has_gap_v1
check_negative unused_tail_is_nonzero mutated_unused_tail_is_nonzero_v1
check_negative expert_count_exceeds_capacity mutated_expert_count_exceeds_capacity_v1

check_digest "$expected_model" "$proof"
check_digest "$expected_closure" "$closure_manifest"
(cd "$pin_dir" && "$sha256_path" -c NEGATIVE_SHA256 >/dev/null)
"$closure_checker" "$verus_root" "$closure_manifest" >/dev/null
transcript='FE2O3_MOE_EXPERT_COMPACT_PLAN_V1_VERUS_OK mutations=7 obligations=19'
actual_transcript=$(printf '%s' "$transcript" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_transcript" != "$expected_transcript" ]; then
    printf 'FAIL: canonical compact-plan proof transcript identity drifted\n' >&2
    exit 1
fi
printf '%s\n' "$transcript"
