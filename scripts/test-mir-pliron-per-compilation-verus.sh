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
readonly template="$root/crates/fe2o3-verifier/verus/mir_pliron_per_compilation_template_v1.rs"
readonly generated="$root/crates/fe2o3-verifier/verus/mir_pliron_per_compilation_generated_fixture_v1.rs"
readonly generated_multi="$root/crates/fe2o3-verifier/verus/mir_pliron_per_compilation_generated_multi_output_fixture_v1.rs"
readonly negative_dir="$root/crates/fe2o3-verifier/verus/negative"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-per-compilation-verus.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
printf '%s  %s\n' \
    0b958374e154e88a916f7573ffc7529f8e0ba40d19bd94dd49db5eb4630647f8 "$template" \
    ffce962f103f6d518a2b629ad4a310f64630b53778a1315e91617558b254b9e8 "$generated" \
    2425d9c3640de0f8476ba61e751485e7b0d02b7984fd303a12e601fdaf2cc8bc "$generated_multi" \
    7a12b2e498b43d7053d1cf24d2da0b126978edd27e0ccf6a734fdd696342a25d "$negative_dir/mir_pliron_per_compilation_missing_output_v1.rs" \
    f1103158bed21996729d9af15c8233c876d3ee9ff773cab49c85c188c140f168 "$negative_dir/mir_pliron_per_compilation_wrong_recurrence_v1.rs" \
    b0406bca54d4f0b1bac434cc26d3ec80d9c117b48ecfbc78daa2a915807dbcd8 "$negative_dir/mir_pliron_per_compilation_multi_output_substitution_v1.rs" \
    | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$template" "$generated" "$generated_multi" >/dev/null; then
        printf 'forbidden trust token %s in generated proof sources\n' "$token" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$template" \
    >"$tmp/template.log" 2>&1
grep -F 'verification results:: 0 verified, 0 errors' "$tmp/template.log" >/dev/null

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$generated" \
    >"$tmp/generated.log" 2>&1
grep -F 'verification results:: 4 verified, 0 errors' "$tmp/generated.log" >/dev/null

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$generated_multi" \
    >"$tmp/generated-multi.log" 2>&1
grep -F 'verification results:: 5 verified, 0 errors' "$tmp/generated-multi.log" >/dev/null

for name in \
    mir_pliron_per_compilation_missing_output_v1 \
    mir_pliron_per_compilation_wrong_recurrence_v1
do
    file="$negative_dir/$name.rs"
    if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$file" \
        >"$tmp/$name.log" 2>&1; then
        printf 'negative Verus fixture unexpectedly verified: %s\n' "$file" >&2
        exit 1
    fi
    grep -F 'postcondition not satisfied' "$tmp/$name.log" >/dev/null
done

readonly multi_substitution="$negative_dir/mir_pliron_per_compilation_multi_output_substitution_v1.rs"
if timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$multi_substitution" \
    >"$tmp/multi-output-substitution.log" 2>&1; then
    printf 'multi-output formula substitution unexpectedly verified: %s\n' "$multi_substitution" >&2
    exit 1
fi
grep -F 'assertion failed' "$tmp/multi-output-substitution.log" >/dev/null

printf 'per-compilation MIR/PLIRON replay: single- and multi-output generated obligations; 3 expected rejections\n'
