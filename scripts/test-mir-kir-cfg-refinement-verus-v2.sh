#!/usr/bin/env bash
set -euo pipefail

readonly root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly requested=${VERUS:?set VERUS to the pinned Verus executable}
case "$requested" in */*) readonly verus=$requested ;; *) readonly verus=$(command -v "$requested") ;; esac
readonly proof="$root/crates/fe2o3-lower-mir-kernel/verus/mir_kir_cfg_refinement_v2.rs"
readonly negative="$root/crates/fe2o3-lower-mir-kernel/verus/negative"
readonly manifest="$root/crates/fe2o3-lower-mir-kernel/verus/pins/VERUS_CLOSURE_MANIFEST"
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-cfg-proof.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd "$verus" | sha256sum -c -
"$verus" --version | grep -F 'Version: 0.2026.08.02.b677dd5' >/dev/null
"$root/examples/row_softmax_v1/verify-verus-closure.sh" "$(dirname -- "$(readlink -f -- "$verus")")" "$manifest" >/dev/null
printf '%s  %s\n' \
  d2a84952e585673c686a95627a9e18f75ce4ba809ba0a6f40a7c5f937b1b2af9 "$proof" \
  d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019 "$manifest" \
  8a7110ec1f314ac39ccd8b56674a6872b7221655bb7f83d5891775fff6c6dca2 "$negative/mir_kir_cfg_wrong_branch_v2.rs" \
  4f2186ff7de8d0304357cd4c83b118008821db8d89e8530d4d0dd0043f209feb "$negative/mir_kir_cfg_wrong_callee_v2.rs" \
  423265ab124417916789efa4cd54c89ea50ff14cb116a20279bc5f3bf7ef4193 "$negative/mir_kir_cfg_wrong_phi_v2.rs" \
  e850a961014ef039f53ca3ea288a3b5dd4987c53c10167f6a0e3636063a29833 "$negative/mir_kir_cfg_wrong_return_v2.rs" | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
  if grep -F "$token" "$proof" "$negative"/mir_kir_cfg_*_v2.rs >/dev/null; then
    printf 'forbidden trust token %s in CFG proof slice\n' "$token" >&2
    exit 1
  fi
done

timeout 120 "$verus" --crate-type lib --triggers-mode silent "$proof" >"$tmp/positive.log" 2>&1
grep -F 'verification results:: 4 verified, 0 errors' "$tmp/positive.log" >/dev/null
for file in "$negative"/mir_kir_cfg_*_v2.rs; do
  if timeout 120 "$verus" --crate-type lib --triggers-mode silent "$file" >"$tmp/$(basename "$file").log" 2>&1; then
    printf 'hostile mutation unexpectedly verified: %s\n' "$file" >&2
    exit 1
  fi
  grep -E 'postcondition not satisfied|assertion failed' "$tmp/$(basename "$file").log" >/dev/null
done
printf 'MIR-to-KIR u32 diamond/direct-call refinement: 4 verified; 4 hostile mutations rejected\n'
