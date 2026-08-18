#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
proof="$script_dir/runtime_lifecycle_v1.rs"
negative="$script_dir/negative/runtime_lifecycle_v1_release_while_published.rs"
verus_bin=${VERUS:-verus}

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

case "$verus_bin" in
    */*) [ -x "$verus_bin" ] || { printf 'FAIL: VERUS is not executable: %s\n' "$verus_bin" >&2; exit 1; } ;;
    *) verus_bin=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_bin" ]; then
    printf 'FAIL: Verus is unavailable; set VERUS=/absolute/path/to/verus\n' >&2
    exit 1
fi

for source in "$proof" "$negative"; do
    if grep -Eq 'assume[[:space:]]*\(|admit[[:space:]]*\(|external_body|external_fn_specification|uninterp[[:space:]]+spec' "$source"; then
        printf 'FAIL: forbidden trusted construct in %s\n' "$source" >&2
        exit 1
    fi
done

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-model-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

positive_log="$tmp_dir/positive.log"
if ! "$verus_bin" --crate-type lib --triggers-mode silent "$proof" >"$positive_log" 2>&1; then
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
if "$verus_bin" --crate-type lib --triggers-mode silent "$negative" >"$negative_log" 2>&1; then
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
