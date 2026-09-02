#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
proof="$root/crates/fe2o3-proof-contracts/verus/constrained_affine_bounds_v2.rs"
negative_dir="$root/crates/fe2o3-proof-contracts/verus/negative"
pin_dir="$root/crates/fe2o3-runtime-model/verus/pins"
manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"
closure_checker="$root/examples/row_softmax_v1/verify-verus-closure.sh"
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
printf '%s  %s\n' \
    f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01 "$manifest" \
    61c687297864074796b97c0a95a619955c186a2f62007157e3d8f1af17ec6aec "$proof" \
    60cc97adf5572b27357673385cd9b6fb3f59170f8584853e3776e647a061cca6 "$negative_dir/constrained_affine_constraint_substitution_v2.rs" \
    26285432cb559c2c972311a96c82f8c887591c4a9d04d5da64501eb734b6bc5f "$negative_dir/constrained_affine_invalid_witness_v2.rs" \
    4cc89255c19bf0a29b805b4053301d62cfbcb428cfe4a984f2a65fc1111f265f "$negative_dir/constrained_affine_removed_edge_bypass_v2.rs" \
    4d90fb0a4c97d887bba48cb9c3063d7c3270757cff243bb2267d7e7055ddadab "$negative_dir/constrained_affine_tightened_extent_v2.rs" \
    | sha256sum -c - >/dev/null
"$closure_checker" "$(dirname -- "$(readlink -f -- "$verus")")" "$manifest" >/dev/null

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$proof" "$negative_dir"/constrained_affine_*_v2.rs >/dev/null; then
        printf 'forbidden trust token %s in constrained affine proof sources\n' "$token" >&2
        exit 1
    fi
done

if ! positive=$(timeout "$timeout_seconds" "$verus" \
    --crate-type lib --triggers-mode silent "$proof" 2>&1); then
    printf '%s\n' "$positive" >&2
    exit 1
fi
printf '%s\n' "$positive" \
    | grep -F 'verification results:: 19 verified, 0 errors' >/dev/null

mutations=0
for negative in "$negative_dir"/constrained_affine_*_v2.rs; do
    if output=$(timeout "$timeout_seconds" "$verus" \
        --crate-type lib --triggers-mode silent "$negative" 2>&1); then
        printf 'hostile constrained affine mutation unexpectedly verified: %s\n' \
            "$negative" >&2
        exit 1
    fi
    printf '%s\n' "$output" | grep -F 'postcondition not satisfied' >/dev/null
    mutations=$((mutations + 1))
done
[ "$mutations" -eq 4 ]

printf '%s\n' \
    'constrained affine bounds V2: 19 verified, 0 errors; 4 hostile mutations rejected'
