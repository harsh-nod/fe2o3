#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
record() { bash "$archive/record.sh" "$@"; }
filters=(
    queue::live::model_loan::tests::
    queue::live::tests::
    sdma::tests::
    queue_linux::tests::
    shared_memory::tests::live_foundation_
    shared_memory::tests::panic_before_and_after_allocation_map_projection_remains_retakeable
    queue::live::construction_primary::integration_tests::release_cases::sdma_creation_cases::
)
for target in gnu musl; do
    flags=()
    if [[ $target == musl ]]; then flags=(--target x86_64-unknown-linux-musl); fi
    target=$target-final
    record "$target-build" env CARGO_INCREMENTAL=0 cargo test --locked --offline \
        -p fe2o3-kfd --all-features --lib "${flags[@]}" --no-run --message-format=json
    binary=$(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_kfd") | .executable // empty' "$archive/raw/$target-build.log")
    [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
    binary=${binary#"$root/"}
    record "$target-roster" "$binary" --list
    record "$target-binary" sha256sum "$binary"
    record "$target-selected" prlimit --core=0:0 -- "$binary" "${filters[@]}" --test-threads=4
    record "$target-runtime" prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 \
        cargo test --locked --offline -p fe2o3-runtime --all-features --lib "${flags[@]}" -- --test-threads=4
    runtime=$(sed -n 's/.*Running unittests src\/lib.rs (\(.*\)).*/\1/p' "$archive/raw/$target-runtime.log")
    [[ -n $runtime && $runtime != *$'\n'* && -x $runtime ]]
    record "$target-runtime-roster" "$runtime" --list
    record "$target-runtime-binary" sha256sum "$runtime"
done
record clippy-final env CARGO_INCREMENTAL=0 cargo clippy --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
record fmt cargo fmt --all -- --check
record doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --all-features --doc
record no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --no-default-features
record unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p cargo-fe2o3 --test unsafe_source_policy
