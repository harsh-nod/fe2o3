#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
target=${1:?gnu or musl-no-hip}
[[ $target == gnu || $target == musl-no-hip ]]
cd -- "$root"
record() { bash "$archive/record.sh" "$target-$1" "${@:2}"; }
record source-before sha256sum --check "$archive/source-files.sha256"
target_args=()
build_env=()
[[ $target == gnu ]] || target_args=(--target x86_64-unknown-linux-musl)
[[ $target != musl-no-hip ]] || build_env=(FE2O3_HIP_SYS_DISABLE=1)
record build env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
    "${build_env[@]}" \
    cargo test --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features \
    --lib --no-run "${target_args[@]}" --message-format=json
for crate in host runtime; do
    binary=$(jq -Rr --arg name "fe2o3_$crate" '
        fromjson? | select(.reason == "compiler-artifact" and
        .target.name == $name and .profile.test == true) | .executable // empty
    ' "$archive/raw/$target-build.log")
    [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
    record "$crate-binary" sha256sum "$binary"
    record "$crate-roster" "$binary" --list
    record "$crate" "$binary" --test-threads=1
    record "$crate-binary-after" sha256sum --check "$archive/raw/$target-$crate-binary.log"
done
record source-after sha256sum --check "$archive/source-files.sha256"
