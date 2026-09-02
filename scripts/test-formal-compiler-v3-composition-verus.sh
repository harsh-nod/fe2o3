#!/usr/bin/env bash
set -euo pipefail

readonly root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly requested=${VERUS:?set VERUS to the pinned Verus executable}
case "$requested" in
    */*) readonly verus=$requested ;;
    *) readonly verus=$(command -v "$requested") ;;
esac
readonly proof="$root/formal/compiler-v3/guarded_u32_xor_helper_store_composition_v3.rs"
readonly negative="$root/formal/compiler-v3/negative"
readonly manifest="$root/crates/fe2o3-runtime-model/verus/pins/VERUS_CLOSURE_MANIFEST"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-180}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-formal-compiler-v3.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' d97501a883931d1d173b1bf4b6cf4d973f16d105dbcb468e177b52b2331612d2 "$verus" | sha256sum -c -
"$verus" --version | grep -F 'Version: 0.2026.08.09.92f466f' >/dev/null
"$root/examples/row_softmax_v1/verify-verus-closure.sh" \
    "$(dirname -- "$(readlink -f -- "$verus")")" "$manifest" >/dev/null

(
    cd "$root"
    sha256sum -c <<'EOF'
2cf2232626b144d92acfa5e57635a869d90cf21b6d497ec5f6a94908f713dcf3  formal/compiler-v3/guarded_u32_xor_helper_store_composition_v3.rs
2e7f22ca7fbc189f2de8d607be5f59e665ec6fbdcad99f299f00bb7f6747fa37  formal/compiler-v3/negative/composition_collapsed_extent_v3.rs
67dd0c0432a3b525464339967c06492212de267db06ee005baa2fa64502bc576  formal/compiler-v3/negative/composition_duplicate_row_position_v3.rs
6d26f5be529c7fbe411ba3a6cc8204253c3b69c5efd543213d99efd0e89c7b6f  formal/compiler-v3/negative/composition_reordered_guard_v3.rs
d24a1b018ac6b8e17c818ee82ac491b7290ddc6e28eeaed9d13edd9ca269f130  formal/compiler-v3/negative/composition_substituted_byte_extent_v3.rs
733a7df2ed0216d434e6dcced8f109825a55ba6a580f5d3bdddaf3292b9b5f34  formal/compiler-v3/negative/composition_substituted_result_v3.rs
22472060be444e80c3afbb8d2d83925ec7aa246f40bfab3cd7b89747bdb543fd  crates/fe2o3-lower-mir-kernel/verus/formal_compiler_v3_spec_generated.rs
21ffbd4cd193fcf57e8127aadd4ce7a478edb23b039994f5f6547451b80d9021  crates/fe2o3-lower-mir-kernel/verus/mir_kir_structured_cfg_v3.rs
a97ce140fbb8f6b4739c2abab5ceee8802fa0de10df5c8369fd3a77865e34281  crates/fe2o3-lower-mir-kernel/verus/source_mir_kir_memory_refinement_v3.rs
5b452b460e1028519bfda74b4a84067aec2c958d09294d536485bafe8f2fe0af  crates/fe2o3-proof-contracts/verus/dynamic_constrained_affine_bounds_v3.rs
61c687297864074796b97c0a95a619955c186a2f62007157e3d8f1af17ec6aec  crates/fe2o3-proof-contracts/verus/constrained_affine_bounds_v2.rs
f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01  crates/fe2o3-runtime-model/verus/pins/VERUS_CLOSURE_MANIFEST
EOF
)

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$proof" "$negative"/composition_*_v3.rs >/dev/null; then
        printf 'forbidden trust token %s in Formal Compiler V3 composition slice\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/positive.log" 2>&1
grep -F 'verification results:: 48 verified, 0 errors' "$tmp/positive.log" >/dev/null

count=0
for file in "$negative"/composition_*_v3.rs; do
    count=$((count + 1))
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent \
        --verify-root --verify-function hostile_mutation_v3 "$file" \
        >"$tmp/$(basename "$file").log" 2>&1; then
        printf 'hostile composition mutation unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -E 'postcondition not satisfied|assertion failed' \
        "$tmp/$(basename "$file").log" >/dev/null
done
test "$count" -eq 5

printf 'Formal Compiler V3 composition: 48 verified; 5 hostile mutations rejected\n'
