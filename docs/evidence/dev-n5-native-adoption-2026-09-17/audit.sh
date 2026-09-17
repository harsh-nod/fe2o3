#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
sha256sum --check "$archive/source-files.sha256"
base=$(<"$archive/source-base.txt")
git diff "$base" --binary -- crates/fe2o3-runtime/src | cmp - "$archive/source.patch"
for command in "$archive"/raw/*.command; do
    prefix=${command%.command}
    for suffix in started finished exit log; do test -f "$prefix.$suffix"; done
    status=$(<"$prefix.exit")
    if [[ ${prefix##*/} == clippy ]]; then test "$status" = 101; else test "$status" = 0; fi
done
for name in gnu-final musl-final; do
    grep -Fx 'test result: ok. 863 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out;' <(sed -E 's/ finished in [0-9.]+s$//' "$archive/raw/$name.log")
    grep -F 'generated_native_entered_drop_fails_closed_before_owner_destruction ... ok' "$archive/raw/$name.log"
    grep -F 'generated_stream_guards_reject_without_generated_buffer_arguments ... ok' "$archive/raw/$name.log"
    grep -F 'generated_native_cold_primary_auxiliary_rebound_abort_and_shutdown ... ignored' "$archive/raw/$name.log"
    grep -F 'generated_native_bootstrap_primary_auxiliary_rebound_abort_and_shutdown ... ignored' "$archive/raw/$name.log"
done
grep -F '33 passed; 0 failed' "$archive/raw/doctests.log"
grep -F '5 passed; 0 failed; 1 ignored' "$archive/raw/unsafe-policy.log"
printf 'N5 development source and command-result audit passed; native probes remain unexecuted.\n'
