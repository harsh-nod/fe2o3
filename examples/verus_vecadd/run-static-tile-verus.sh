#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
require_verus=0
source_only=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --require) require_verus=1 ;;
        --source-only) source_only=1 ;;
        *)
            printf 'usage: %s [--require] [--source-only]\n' "$0" >&2
            exit 2
            ;;
    esac
    shift
done

positive="$script_dir/verus/static_tile.rs"
negative="$script_dir/verus/negative/static_tile_out_of_bounds.rs"
for marker in \
    'pub proof fn checked_tile_constant_access_is_in_allocation' \
    'pub proof fn different_parent_witness_is_rejected'
do
    grep -Fq "$marker" "$positive" || {
        printf 'FAIL: missing positive marker %s\n' "$marker" >&2
        exit 1
    }
done
grep -Fq 'mutated_static_tile_out_of_bounds_is_safe' "$negative" || {
    printf 'FAIL: missing negative marker\n' >&2
    exit 1
}
for forbidden in 'admit(' 'assume(false' '#[verifier::external_body]'; do
    if grep -Fq "$forbidden" "$positive" "$negative"; then
        printf 'FAIL: forbidden proof shortcut %s\n' "$forbidden" >&2
        exit 1
    fi
done
printf 'PASS: static-tile positive and negative proof source shapes are paired\n'

if [ "$source_only" -eq 1 ]; then
    exit 0
fi

verus_bin=${VERUS:-verus}
case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'SKIP: Verus is unavailable (set VERUS=/path/to/verus)\n'
    [ "$require_verus" -eq 0 ] || exit 1
    exit 0
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-static-tile-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

"$verus_path" --crate-type lib --triggers-mode silent "$positive" \
    >"$tmp_dir/positive.log" 2>&1 || {
        printf 'FAIL: static-tile model did not verify\n' >&2
        cat "$tmp_dir/positive.log" >&2
        exit 1
    }
printf 'PASS: static-tile model verified\n'

if "$verus_path" --crate-type lib --triggers-mode silent "$negative" \
    >"$tmp_dir/negative.log" 2>&1
then
    printf 'FAIL: out-of-bounds mutation unexpectedly verified\n' >&2
    exit 1
fi
grep -Fq 'mutated_static_tile_out_of_bounds_is_safe' "$tmp_dir/negative.log" || {
    printf 'FAIL: negative result omitted the expected marker\n' >&2
    cat "$tmp_dir/negative.log" >&2
    exit 1
}
grep -Eiq 'postcondition.*not satisfied|postcondition failure' "$tmp_dir/negative.log" || {
    printf 'FAIL: negative result was not the expected proof failure\n' >&2
    cat "$tmp_dir/negative.log" >&2
    exit 1
}
printf 'XFAIL: out-of-bounds static-tile mutation rejected\n'
