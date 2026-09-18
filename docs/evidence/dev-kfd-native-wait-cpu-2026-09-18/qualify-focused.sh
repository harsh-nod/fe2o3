#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1
bash "$archive/record.sh" gnu-kfd-sdma cargo test --frozen -p fe2o3-kfd --features hardware-diagnostic --lib sdma::
bash "$archive/record.sh" gnu-kfd-wait cargo test --frozen -p fe2o3-kfd --features hardware-diagnostic --lib wait::
bash "$archive/record.sh" gnu-runtime cargo test --frozen -p fe2o3-runtime --all-features --lib
bash "$archive/record.sh" gnu-example-default cargo test --frozen -p fe2o3-runtime --example gfx942-runtime-directional-window-benchmark
bash "$archive/record.sh" model-r39 cargo test --frozen -p fe2o3-runtime-model --lib r39_scoped_persistent_sdma_wait_policy
bash "$archive/record.sh" unsafe-source cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
bash "$archive/record.sh" clippy cargo clippy --frozen -p fe2o3-kfd -p fe2o3-runtime --all-targets --all-features -- -D warnings
bash "$archive/record.sh" fmt cargo fmt --all -- --check
