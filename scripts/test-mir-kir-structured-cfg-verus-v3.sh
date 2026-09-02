#!/usr/bin/env bash
set -euo pipefail

readonly root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly requested=${VERUS:?set VERUS to the pinned Verus executable}
case "$requested" in */*) readonly verus=$requested ;; *) readonly verus=$(command -v "$requested") ;; esac
readonly proof="$root/crates/fe2o3-lower-mir-kernel/verus/mir_kir_structured_cfg_v3.rs"
readonly generated="$root/crates/fe2o3-lower-mir-kernel/verus/formal_compiler_v3_spec_generated.rs"
readonly negative="$root/crates/fe2o3-lower-mir-kernel/verus/negative"
readonly manifest="$root/crates/fe2o3-lower-mir-kernel/verus/pins/VERUS_CLOSURE_MANIFEST"
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-structured-cfg-proof.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd "$verus" | sha256sum -c -
"$verus" --version | grep -F 'Version: 0.2026.08.02.b677dd5' >/dev/null
"$root/examples/row_softmax_v1/verify-verus-closure.sh" "$(dirname -- "$(readlink -f -- "$verus")")" "$manifest" >/dev/null

(
  cd "$root"
  sha256sum -c <<'EOF'
21ffbd4cd193fcf57e8127aadd4ce7a478edb23b039994f5f6547451b80d9021  crates/fe2o3-lower-mir-kernel/verus/mir_kir_structured_cfg_v3.rs
1210dbc6a62f9087769f29672bedce40de49cc56e218f2833fae60a5f94ff2cc  crates/fe2o3-lower-mir-kernel/verus/formal_compiler_v3_spec_generated.rs
05be1ded212935e72d53a769aa056dfabc7c73583abcac75200232c5413a9953  crates/fe2o3-lower-mir-kernel/verus/negative/mir_kir_structured_checked_as_wrapping_v3.rs
13e2257f40ba8661b932151e2fcc531d23b68af5b46b4fe4fb3d19176c8a6a6a  crates/fe2o3-lower-mir-kernel/verus/negative/mir_kir_structured_wrong_branch_v3.rs
f733f5e045883a16ae4ef20d034e522b2f7087bb409897ac07612757d3cc3829  crates/fe2o3-lower-mir-kernel/verus/negative/mir_kir_structured_wrong_call_v3.rs
6504878573143918de4e14ad91651b37173a9a864653dbb5540a1bba55f2ba7f  crates/fe2o3-lower-mir-kernel/verus/negative/mir_kir_structured_wrong_loop_v3.rs
71b96572d9e0922c607b727f4a02e63b55522eddf0e89a4ed634319435fcf7d6  crates/fe2o3-lower-mir-kernel/verus/negative/mir_kir_structured_wrong_phi_v3.rs
EOF
)

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
  if grep -F "$token" "$proof" "$generated" "$negative"/mir_kir_structured_*_v3.rs >/dev/null; then
    printf 'forbidden trust token %s in structured CFG proof slice\n' "$token" >&2
    exit 1
  fi
done

timeout 120 "$verus" --crate-type lib --triggers-mode silent "$proof" >"$tmp/positive.log" 2>&1
grep -F 'verification results:: 14 verified, 0 errors' "$tmp/positive.log" >/dev/null
count=0
for file in "$negative"/mir_kir_structured_*_v3.rs; do
  count=$((count + 1))
  if timeout 120 "$verus" --crate-type lib --triggers-mode silent --verify-root \
      --verify-function hostile_mutation_v3 "$file" >"$tmp/$(basename "$file").log" 2>&1; then
    printf 'hostile mutation unexpectedly verified: %s\n' "$file" >&2
    exit 1
  fi
  grep -E 'postcondition not satisfied|assertion failed' "$tmp/$(basename "$file").log" >/dev/null
done
test "$count" -eq 5
printf 'structured MIR-to-KIR CFG V3: 14 verified; 5 hostile mutations rejected\n'
