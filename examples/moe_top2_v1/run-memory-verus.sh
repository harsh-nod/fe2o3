#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/moe_top2_memory_v1.rs"
kernel="$script_dir/src/kernel.rs"
negative_dir="$script_dir/verus/memory_negative"
negative_manifest="$script_dir/verus/MEMORY_NEGATIVE_SHA256"
source_checker="$script_dir/../flash_attention_v1/verus/check-proof-source.py"
closure_checker="$script_dir/../row_softmax_v1/verify-verus-closure.sh"
closure_manifest="$script_dir/verus/MEMORY_VERUS_CLOSURE_MANIFEST"

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

expected_proof=$(read_pin "$script_dir/verus/MEMORY_MODEL_SHA256")
expected_verus=$(read_pin "$script_dir/verus/MEMORY_VERUS_SHA256")
expected_closure=$(read_pin "$script_dir/verus/MEMORY_VERUS_CLOSURE_MANIFEST_SHA256")
expected_transcript=$(read_pin "$script_dir/verus/MEMORY_TRANSCRIPT_SHA256")
expected_version=$(sed -n '1p' "$script_dir/verus/MEMORY_VERUS_VERSION")
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

check_digest "$expected_proof" "$proof"
check_digest '0e4570bd52866dd23b8b00d83983aadc818c77580de8f7f5e2982e12a57e20e2' "$kernel"
check_digest 'a2cf9bebabb0a95b0b8c23586b1fe120a3d8571d9d7809be8ed9fdd2a035d531' "$source_checker"
check_digest "$expected_closure" "$closure_manifest"

analyzer_profile='fe2o3.moe_top2_v1.physical_machine_effect.gfx942.v1'
actual_analyzer=$(printf '%s' "$analyzer_profile" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_analyzer" != '40bea576eb92b0a196914bf544f3770d2b0757e379e5342c956cb200b4454051' ]; then
    printf 'FAIL: exact analyzer profile identity drifted\n' >&2
    exit 1
fi

negative_count=$(wc -l <"$negative_manifest" | tr -d '[:space:]')
if [ "$negative_count" -ne 8 ]; then
    printf 'FAIL: expected exactly eight independently pinned memory mutations\n' >&2
    exit 1
fi
(cd "$script_dir/verus" && "$sha256_path" -c MEMORY_NEGATIVE_SHA256 >/dev/null)
"$source_checker" "$proof" "$negative_dir"/*.rs

for marker in \
    'pub proof fn exact_evidence_identities_are_admitted_v1' \
    'pub proof fn exact_eight_buffer_extents_v1' \
    'pub proof fn token_logit_index_is_bounded_v1' \
    'pub proof fn token_rank_route_id_is_bounded_v1' \
    'pub proof fn accepted_route_slot_is_bounded_v1' \
    'pub proof fn every_exact_abi_access_is_in_bounds_v1' \
    'pub proof fn pairwise_disjoint_regions_have_distinct_element_addresses_v1' \
    'pub proof fn distinct_output_elements_have_distinct_write_owners_v1' \
    'pub proof fn no_duplicate_external_write_ownership_v1' \
    'pub proof fn stable_routing_phases_precede_output_commit_v1' \
    'pub proof fn assurance_boundary_is_conservative_v1'
do
    if ! grep -Fq "$marker" "$proof"; then
        printf 'FAIL: pinned memory proof marker is missing: %s\n' "$marker" >&2
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-moe-top2-memory-verus.XXXXXX")
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
    printf 'FAIL: positive exact MoE routing memory proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 16 verified, 0 errors' "$positive_log"; then
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

check_negative address_space_wrong mutated_address_space_is_exact_v1
check_negative duplicate_owner_wrong mutated_identical_writes_have_distinct_owners_v1
check_negative effect_order_wrong mutated_output_commit_precedes_permutation_v1
check_negative logit_index_wrong mutated_logit_index_is_bounded_v1
check_negative machine_identity_wrong mutated_machine_identity_is_exact_v1
check_negative offset_extent_wrong mutated_offset_index_is_bounded_v1
check_negative route_value_wrong mutated_drop_route_is_in_range_v1
check_negative slot_bound_wrong mutated_accepted_slot_is_bounded_v1

check_digest "$expected_proof" "$proof"
check_digest "$expected_closure" "$closure_manifest"
(cd "$script_dir/verus" && "$sha256_path" -c MEMORY_NEGATIVE_SHA256 >/dev/null)
"$closure_checker" "$verus_root" "$closure_manifest" >/dev/null
transcript='FE2O3_MOE_TOP2_MEMORY_V1_VERUS_OK mutations=8 obligations=16'
actual_transcript=$(printf '%s' "$transcript" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_transcript" != "$expected_transcript" ]; then
    printf 'FAIL: canonical proof transcript identity drifted\n' >&2
    exit 1
fi
printf '%s\n' "$transcript"
