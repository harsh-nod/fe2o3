#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
bash "$archive/record.sh" rustc rustc --version --verbose
bash "$archive/record.sh" cargo cargo --version --verbose
bash "$archive/record.sh" source-before python3 -I "$archive/source.py"
for target in gnu musl; do
    prefix=("${environment[@]}")
    flags=()
    if [[ $target == musl ]]; then
        prefix+=(FE2O3_HIP_SYS_DISABLE=1)
        flags=(--target x86_64-unknown-linux-musl)
    fi
    for crate in host runtime; do
        bash "$archive/record.sh" "$target-$crate-build" "${prefix[@]}" cargo test --frozen -p "fe2o3-$crate" --all-features --lib "${flags[@]}" --no-run
    done
done
bash "$archive/record.sh" binaries-before python3 -I "$archive/binaries.py"
for target in gnu musl; do
    prefix=("${environment[@]}")
    flags=()
    if [[ $target == musl ]]; then
        prefix+=(FE2O3_HIP_SYS_DISABLE=1)
        flags=(--target x86_64-unknown-linux-musl)
    fi
    for crate in host runtime; do
        bash "$archive/record.sh" "$target-$crate" "${prefix[@]}" cargo test --frozen -p "fe2o3-$crate" --all-features --lib "${flags[@]}"
    done
done
bash "$archive/record.sh" binaries-after python3 -I "$archive/binaries.py"
bash "$archive/record.sh" doctests "${environment[@]}" cargo test --frozen -p fe2o3-host -p fe2o3-runtime --all-features --doc
bash "$archive/record.sh" no-default "${environment[@]}" cargo check --frozen -p fe2o3-host -p fe2o3-runtime --no-default-features
bash "$archive/record.sh" clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-host -p fe2o3-runtime --all-targets --all-features -- -D warnings
bash "$archive/record.sh" unsafe-source "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
bash "$archive/record.sh" fmt "${environment[@]}" cargo fmt --all -- --check
bash "$archive/record.sh" source-after python3 -I "$archive/source.py"
