#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
record() { bash "$archive/record.sh" "$@"; }

record source-before python3 -I "$archive/source.py"
record rustc rustc -vV
record cargo cargo -V

record gnu-runtime-build "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib --no-run
record musl-runtime-build "${environment[@]}" FE2O3_HIP_SYS_DISABLE=1 \
    cargo test --frozen -p fe2o3-runtime --all-features --lib \
    --target x86_64-unknown-linux-musl --no-run
record binaries-before python3 -I "$archive/binaries.py"

record gnu-runtime "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib
record musl-runtime "${environment[@]}" FE2O3_HIP_SYS_DISABLE=1 \
    cargo test --frozen -p fe2o3-runtime --all-features --lib \
    --target x86_64-unknown-linux-musl
record binaries-after python3 -I "$archive/binaries.py"

record doctests "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --doc
record clippy "${environment[@]}" \
    cargo clippy --frozen -p fe2o3-runtime --all-targets --all-features -- -D warnings
record no-default "${environment[@]}" \
    cargo check --frozen -p fe2o3-runtime --no-default-features
record fmt "${environment[@]}" cargo fmt --all -- --check
record unsafe-source "${environment[@]}" \
    cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
record source-after python3 -I "$archive/source.py"
