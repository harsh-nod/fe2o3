#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
proof="$root/crates/fe2o3-proof-contracts/verus/dynamic_constrained_affine_bounds_v3.rs"
v2_dependency="$root/crates/fe2o3-proof-contracts/verus/constrained_affine_bounds_v2.rs"
negative_dir="$root/crates/fe2o3-proof-contracts/verus/negative"
pin_dir="$root/crates/fe2o3-runtime-model/verus/pins"
manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"
closure_checker="$root/examples/row_softmax_v1/verify-verus-closure.sh"
verus_request=${VERUS:?set VERUS to the pinned runtime Verus executable}
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
    61c687297864074796b97c0a95a619955c186a2f62007157e3d8f1af17ec6aec "$v2_dependency" \
    5b452b460e1028519bfda74b4a84067aec2c958d09294d536485bafe8f2fe0af "$proof" \
    c82dfb4d1da50fb785e115f6b2000414a918be5579ef4c9fddfef97a8b7f2c31 "$negative_dir/dynamic_affine_duplicate_symbol_v3.rs" \
    fc20919b9fe9ebf794b5d5d7335362393ea6edf60eb614da0034ae0ccd291a89 "$negative_dir/dynamic_affine_guard_bypass_v3.rs" \
    7565b64ccffd591596f2e1c753af9f0dd8204340d25b9185d3b33400cc9760c3 "$negative_dir/dynamic_affine_off_by_one_v3.rs" \
    a3a205be9d1f814ebfdd126c304051ac2d4e9d48a66961e7b0656c517b6fbcde "$negative_dir/dynamic_affine_single_guard_v3.rs" \
    | sha256sum -c - >/dev/null
"$closure_checker" "$(dirname -- "$(readlink -f -- "$verus")")" "$manifest" >/dev/null

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$proof" "$v2_dependency" "$negative_dir"/dynamic_affine_*_v3.rs >/dev/null; then
        printf 'forbidden trust token %s in dynamic affine proof sources\n' "$token" >&2
        exit 1
    fi
done

if ! positive=$(timeout "$timeout_seconds" "$verus" \
    --crate-type lib --triggers-mode silent "$proof" 2>&1); then
    printf '%s\n' "$positive" >&2
    exit 1
fi
printf '%s\n' "$positive" \
    | grep -F 'verification results:: 24 verified, 0 errors' >/dev/null

mutations=0
for negative in "$negative_dir"/dynamic_affine_*_v3.rs; do
    if output=$(timeout "$timeout_seconds" "$verus" \
        --crate-type lib --triggers-mode silent "$negative" 2>&1); then
        printf 'hostile dynamic affine mutation unexpectedly verified: %s\n' \
            "$negative" >&2
        exit 1
    fi
    printf '%s\n' "$output" | grep -F 'postcondition not satisfied' >/dev/null
    mutations=$((mutations + 1))
done
[ "$mutations" -eq 4 ]

printf '%s\n' \
    'dynamic constrained affine bounds V3: 24 verified, 0 errors; 4 hostile mutations rejected'
