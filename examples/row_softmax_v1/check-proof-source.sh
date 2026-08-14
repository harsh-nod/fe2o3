#!/bin/sh
set -eu

if [ "$#" -lt 1 ]; then
    printf 'usage: %s PROOF [PROOF ...]\n' "$0" >&2
    exit 2
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-row-softmax-source-check.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

for file in "$@"; do
    if [ ! -f "$file" ]; then
        printf 'FAIL: proof source is not a regular file: %s\n' "$file" >&2
        exit 1
    fi
    normalized="$tmp_dir/normalized"
    LC_ALL=C tr -d '[:space:]' <"$file" >"$normalized"
    for forbidden in 'assume(' 'admit(' '#[verifier::external_body]'; do
        if grep -Fq "$forbidden" "$normalized"; then
            printf 'FAIL: %s contains forbidden normalized construct %s\n' \
                "$file" "$forbidden" >&2
            exit 1
        fi
    done
done
