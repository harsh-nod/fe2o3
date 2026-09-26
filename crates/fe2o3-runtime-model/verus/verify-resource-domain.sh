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
checker_sha=a52face08c0c3080aa1711b19403a55a39ff1aef07171962b68407b556f9cadc
tests_sha=4b7219f0a5c8035252f076eac39513649547276b55d6bddaac253be3f7099594
manifest_sha=7d623ac3fac1aaa9addd901fcaac58494da034025f602e84a3f4d08c6ebeefc0
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
