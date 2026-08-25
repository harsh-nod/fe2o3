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
readonly positive="$root/crates/fe2o3-verifier/verus/mir_pliron_per_compilation_template_v1.rs"
readonly negative_dir="$root/crates/fe2o3-verifier/verus/negative"
readonly timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
readonly tmp=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-per-compilation-verus.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

test -x "$verus"
printf '%s  %s\n' "$expected_verus_sha" "$verus" | sha256sum -c -
"$verus" --version | grep -F "Version: $expected_version" >/dev/null
printf '%s  %s\n' \
    7d130729c3601d771da1ba12114cd7bc527a8c9dce14a90736bb0113f3aff6b6 "$positive" \
    7a12b2e498b43d7053d1cf24d2da0b126978edd27e0ccf6a734fdd696342a25d "$negative_dir/mir_pliron_per_compilation_missing_output_v1.rs" \
    f1103158bed21996729d9af15c8233c876d3ee9ff773cab49c85c188c140f168 "$negative_dir/mir_pliron_per_compilation_wrong_recurrence_v1.rs" \
    | sha256sum -c -

for token in 'assume(' 'admit(' '#[verifier::external_body]' '#[verifier::external]'; do
    if grep -F "$token" "$positive" >/dev/null; then
        printf 'forbidden trust token %s in %s\n' "$token" "$positive" >&2
        exit 1
    fi
done

timeout "$timeout_seconds" "$verus" --crate-type lib --triggers-mode silent "$positive" \
    >"$tmp/positive.log" 2>&1
grep -F 'verification results:: 3 verified, 0 errors' "$tmp/positive.log" >/dev/null

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

printf 'per-compilation MIR/PLIRON composition: 3 positive obligations, 2 expected rejections\n'
