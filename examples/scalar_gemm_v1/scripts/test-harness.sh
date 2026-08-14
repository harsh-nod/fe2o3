#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

cargo test --locked -p fe2o3-scalar-gemm-v1
cargo clippy --locked -p fe2o3-scalar-gemm-v1 --all-targets --no-deps -- \
    -D warnings -A clippy::assign-op-pattern

cargo test --locked --manifest-path examples/scalar_gemm_v1/hardware-harness/Cargo.toml
cargo clippy --locked --manifest-path examples/scalar_gemm_v1/hardware-harness/Cargo.toml \
    --all-targets --no-deps -- -D warnings
