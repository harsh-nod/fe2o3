#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/row_softmax_v1.rs"
negative_dir="$script_dir/verus/negative"
version_file="$script_dir/verus/VERUS_VERSION"
sha256_file="$script_dir/verus/VERUS_SHA256"
closure_manifest="$script_dir/verus/VERUS_CLOSURE_MANIFEST"
source_checker="$script_dir/check-proof-source.sh"
closure_checker="$script_dir/verify-verus-closure.sh"

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

for marker in \
    'pub uninterp spec fn exp_real_v1' \
    'pub open spec fn stable_softmax_spec_v1' \
    'pub open spec fn denominator_state_v1' \
    'pub proof fn active_lane_indices_are_in_bounds_v1' \
    'pub proof fn separate_input_and_output_accesses_do_not_alias_v1' \
    'pub proof fn distinct_output_element_addresses_v1' \
    'pub proof fn distinct_scratch_element_addresses_v1' \
    'pub proof fn denominator_reduction_step_preserves_state_v1' \
    'pub proof fn stable_softmax_spec_preserves_lane_numerator_correspondence_v1' \
    'pub proof fn positive_weight_premises_give_positive_denominator_v1' \
    'pub proof fn finite_numerator_premises_give_positive_lane_v1' \
    'pub proof fn finite_numerator_premises_transport_sum_to_denominator_v1' \
    'pub proof fn stable_softmax_spec_premises_give_positive_denominator_v1'
do
    require_source "$proof" "$marker"
done

require_source "$negative_dir/lane_plus_one_out_of_bounds.rs" \
    'mutated_lane_plus_one_is_bounded_v1'
require_source "$negative_dir/duplicate_writer.rs" \
    'mutated_output_ownership_is_injective_v1'
require_source "$negative_dir/wrong_numerator_index.rs" \
    'mutated_stable_softmax_spec_preserves_lane_numerator_correspondence_v1'

"$source_checker" "$proof" "$negative_dir"/*.rs

expected_version=$(sed -n '1p' "$version_file")
expected_sha256=$(sed -n '1p' "$sha256_file")
case "$expected_version" in
    ''|*[!0-9A-Za-z.-]*) printf 'FAIL: invalid pinned Verus version\n' >&2; exit 1 ;;
esac
case "$expected_sha256" in
    *[!0-9a-f]*|'') printf 'FAIL: invalid pinned Verus SHA-256\n' >&2; exit 1 ;;
esac
if [ "${#expected_sha256}" -ne 64 ]; then
    printf 'FAIL: pinned Verus SHA-256 must contain 64 hex digits\n' >&2
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
timeout_path=$(command -v timeout 2>/dev/null || true)
if [ -z "$sha256_path" ] || [ -z "$timeout_path" ]; then
    printf 'FAIL: sha256sum and timeout are required\n' >&2
    exit 1
fi
actual_sha256=$("$sha256_path" "$verus_path" | awk '{ print $1 }')
if [ "$actual_sha256" != "$expected_sha256" ]; then
    printf 'FAIL: Verus executable SHA-256 %s does not match pinned %s\n' \
        "${actual_sha256:-unknown}" "$expected_sha256" >&2
    exit 1
fi
readlink_path=$(command -v readlink 2>/dev/null || true)
if [ -z "$readlink_path" ]; then
    printf 'FAIL: readlink is required to locate the complete Verus release closure\n' >&2
    exit 1
fi
verus_path=$("$readlink_path" -f "$verus_path")
if [ "$(basename "$verus_path")" != verus ]; then
    printf 'FAIL: pinned Verus launcher must be named verus inside its release closure\n' >&2
    exit 1
fi
verus_root=$(CDPATH='' cd -- "$(dirname -- "$verus_path")" && pwd)
"$closure_checker" "$verus_root" "$closure_manifest"

env_path=$(command -v env 2>/dev/null || true)
if [ -z "$env_path" ]; then
    printf 'FAIL: env is required to isolate Verus execution\n' >&2
    exit 1
fi
runner_home=${HOME:-/nonexistent}
runner_path=${PATH:-/usr/local/bin:/usr/bin:/bin}
runner_rustup_home=${RUSTUP_HOME:-"$runner_home/.rustup"}
runner_cargo_home=${CARGO_HOME:-"$runner_home/.cargo"}

actual_version=$(
    "$env_path" -i \
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-row-softmax-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    "$timeout_path" --foreground --signal=TERM --kill-after=5 \
        "$timeout_seconds" \
        "$env_path" -i \
        "HOME=$runner_home" \
        "PATH=$runner_path" \
        "RUSTUP_HOME=$runner_rustup_home" \
        "CARGO_HOME=$runner_cargo_home" \
        "VERUS_Z3_PATH=$verus_root/z3" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

positive_log="$tmp_dir/positive.log"
if run_verus "$proof" >"$positive_log" 2>&1; then
    :
else
    status=$?
    printf 'FAIL: positive row-softmax proof did not verify (status %s)\n' "$status" >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 18 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: positive proof emitted an unexpected verification summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
cat "$positive_log"

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
        printf 'FAIL: %s exceeded the %ss Verus time limit\n' "$name" "$timeout_seconds" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log" || \
        ! grep -Fq 'error: postcondition not satisfied' "$log" || \
        ! grep -Eq '^verification results:: [0-9]+ verified, 1 errors$' "$log"; then
        printf 'FAIL: %s did not fail only its pinned postcondition\n' "$name" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'XFAIL: %s rejected at %s\n' "$name" "$marker"
}

run_rejected lane_plus_one_out_of_bounds \
    "$negative_dir/lane_plus_one_out_of_bounds.rs" \
    'mutated_lane_plus_one_is_bounded_v1'
run_rejected duplicate_writer "$negative_dir/duplicate_writer.rs" \
    'mutated_output_ownership_is_injective_v1'
run_rejected wrong_numerator_index "$negative_dir/wrong_numerator_index.rs" \
    'mutated_stable_softmax_spec_preserves_lane_numerator_correspondence_v1'

printf 'PASS: row-softmax V1 proof and 3 expected rejections\n'
