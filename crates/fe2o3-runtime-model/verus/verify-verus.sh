#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../../.." && pwd)
proof="$script_dir/runtime_lifecycle_v1.rs"
negative="$script_dir/negative/runtime_lifecycle_v1_release_while_published.rs"
pin_dir="$script_dir/pins"
closure_manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"
closure_checker="$repo_root/examples/row_softmax_v1/verify-verus-closure.sh"
verus_bin=${VERUS:-verus}

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
expected_negative=$(read_pin "$pin_dir/NEGATIVE_SHA256")
expected_verus=$(read_pin "$pin_dir/VERUS_SHA256")
expected_closure=$(read_pin "$pin_dir/VERUS_CLOSURE_MANIFEST_SHA256")
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
check_digest "$expected_negative" "$negative"
check_digest "$expected_closure" "$closure_manifest"
check_digest 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c' "$closure_checker"

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

for source in "$proof" "$negative"; do
    if grep -Eq 'assume[[:space:]]*\(|admit[[:space:]]*\(|external_body|external_fn_specification|uninterp[[:space:]]+spec' "$source"; then
        printf 'FAIL: forbidden trusted construct in %s\n' "$source" >&2
        exit 1
    fi
done

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-model-verus.XXXXXX")
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
    cat "$positive_log" >&2
    exit 1
fi
if ! grep -Fq 'verification results:: 2 verified, 0 errors' "$positive_log"; then
    printf 'FAIL: unexpected positive verification summary\n' >&2
    cat "$positive_log" >&2
    exit 1
fi
cat "$positive_log"

negative_log="$tmp_dir/negative.log"
if run_verus "$negative" >"$negative_log" 2>&1; then
    printf 'FAIL: release-while-published mutation unexpectedly verified\n' >&2
    exit 1
fi
if ! grep -Fq 'mutated_release_while_published_is_safe_v1' "$negative_log" \
    || ! grep -Fq 'error: postcondition not satisfied' "$negative_log"; then
    printf 'FAIL: mutation failed at an unexpected verification surface\n' >&2
    cat "$negative_log" >&2
    exit 1
fi
printf '%s\n' 'XFAIL: Verus rejected release while a dispatch retains the mapping'
