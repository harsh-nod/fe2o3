#!/bin/sh
# Authenticate the controller before importing it or running its calibrations.
set -eu
PATH=/usr/bin:/bin
export PATH
[ "$#" -eq 2 ] || { printf 'usage: %s ABSOLUTE_NEW_OUTPUT ABSOLUTE_VERUS\n' "$0" >&2; exit 2; }
output=$1
verus=$2
repo=$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd -P)
prefix=crates/fe2o3-runtime-model/verus
checker=$prefix/check-resource-domain.py
tests=$prefix/test-resource-domain.py
manifest=$prefix/pins/R75_DOMAIN_SOURCES_SHA256
checker_sha=193f98b4011a3a1966391d622e2e78ea4683837bac46d3dddb8700e71121a649
tests_sha=ae6b14d9d64767e7f3330b39c2dbfe7c53687de985b901322f5b54dc43e44dcb
manifest_sha=b8a02242c33aef316f6c0af54490a7cd26e84657e60f294a806c269990d47250
case "$output:$verus" in /*:/*) ;; *) exit 2 ;; esac
[ "$(realpath -m -- "$output")" = "$output" ]
[ ! -e "$output" ]
mkdir -- "$output"
capture=$output/controller-source
check() {
    actual=$(sha256sum -- "$1")
    [ "${actual%% *}" = "$2" ] || { printf 'digest mismatch: %s\n' "$1" >&2; exit 1; }
}
copy() {
    mkdir -p -- "$capture/$(dirname -- "$1")"
    cp -- "$repo/$1" "$capture/$1"
}
check "$repo/$checker" "$checker_sha"
check "$repo/$tests" "$tests_sha"
check "$repo/$manifest" "$manifest_sha"
copy "$checker"
copy "$tests"
copy "$manifest"
check "$capture/$checker" "$checker_sha"
check "$capture/$tests" "$tests_sha"
check "$capture/$manifest" "$manifest_sha"
gate() {
    /usr/bin/env -i "HOME=$HOME" PATH=/usr/bin:/bin LC_ALL=C /usr/bin/python3 -I -B -c '
import importlib.util, sys
from pathlib import Path
spec = importlib.util.spec_from_file_location("checker", sys.argv[1])
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
repo = Path(sys.argv[2])
checker.source_gate(checker.ordinary(repo / checker.MANIFEST),
    {path: checker.ordinary(repo / path) for path in checker.SOURCES})
' "$capture/$checker" "$repo"
}
gate > "$output/source-before.log" 2>&1
while read -r digest path; do
    copy "$path"
    check "$capture/$path" "$digest"
done < "$capture/$manifest"
copy examples/row_softmax_v1/verify-verus-closure.sh
copy "$prefix/pins/VERUS_CLOSURE_MANIFEST"
/usr/bin/env -i "HOME=$HOME" PATH=/usr/bin:/bin LC_ALL=C /usr/bin/python3 -I -B \
    "$capture/$tests" > "$output/controller-tests.log" 2>&1
/usr/bin/env -i "HOME=$HOME" PATH=/usr/bin:/bin LC_ALL=C /usr/bin/python3 -I -B \
    "$capture/$checker" --verus "$verus" --output "$output/proof" > "$output/proof-driver.log" 2>&1
gate > "$output/source-after.log" 2>&1
check "$repo/$checker" "$checker_sha"
check "$repo/$tests" "$tests_sha"
check "$capture/$checker" "$checker_sha"
check "$capture/$tests" "$tests_sha"
printf 'R75_AUTHENTICATED_DOMAIN_CAMPAIGN_OK\n'
