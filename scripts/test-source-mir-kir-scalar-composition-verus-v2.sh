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
readonly proof="$root/crates/fe2o3-lower-mir-kernel/verus/source_mir_kir_scalar_composition_v2.rs"
readonly negative_dir="$root/crates/fe2o3-lower-mir-kernel/verus/negative"
readonly closure_manifest="$root/crates/fe2o3-lower-mir-kernel/verus/pins/VERUS_CLOSURE_MANIFEST"
readonly closure_checker="$root/examples/row_softmax_v1/verify-verus-closure.sh"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-source-mir-kir-composition.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
"$closure_checker" "$(dirname -- "$(readlink -f -- "$verus")")" "$closure_manifest" >/dev/null
printf '%s  %s\n' \
    d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019 "$closure_manifest" \
    a140e610d67925cf32931ec22e7a7f58b0c45aae3392d04f38697b9b764ef26b "$proof" \
    fb22cdb029ba070990c63dc9b119c815b37bc09f02def377352b371362af6720 "$negative_dir/source_mir_kir_cross_owner_v2.rs" \
    6887b9a5d40bdea1190000cdb9f5b0fb9a6f05bc4d95eb6cae352ee7e4f09374 "$negative_dir/source_mir_kir_swapped_operands_v2.rs" \
    302788b40802915eb10c59896bd394398b461b3721989d5e7d80ce1b47f5c9b2 "$negative_dir/source_mir_kir_wrong_destination_v2.rs" \
    b4609f419781b0273f2946548280e9f5468266373e5b08b6b45682afee6202ee "$negative_dir/source_mir_kir_wrong_parameter_v2.rs" \
    | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -R -F "$token" "$proof" \
        "$negative_dir"/source_mir_kir_*_v2.rs >/dev/null; then
        printf 'forbidden trust token %s in source-to-KIR composition sources\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/proof.log" 2>&1
grep -F 'verification results:: 5 verified, 0 errors' "$tmp/proof.log" >/dev/null

for name in \
    source_mir_kir_cross_owner_v2 \
    source_mir_kir_swapped_operands_v2 \
    source_mir_kir_wrong_destination_v2 \
    source_mir_kir_wrong_parameter_v2
do
    file="$negative_dir/$name.rs"
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$file" \
        >"$tmp/$name.log" 2>&1; then
        printf 'negative Verus fixture unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -E 'postcondition not satisfied|assertion failed' "$tmp/$name.log" >/dev/null
done

printf 'Source-to-MIR-to-KIR u32 parameter composition: 5 obligations; 4 expected rejections\n'
