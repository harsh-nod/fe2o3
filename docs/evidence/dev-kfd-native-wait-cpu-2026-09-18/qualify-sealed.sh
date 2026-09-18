#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1
bash "$archive/record.sh" source-qualified-before-v2 python3 -I "$archive/source.py"
bash "$archive/record.sh" clippy-final cargo clippy --frozen -p fe2o3-kfd -p fe2o3-runtime --all-targets --all-features -- -D warnings
bash "$archive/record.sh" fmt cargo fmt --all -- --check
bash "$archive/record.sh" gnu-example-qualified cargo test --frozen -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark
bash "$archive/record.sh" musl-example env FE2O3_HIP_SYS_DISABLE=1 cargo test --frozen -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark --target x86_64-unknown-linux-musl
bash "$archive/record.sh" musl-runtime env FE2O3_HIP_SYS_DISABLE=1 cargo test --frozen -p fe2o3-runtime --all-features --lib --target x86_64-unknown-linux-musl
bash "$archive/record.sh" gnu-runtime-build cargo test --frozen -p fe2o3-runtime --all-features --lib --no-run
bash "$archive/record.sh" gnu-kfd-sdma-final cargo test --frozen -p fe2o3-kfd --features hardware-diagnostic --lib sdma::
bash "$archive/record.sh" gnu-kfd-wait-final cargo test --frozen -p fe2o3-kfd --features hardware-diagnostic --lib wait::
bash "$archive/record.sh" binaries-before python3 -I "$archive/binaries.py"
bash "$archive/record.sh" gnu-runtime-final cargo test --frozen -p fe2o3-runtime --all-features --lib
bash "$archive/record.sh" binaries-after python3 -I "$archive/binaries.py"
bash "$archive/record.sh" source-after python3 -I "$archive/source.py"
