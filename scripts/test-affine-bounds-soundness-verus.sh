#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
proof="$repo_root/crates/fe2o3-proof-contracts/verus/affine_bounds_v1.rs"
negative="$repo_root/crates/fe2o3-proof-contracts/verus/negative/affine_bounds_tightened_extent_v1.rs"
pin_dir="$repo_root/crates/fe2o3-runtime-model/verus/pins"
closure_manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"
closure_checker="$repo_root/examples/row_softmax_v1/verify-verus-closure.sh"
verus_request=${VERUS:?set VERUS to the pinned Verus executable}
timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}

case "$verus_request" in
    /*) verus=$verus_request ;;
    *) printf 'VERUS must be an absolute path\n' >&2; exit 2 ;;
esac

case "$timeout_seconds" in
    ''|*[!0-9]*) printf 'VERUS_TIMEOUT_SECONDS must be a positive integer\n' >&2; exit 2 ;;
esac
if [ "$timeout_seconds" -lt 1 ] || [ "$timeout_seconds" -gt 300 ]; then
    printf 'VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2
    exit 2
fi

expected_version=$(sed -n '1p' "$pin_dir/VERUS_VERSION")
expected_verus=$(sed -n '1p' "$pin_dir/VERUS_SHA256")
actual_verus=$(sha256sum "$verus" | awk '{ print $1 }')
if [ "$actual_verus" != "$expected_verus" ]; then
    printf 'Verus SHA-256 substitution: expected %s, found %s\n' \
        "$expected_verus" "$actual_verus" >&2
    exit 1
fi
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
"$closure_checker" "$(dirname -- "$verus")" "$closure_manifest" >/dev/null

printf '%s  %s\n' \
    a47846101609abf9e1f2d89d229550fc4a6b19a80feccc75c8fa2c557648625b "$proof" \
    a97c0848da62a42b21a7d1736e93f2dfdd9e071b2b80c28efd3facc3f0cae252 "$negative" \
    | sha256sum -c - >/dev/null

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$proof" "$negative" >/dev/null; then
        printf 'forbidden trust token %s in affine-bounds proof sources\n' "$token" >&2
        exit 1
    fi
done

if ! positive_output=$(timeout "$timeout_seconds" "$verus" \
    --crate-type lib --triggers-mode silent "$proof" 2>&1); then
    printf '%s\n' "$positive_output" >&2
    exit 1
fi
printf '%s\n' "$positive_output" \
    | grep -F 'verification results:: 5 verified, 0 errors' >/dev/null

if negative_output=$(timeout "$timeout_seconds" "$verus" \
    --crate-type lib --triggers-mode silent "$negative" 2>&1); then
    printf 'tightened-extent mutation unexpectedly verified\n' >&2
    exit 1
fi
printf '%s\n' "$negative_output" | grep -F 'postcondition not satisfied' >/dev/null

printf '%s\n' \
    'affine bounds soundness: 5 verified, 0 errors; 1 hostile mutation rejected'
