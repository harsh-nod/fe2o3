#!/usr/bin/env bash
set -euo pipefail

archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
run() { bash "$archive/record.sh" final "$@"; }

run base git rev-parse HEAD
run source-before sha256sum --check "$archive/source-files.sha256"
run rustc rustc -vV
run cargo cargo -V
run clippy env CARGO_INCREMENTAL=0 cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
run formatting cargo fmt --all -- --check
for target in gnu musl; do
    args=()
    if [[ $target == musl ]]; then args=(--target x86_64-unknown-linux-musl); fi
    run "$target-build" env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib "${args[@]}" --no-run --message-format=json
    for crate in kfd runtime; do
        mapfile -t executables < <(jq -Rr --arg name "fe2o3_$crate" 'fromjson? | select(.reason == "compiler-artifact" and .target.name == $name and .profile.test == true) | .executable' "$archive/final/$target-build.log")
        [[ ${#executables[@]} == 1 && -x ${executables[0]} ]]
        sha256sum "${executables[0]}" > "$archive/final/$target-$crate-binary.sha256"
        run "$target-$crate-roster" "${executables[0]}" --list
        if [[ $crate == runtime ]]; then
            run "$target-focused" prlimit --core=0:0 -- "${executables[0]}" generated_native_inputs --test-threads=1
            run "$target-ignored" "${executables[0]}" --ignored --list
        fi
        run "$target-$crate-full" prlimit --core=0:0 -- "${executables[0]}" --test-threads=4
    done
done
run doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-runtime --all-features --doc
run no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline -p fe2o3-runtime --no-default-features
run unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
run source-after sha256sum --check "$archive/source-files.sha256"
