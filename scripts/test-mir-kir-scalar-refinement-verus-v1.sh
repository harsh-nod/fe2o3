#!/usr/bin/env bash
set -euo pipefail

readonly root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly verus_request=${VERUS:?set VERUS to the pinned Verus executable}
case "$verus_request" in
    */*) readonly verus=$verus_request ;;
    *) readonly verus=$(command -v "$verus_request") ;;
esac
readonly expected_version=0.2026.08.02.b677dd5
readonly expected_verus_sha=ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd
readonly proof="$root/crates/fe2o3-lower-mir-kernel/verus/mir_kir_scalar_refinement_v1.rs"
readonly negative_dir="$root/crates/fe2o3-lower-mir-kernel/verus/negative"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-mir-kir-refinement.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
printf '%s  %s\n' \
    c5dbf8bede7bfe3b4225ac381c4488fe77ca987ba2b4a76d16f3d74616fdd2be "$proof" \
    d132841e3d4d2434dc2dc80083fdc876cf6c764966a65678f524b24c4430271a "$negative_dir/mir_kir_scalar_wrong_effect_v1.rs" \
    48f41d1506736dd4ca3c4fcecf09aa96deb0c2abcc33913b98f8d6d44e2ed99c "$negative_dir/mir_kir_scalar_wrong_operator_v1.rs" \
    | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -R -F "$token" "$proof" "$negative_dir" >/dev/null; then
        printf 'forbidden trust token %s in MIR-to-KIR proof sources\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/proof.log" 2>&1
grep -F '0 errors' "$tmp/proof.log" >/dev/null

for name in mir_kir_scalar_wrong_operator_v1 mir_kir_scalar_wrong_effect_v1; do
    file="$negative_dir/$name.rs"
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$file" \
        >"$tmp/$name.log" 2>&1; then
        printf 'negative Verus fixture unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -E 'postcondition not satisfied|assertion failed' "$tmp/$name.log" >/dev/null
done

printf 'MIR-to-KIR u32 element refinement: output/effect theorem; 2 expected rejections\n'
