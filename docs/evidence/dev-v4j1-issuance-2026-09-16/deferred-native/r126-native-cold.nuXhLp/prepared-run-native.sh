#!/usr/bin/env bash
set -euo pipefail

kind=${1:?usage: run-native.sh host|device}
case "$kind" in host|device) ;; *) exit 2 ;; esac
: "${FE2O3_TEST_NATIVE_UNIQUE_ID:?explicit native device required}"
[[ ${FE2O3_TEST_NATIVE_ISOLATED:-} == 1 ]]
cd -- "$(dirname -- "$0")"
printf '%s  %s\n' 86f2d5f934e3e4e935ac400828708bf1e12b369f61e21d9565370156f34fee77 runtime-tests | sha256sum --check
name="kfd_backend::retained_release_tests::cold_allocation::native_runtime_cold_${kind}_capacity_refunds_context_and_retries"
command=(env -i PATH=/usr/bin:/bin
    "FE2O3_TEST_NATIVE_UNIQUE_ID=$FE2O3_TEST_NATIVE_UNIQUE_ID"
    FE2O3_TEST_NATIVE_ISOLATED=1
    /usr/bin/prlimit --core=0:0 -- /usr/bin/timeout --signal=TERM --kill-after=5 90
    "$PWD/runtime-tests" "$name" --exact --ignored --nocapture --test-threads=1)
printf '%q ' "${command[@]}" > "$kind.command"
printf '\n' >> "$kind.command"
date -u +%FT%T.%NZ > "$kind.started"
if "${command[@]}" > "$kind.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$kind.finished"
printf '%s\n' "$status" > "$kind.exit"
cat "$kind.log"
exit "$status"
