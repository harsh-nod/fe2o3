#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/wave64_collectives_v1.rs"
negative_dir="$script_dir/verus/negative"
source_checker="$script_dir/check-proof-source.py"
version_file="$script_dir/verus/VERUS_VERSION"
sha256_file="$script_dir/verus/VERUS_SHA256"
closure_checker="$script_dir/../row_softmax_v1/verify-verus-closure.sh"
closure_manifest="$script_dir/../row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST"

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

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

actual_sha256=$(sha256sum "$verus_path" | awk '{ print $1 }')
if [ "$actual_sha256" != "$expected_sha256" ]; then
    printf 'FAIL: Verus executable SHA-256 %s does not match pinned %s\n' \
        "${actual_sha256:-unknown}" "$expected_sha256" >&2
    exit 1
fi
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-wave64-collectives-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    timeout --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
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
    printf 'FAIL: positive Wave64 collective proof did not verify\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 12 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: positive proof emitted an unexpected verification summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
cat "$positive_log"

for negative in \
    active_exclusion_wrong \
    bounds_wrong \
    ownership_wrong \
    reduction_wrong \
    scan_recurrence_wrong
do
    source="$negative_dir/$negative.rs"
    log="$tmp_dir/$negative.log"
    if run_verus "$source" >"$log" 2>&1; then
        printf 'FAIL: expected-negative proof unexpectedly verified: %s\n' "$negative" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq 'verification results::' "$log" || ! grep -Eq '[1-9][0-9]* errors' "$log"; then
        printf 'FAIL: expected-negative proof had an unexpected failure: %s\n' "$negative" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'expected-negative rejected: %s\n' "$negative"
done

printf 'FE2O3_WAVE64_COLLECTIVES_V1_VERUS_OK\n'
