#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
record() { bash "$archive/record.sh" "$@"; }
record source-before python3 -I "$archive/source.py"
record rustc rustc -vV
record cargo cargo -V
for target in gnu musl; do
    prefix=("${environment[@]}")
    flags=()
    if [[ $target == musl ]]; then
        prefix+=(FE2O3_HIP_SYS_DISABLE=1)
        flags=(--target x86_64-unknown-linux-musl)
    fi
    for crate in runtime host; do
        record "$target-$crate-build" "${prefix[@]}" cargo test --frozen -p "fe2o3-$crate" --all-features --lib "${flags[@]}" --no-run
    done
done
record binaries-before python3 -I "$archive/binaries.py"
for target in gnu musl; do
    prefix=("${environment[@]}")
    flags=()
    if [[ $target == musl ]]; then
        prefix+=(FE2O3_HIP_SYS_DISABLE=1)
        flags=(--target x86_64-unknown-linux-musl)
    fi
    for crate in runtime host; do
        record "$target-$crate" "${prefix[@]}" cargo test --frozen -p "fe2o3-$crate" --all-features --lib "${flags[@]}"
    done
done
record binaries-after python3 -I "$archive/binaries.py"
record doctests "${environment[@]}" cargo test --frozen -p fe2o3-runtime -p fe2o3-host --all-features --doc
record public-api "${environment[@]}" cargo test --frozen -p fe2o3-host --no-default-features --test production_descriptor_error_api
record clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-runtime -p fe2o3-host --all-targets --all-features -- -D warnings
record no-default "${environment[@]}" cargo check --frozen -p fe2o3-runtime -p fe2o3-host --no-default-features
record fmt "${environment[@]}" cargo fmt --all -- --check
record unsafe-source "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
record source-after python3 -I "$archive/source.py"
