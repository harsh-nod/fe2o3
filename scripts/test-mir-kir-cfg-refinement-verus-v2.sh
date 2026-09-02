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
  c9d5881ff9f03e016ebec753bc15ee5a1ff0e5ecacd9f8f531e2810ebde06a34 "$proof" \
  d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019 "$manifest" \
  3b18d0f690d9d7621c40c5ae6aaa0b736bcf624d554e9215e0ea1d597e131acd "$negative/mir_kir_cfg_wrong_branch_v2.rs" \
  092dad3e0aa0e4781ac86b6485f4d1d9f35fbb2d958d6db6c308d4363be08728 "$negative/mir_kir_cfg_wrong_callee_v2.rs" \
  edf93150dedb115bc430b01f99f1e470532ae42b1d44a8ad7aa6f194de3ff837 "$negative/mir_kir_cfg_wrong_phi_v2.rs" \
  480e1170fb1be87715a2a451b4fc33e8f723ff0fe02b1eb85ec15b63ab52918d "$negative/mir_kir_cfg_wrong_return_v2.rs" | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
  if grep -F "$token" "$proof" "$negative"/mir_kir_cfg_*_v2.rs >/dev/null; then
    printf 'forbidden trust token %s in CFG proof slice\n' "$token" >&2
    exit 1
  fi
done

timeout 120 "$verus" --crate-type lib --triggers-mode silent "$proof" >"$tmp/positive.log" 2>&1
grep -F 'verification results:: 6 verified, 0 errors' "$tmp/positive.log" >/dev/null
for file in "$negative"/mir_kir_cfg_*_v2.rs; do
  if timeout 120 "$verus" --crate-type lib --triggers-mode silent "$file" >"$tmp/$(basename "$file").log" 2>&1; then
    printf 'hostile mutation unexpectedly verified: %s\n' "$file" >&2
    exit 1
  fi
  grep -E 'postcondition not satisfied|assertion failed' "$tmp/$(basename "$file").log" >/dev/null
done
printf 'MIR-to-KIR u32 internal-helper/call-result refinement: 6 verified; 4 hostile mutations rejected\n'
