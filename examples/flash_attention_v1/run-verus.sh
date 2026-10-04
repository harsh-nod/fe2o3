#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/flash_attention_v1.rs"
kernel="$script_dir/src/kernel.rs"
negative_dir="$script_dir/verus/negative"
source_checker="$script_dir/verus/check-proof-source.py"
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
expected_checker=$(read_pin "$script_dir/verus/SOURCE_CHECKER_SHA256")
expected_closure=$(read_pin "$script_dir/verus/VERUS_CLOSURE_MANIFEST_SHA256")
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
check_digest "$expected_checker" "$source_checker"
check_digest "$expected_closure" "$closure_manifest"

profile_identity='fe2o3.flash_attention_v1.causal.qkv_f32.b1_h1_n8_d16.row_major.scale_0p25.gfx942_xnack_minus.wave64'
actual_profile=$(printf '%s' "$profile_identity" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_profile" != "$expected_profile" ]; then
    printf 'FAIL: exact FlashAttention profile identity drifted\n' >&2
    exit 1
fi
model_schema='fe2o3.flash_attention_verus_v1.exact_rational.b1_h1_n8_d16.causal_online_max_rescale_sum_numerator_output_ownership'
actual_schema=$(printf '%s' "$model_schema" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_schema" != "$expected_schema" ]; then
    printf 'FAIL: exact mathematical model schema identity drifted\n' >&2
    exit 1
fi

negative_count=$(wc -l <"$script_dir/verus/NEGATIVE_SHA256" | tr -d '[:space:]')
if [ "$negative_count" -ne 10 ]; then
    printf 'FAIL: expected exactly ten independently pinned mutations\n' >&2
    exit 1
fi
(cd "$script_dir/verus" && "$sha256_path" -c NEGATIVE_SHA256 >/dev/null)
"$source_checker" "$proof" "$negative_dir"/*.rs

for marker in \
    'pub proof fn exact_evidence_identity_is_admitted_v1' \
    'pub proof fn exact_profile_dimensions_and_extent_v1' \
    'pub proof fn future_keys_are_excluded_v1' \
    'pub proof fn causal_qkv_indices_are_bounded_v1' \
    'pub proof fn distinct_lane_slots_have_distinct_outputs_v1' \
    'pub proof fn every_output_has_exact_owner_v1' \
    'pub uninterp spec fn exp_weight_v1' \
    'pub proof fn maximum_frame_update_bounds_both_v1' \
    'pub proof fn online_step_preserves_sum_and_numerator_v1' \
    'pub proof fn online_denominator_is_nonzero_v1' \
    'pub proof fn online_state_matches_causal_reference_v1' \
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-flash-attention-verus.XXXXXX")
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
    printf 'FAIL: positive FlashAttention mathematical proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 25 verified, 0 errors' "$positive_log"; then
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

check_negative causal_mask_wrong mutated_future_key_is_excluded_v1
check_negative max_rescaling_wrong mutated_running_max_bounds_next_score_v1
check_negative denominator_wrong mutated_denominator_rescales_old_frame_v1
check_negative numerator_wrong mutated_numerator_weights_current_value_v1
check_negative indexing_wrong mutated_last_tensor_coordinate_is_exact_v1
check_negative dimensions_wrong mutated_sequence_is_exact_profile_v1
check_negative output_ownership_wrong mutated_lane_slots_have_distinct_outputs_v1
check_negative profile_identity_substitution mutated_profile_identity_is_still_exact_v1
check_negative source_identity_substitution mutated_source_identity_is_still_exact_v1
check_negative model_identity_substitution mutated_model_identity_is_still_exact_v1

check_digest "$expected_model" "$proof"
check_digest "$expected_kernel" "$kernel"
check_digest "$expected_checker" "$source_checker"
check_digest "$expected_closure" "$closure_manifest"
"$closure_checker" "$verus_root" "$closure_manifest"
printf 'FE2O3_FLASH_ATTENTION_V1_VERUS_OK mutations=10 obligations=25\n'
