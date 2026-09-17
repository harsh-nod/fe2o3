#!/usr/bin/env bash
set -euo pipefail

manifest=$(cd -- "$(dirname -- "$0")" && pwd)/source-files.sha256
root=${1:?usage: verify-auxiliary-release.sh CHECKOUT OUTPUT_DIRECTORY}
out=${2:?usage: verify-auxiliary-release.sh CHECKOUT OUTPUT_DIRECTORY}
out=$(realpath -- "$out")
cd -- "$root"
export CARGO_INCREMENTAL=0

run() {
    local name=$1 status
    shift
    printf '%q ' "$@" > "$out/$name.command"
    printf '\n' >> "$out/$name.command"
    date -u +%FT%T.%NZ > "$out/$name.started"
    if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%T.%NZ > "$out/$name.finished"
    printf '%s\n' "$status" > "$out/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}

run source-before sha256sum --check "$manifest"
run rustc rustc -vV
run cargo cargo -V
run python python3 --version
run clippy cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
run formatting cargo fmt --all -- --check
for target in gnu musl; do
    args=()
    if [[ $target == musl ]]; then args=(--target x86_64-unknown-linux-musl); fi
    run "$target-build" cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib "${args[@]}" --no-run --message-format=json
    for crate in kfd runtime; do
        mapfile -t executables < <(jq -Rr --arg name "fe2o3_$crate" 'fromjson? | select(.reason == "compiler-artifact" and .target.name == $name and .profile.test == true) | .executable' "$out/$target-build.log")
        [[ ${#executables[@]} == 1 && -x ${executables[0]} ]]
        sha256sum "${executables[0]}" > "$out/$target-$crate-binary.sha256"
        run "$target-$crate-roster" "${executables[0]}" --list
        if [[ $crate == runtime ]]; then
            run "$target-runtime-ignored" "${executables[0]}" --ignored --list
            grep -Fxq 'kfd_backend::retained_release_tests::native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection: test' "$out/$target-runtime-ignored.log"
        fi
        run "$target-$crate-full" prlimit --core=0:0 -- "${executables[0]}" --test-threads=4
        grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$out/$target-$crate-full.log"
    done
done
run no-default cargo check --locked --offline -p fe2o3-runtime --no-default-features
run unsafe-policy cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
run source-after sha256sum --check "$manifest"
