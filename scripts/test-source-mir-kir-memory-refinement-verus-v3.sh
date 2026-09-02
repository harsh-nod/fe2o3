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
readonly proof="$root/crates/fe2o3-lower-mir-kernel/verus/source_mir_kir_memory_refinement_v3.rs"
readonly generated_contract="$root/crates/fe2o3-lower-mir-kernel/verus/formal_compiler_v3_spec_generated.rs"
readonly negative_dir="$root/crates/fe2o3-lower-mir-kernel/verus/negative"
readonly closure_manifest="$root/crates/fe2o3-lower-mir-kernel/verus/pins/VERUS_CLOSURE_MANIFEST"
readonly closure_checker="$root/examples/row_softmax_v1/verify-verus-closure.sh"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-180}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-memory-refinement-v3.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
"$closure_checker" "$(dirname -- "$(readlink -f -- "$verus")")" "$closure_manifest" >/dev/null
printf '%s  %s\n' \
    d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019 "$closure_manifest" \
    22472060be444e80c3afbb8d2d83925ec7aa246f40bfab3cd7b89747bdb543fd "$generated_contract" \
    a97ce140fbb8f6b4739c2abab5ceee8802fa0de10df5c8369fd3a77865e34281 "$proof" \
    6a4b25346e58c4025e77c49b2aea26cad8205d7a811841a3d80f14791a40d10f "$negative_dir/memory_trace_unguarded_effect_v3.rs" \
    3cab47b4002cf20dff8f755e580a3f1b1eee48ef376c565e6564e3c1d72d6b19 "$negative_dir/memory_trace_wrong_provenance_v3.rs" \
    799fe50f0b77fb5cd3458952115519b9c923616f9c32811e42a8c3d21c9fcd1a "$negative_dir/memory_trace_wrong_range_v3.rs" \
    33d9ca0c63db0c7ab628bc3ea00dc1571195560ee573c31f827468ae11190233 "$negative_dir/memory_trace_wrong_store_value_v3.rs" \
    | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -R -F "$token" "$proof" "$negative_dir"/memory_trace_*_v3.rs >/dev/null; then
        printf 'forbidden trust token %s in memory-refinement sources\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/proof.log" 2>&1
grep -F 'verification results:: 3 verified, 0 errors' "$tmp/proof.log" >/dev/null

for name in \
    memory_trace_unguarded_effect_v3 \
    memory_trace_wrong_provenance_v3 \
    memory_trace_wrong_range_v3 \
    memory_trace_wrong_store_value_v3
do
    file="$negative_dir/$name.rs"
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$file" \
        >"$tmp/$name.log" 2>&1; then
        printf 'negative Verus fixture unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -E 'postcondition not satisfied|assertion failed' "$tmp/$name.log" >/dev/null
done

printf 'Guarded byte-memory source/MIR/KIR refinement: 3 obligations; 4 expected rejections\n'
