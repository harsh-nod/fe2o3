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
readonly expected_proof_sha=d3eb7a0ee4182ac34d1b9324626243422420b9d43abbfdd63bbbe652ed44223d
readonly proof="$root/crates/fe2o3-mir-model/verus/source_mir_scalar_refinement_v1.rs"
readonly negative_dir="$root/crates/fe2o3-mir-model/verus/negative"
readonly closure_checker="$root/examples/row_softmax_v1/verify-verus-closure.sh"
readonly closure_manifest="$root/examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-source-mir-refinement.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
printf '%s  %s\n' "$expected_proof_sha" "$proof" | sha256sum -c -
printf '%s  %s\n' \
    0c6772e2b8982b4e21cda9dcbb4d109b8d31f525bef64a11669f2066683fb2f9 \
    "$negative_dir/source_mir_wrong_effect_v1.rs" \
    7ad93baedbffa1c7c277793fae0cc4bdcfdee03fa5731feea3eef939bed1b9ae \
    "$negative_dir/source_mir_wrong_operator_v1.rs" \
    a478ad5c28efeb69fa89ced35ada43659b63dadb60b17c1ecf2a10a02fd00e06 \
    "$negative_dir/source_mir_wrong_source_v1.rs" \
    9ce9714ed02ea46e7c69c064eb7917be95158d54c75d6b1286e1a1861d7f29a3 \
    "$negative_dir/source_mir_wrong_type_v1.rs" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
"$closure_checker" "$(dirname -- "$(readlink -f -- "$verus")")" "$closure_manifest"

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -R -F "$token" "$proof" "$negative_dir" >/dev/null; then
        printf 'forbidden trust token %s in source-to-MIR proof sources\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/proof.log" 2>&1
grep -F 'verification results:: 3 verified, 0 errors' "$tmp/proof.log" >/dev/null

for name in source_mir_wrong_operator_v1 source_mir_wrong_type_v1 source_mir_wrong_effect_v1 source_mir_wrong_source_v1; do
    file="$negative_dir/$name.rs"
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$file" \
        >"$tmp/$name.log" 2>&1; then
        printf 'negative Verus fixture unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -E 'postcondition not satisfied|assertion failed' "$tmp/$name.log" >/dev/null
done

printf 'source-to-MIR u32 element refinement: output/effect theorem; 4 expected rejections\n'
