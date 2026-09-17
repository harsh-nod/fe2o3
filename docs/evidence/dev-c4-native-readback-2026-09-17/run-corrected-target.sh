#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
target=${1:?gnu or musl}
[[ $target == gnu || $target == musl ]]
[[ $(< "$archive/raw/corrected-$target-build.exit") == 0 ]]
cd -- "$root"
record() { bash "$archive/record.sh" "corrected-$target-$1" "${@:2}"; }
record source-before sha256sum --check "$archive/corrected-source-files.sha256"
for crate in kfd runtime service_host; do
    binary=$(jq -Rr --arg name "fe2o3_$crate" '
        fromjson? | select(.reason == "compiler-artifact" and
        .target.name == $name and .profile.test == true) | .executable // empty
    ' "$archive/raw/corrected-$target-build.log")
    [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
    record "$crate-binary" sha256sum "$binary"
    record "$crate-roster" "$binary" --list
    if [[ $crate == kfd ]]; then
        # Self-spawning abort tests need an uninterleaved transcript. The union
        # of both partitions must equal the complete frozen executable roster.
        record kfd-main-roster "$binary" --list --skip queue_linux::tests::
        record kfd-abort-roster "$binary" --list queue_linux::tests::
        record kfd-main "$binary" --test-threads=4 --skip queue_linux::tests::
        record kfd-abort "$binary" --test-threads=1 queue_linux::tests::
    else
        record "$crate" "$binary" --test-threads=1
    fi
    record "$crate-binary-after" sha256sum --check "$archive/raw/corrected-$target-$crate-binary.log"
done
record source-after sha256sum --check "$archive/corrected-source-files.sha256"
