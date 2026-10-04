#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/moe_top2_v1.rs"
kernel="$script_dir/src/kernel.rs"
negative_dir="$script_dir/verus/negative"
source_checker="$script_dir/../wave64_collectives_v1/check-proof-source.py"
closure_checker="$script_dir/../row_softmax_v1/verify-verus-closure.sh"
closure_manifest="$script_dir/../row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST"

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

expected_model=$(read_pin "$script_dir/verus/MODEL_SHA256")
expected_kernel=$(read_pin "$script_dir/verus/KERNEL_SHA256")
expected_profile=$(read_pin "$script_dir/verus/PROFILE_IDENTITY_SHA256")
expected_schema=$(read_pin "$script_dir/verus/MODEL_SCHEMA_SHA256")
expected_verus=$(read_pin "$script_dir/verus/VERUS_SHA256")
expected_version=$(sed -n '1p' "$script_dir/verus/VERUS_VERSION")
case "$expected_version" in
    ''|*[!0-9A-Za-z.-]*) printf 'FAIL: invalid pinned Verus version\n' >&2; exit 1 ;;
esac

sha256_path=$(command -v sha256sum 2>/dev/null || true)
timeout_path=$(command -v timeout 2>/dev/null || true)
if [ -z "$sha256_path" ] || [ -z "$timeout_path" ]; then
    printf 'FAIL: sha256sum and timeout are required\n' >&2
    exit 1
fi

check_digest() {
    actual=$("$sha256_path" "$2" | awk '{ print $1 }')
    if [ "$actual" != "$1" ]; then
        printf 'FAIL: SHA-256 substitution for %s: got %s, expected %s\n' \
            "$2" "$actual" "$1" >&2
        exit 1
    fi
}

check_digest "$expected_model" "$proof"
check_digest "$expected_kernel" "$kernel"

profile_identity='fe2o3.moe_top2_v1.logits_f32.t8_e4_k2.capacity4.token_major.lower_expert_ties.stable_drop.gfx942_xnack_minus.wave64'
actual_profile=$(printf '%s' "$profile_identity" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_profile" != "$expected_profile" ]; then
    printf 'FAIL: exact profile identity does not match its pinned namespace\n' >&2
    exit 1
fi
model_schema='fe2o3.moe_top2_verus_v1.int_scores.t8_e4_k2_c4.top2_counts_scan_stable_pack_inverse'
actual_schema=$(printf '%s' "$model_schema" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_schema" != "$expected_schema" ]; then
    printf 'FAIL: exact mathematical model schema identity drifted\n' >&2
    exit 1
fi

(cd "$script_dir/verus" && "$sha256_path" -c NEGATIVE_SHA256 >/dev/null)
"$source_checker" "$proof" "$negative_dir"/*.rs

for marker in \
    'pub proof fn exact_evidence_identity_is_admitted_v1' \
    'pub proof fn exact_top2_pair_is_deterministic_v1' \
    'pub proof fn exact_selection_has_two_ordered_distinct_experts_v1' \
    'pub proof fn output_counts_capacity_and_scan_are_exact_v1' \
    'pub proof fn exact_routing_state_joins_selection_counts_and_packing_v1' \
    'pub proof fn stable_prefix_acceptance_and_drop_v1' \
    'pub proof fn accepted_route_slots_are_unique_v1' \
    'pub proof fn accepted_permutation_inverse_round_trip_v1' \
    'pub proof fn dropped_routes_and_permutation_tail_are_sentinels_v1' \
    'pub proof fn assurance_boundary_is_explicit_v1'
do
    if ! grep -Fq "$marker" "$proof"; then
        printf 'FAIL: pinned proof marker is missing: %s\n' "$marker" >&2
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
check_digest "$expected_verus" "$verus_path"
verus_path=$(readlink -f "$verus_path")
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
    printf 'FAIL: Verus version %s does not match pinned %s\n' \
        "${actual_version:-unknown}" "$expected_version" >&2
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-moe-top2-verus.XXXXXX")
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
    printf 'FAIL: positive MoE top-2 mathematical proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 28 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: positive proof emitted an unexpected verification summary\n' >&2
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
    if ! grep -Fq "$marker" "$log" || \
        ! grep -Fq 'error: postcondition not satisfied' "$log" || \
        ! grep -Fq 'verification results:: 0 verified, 1 errors' "$log"; then
        printf 'FAIL: expected-negative proof failed at an unexpected surface: %s\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at %s\n' "$name" "$marker"
}

check_negative profile_identity_substitution mutated_profile_identity_is_still_exact_v1
check_negative model_identity_substitution mutated_model_identity_is_still_exact_v1
check_negative top2_tie_order_wrong mutated_lower_expert_wins_equal_score_v1
check_negative request_capacity_wrong mutated_admission_respects_capacity_v1
check_negative exclusive_scan_wrong mutated_terminal_offset_is_route_bounded_v1
check_negative stable_prefix_wrong mutated_rank_four_is_dropped_v1
check_negative slot_uniqueness_wrong mutated_accepted_slots_are_unique_v1
check_negative inverse_round_trip_wrong mutated_route_one_round_trips_v1
check_negative sentinel_tail_wrong mutated_unused_tail_is_sentinel_v1

printf 'FE2O3_MOE_TOP2_V1_VERUS_OK\n'
