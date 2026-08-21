#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/verus/rope_kv_v1.rs"

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
expected_verus=$(read_pin "$script_dir/verus/VERUS_SHA256")
expected_version=$(sed -n '1p' "$script_dir/verus/VERUS_VERSION")
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
        printf 'FAIL: SHA-256 substitution for %s: got %s, expected %s\n' \
            "$2" "$actual" "$1" >&2
        exit 1
    fi
}

check_digest "$expected_model" "$proof"
if grep -En '(^|[^[:alnum:]_])(assume|admit|external_body)([^[:alnum:]_]|$)' "$proof" >/dev/null; then
    printf 'FAIL: proof contains a forbidden trust token\n' >&2
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
verus_path=$("$readlink_path" -f "$verus_path")
check_digest "$expected_verus" "$verus_path"
verus_root=$(CDPATH='' cd -- "$(dirname -- "$verus_path")" && pwd)

actual_version=$(
    VERUS_Z3_PATH="$verus_root/z3" "$verus_path" --version \
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

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-qwen3-rope-kv-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM
log="$tmp_dir/positive.log"
if ! "$timeout_path" --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
    env VERUS_Z3_PATH="$verus_root/z3" \
    "$verus_path" --crate-type lib --triggers-mode silent --no-cheating "$proof" \
    >"$log" 2>&1; then
    printf 'FAIL: Qwen3 RoPE/KV conditional proof did not verify\n' >&2
    cat "$log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 14 verified, 0 errors' "$log"; then
    printf 'FAIL: proof emitted an unexpected verification summary\n' >&2
    cat "$log" >&2
    exit 1
fi
cat "$log"
check_digest "$expected_model" "$proof"
check_digest "$expected_verus" "$verus_path"
printf 'FE2O3_QWEN3_ROPE_KV_V1_VERUS_OK obligations=14\n'
