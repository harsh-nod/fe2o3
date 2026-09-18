#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
bash "$archive/record.sh" binaries-before python3 -I "$archive/binaries.py"
for target in gnu musl; do
    prefix=("${environment[@]}")
    flags=()
    if [[ $target == musl ]]; then
        prefix+=(FE2O3_HIP_SYS_DISABLE=1)
        flags=(--target x86_64-unknown-linux-musl)
    fi
    bash "$archive/record.sh" "$target-tests" "${prefix[@]}" cargo test --frozen -p fe2o3-runtime --all-features --lib "${flags[@]}"
done
bash "$archive/record.sh" binaries-after python3 -I "$archive/binaries.py"
bash "$archive/record.sh" clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-runtime --all-targets --all-features -- -D warnings
bash "$archive/record.sh" no-default "${environment[@]}" cargo check --frozen -p fe2o3-runtime --no-default-features
bash "$archive/record.sh" fmt "${environment[@]}" cargo fmt -p fe2o3-runtime -- --check
bash "$archive/record.sh" unsafe-source "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
bash "$archive/record.sh" observer-tests-final python3 -B -m unittest discover -s benchmarks/runtime_gfx942 -p test_copy_host_observe.py -v
bash "$archive/record.sh" source-qualified-after python3 -I "$archive/source.py"
