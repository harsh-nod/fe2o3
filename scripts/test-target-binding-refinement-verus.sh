#!/usr/bin/env bash
set -Eeuo pipefail

readonly root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
readonly proof="$root/crates/fe2o3-amdgcn-model/verus/target_binding_refinement_v1.rs"
readonly negative="$root/crates/fe2o3-amdgcn-model/verus/negative/target_binding_refinement_effect_mutation_v1.rs"
readonly pin_dir="$root/crates/fe2o3-amdgcn-model/verus/pins"
readonly source_checker="$root/examples/wave64_collectives_v1/check-proof-source.py"
readonly verus_request=${VERUS:?set VERUS to the pinned Verus executable}

case "$verus_request" in
    */*) verus=$verus_request ;;
    *) verus=$(command -v "$verus_request") ;;
esac
readonly verus=$(readlink -f "$verus")
readonly expected_verus=$(sed -n '1p' "$pin_dir/VERUS_SHA256")
readonly expected_version=$(sed -n '1p' "$pin_dir/VERUS_VERSION")
readonly expected_proof=$(sed -n '1p' "$pin_dir/TARGET_BINDING_REFINEMENT_SHA256")
readonly expected_negative=$(sed -n '1p' "$pin_dir/NEGATIVE_EFFECT_MUTATION_SHA256")
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-target-binding-verus.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

printf '%s  %s\n' "$expected_verus" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
printf '%s  %s\n' "$expected_proof" "$proof" | sha256sum -c -
printf '%s  %s\n' "$expected_negative" "$negative" | sha256sum -c -
"$source_checker" "$proof" "$negative"

timeout 300 "$verus" --crate-type lib --triggers-mode silent "$proof" \
    >"$tmp/positive.log" 2>&1
grep -Eq 'verification results:: [1-9][0-9]* verified, 0 errors' "$tmp/positive.log"

if timeout 300 "$verus" --crate-type lib --triggers-mode silent "$negative" \
    >"$tmp/negative.log" 2>&1; then
    printf 'FAIL: target-binding effect mutation unexpectedly verified\n' >&2
    exit 1
fi
grep -Eq 'verification results:: [0-9]+ verified, [1-9][0-9]* errors' "$tmp/negative.log"

printf 'PASS: target-binding refinement proof and expected-negative mutation\n'
