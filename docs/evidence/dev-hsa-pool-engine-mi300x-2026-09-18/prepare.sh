#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-hsa-pool-engine-20260918.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-pool-engine-3da2d25ac965afa9845b8ab1246e5fdf13c5d821 ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
export CARGO_TARGET_DIR="$owned/target"
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_TERM_COLOR=never
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
mkdir "$owned/results"
date -u +%FT%T.%NZ
git clone --shared --no-checkout "$HOME/fe2o3" "$owned/source"
git -C "$owned/source" fetch --no-tags https://github.com/harsh-nod/fe2o3.git 3da2d25ac965afa9845b8ab1246e5fdf13c5d821
git -C "$owned/source" sparse-checkout set --no-cone /Cargo.toml /Cargo.lock /rust-toolchain.toml /crates/ /examples/ /benchmarks/runtime_gfx942/
git -C "$owned/source" checkout --detach 3da2d25ac965afa9845b8ab1246e5fdf13c5d821
cd "$owned/source"
[[ -z $(git status --porcelain) ]]
git rev-parse HEAD
rustc --version --verbose
cargo --version --verbose
g++ --version
uname -a
readlink -f /opt/rocm
df -B1 "$owned"
cargo build --locked --release -p fe2o3-runtime --example gfx942-runtime-directional-window-benchmark
g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -isystem /opt/rocm/include benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -o "$owned/hsa-copy-pool-engine"
sha256sum "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" "$owned/hsa-copy-pool-engine" > "$owned/results/binaries.sha256"
git ls-files -z Cargo.toml Cargo.lock rust-toolchain.toml crates examples benchmarks/runtime_gfx942 | xargs -0 sha256sum > "$owned/results/source-files.sha256"
ldd "$owned/hsa-copy-pool-engine"
ldd "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark"
sha256sum /opt/rocm/lib/libhsa-runtime64.so /usr/bin/g++ /opt/rocm/include/hsa/hsa.h /opt/rocm/include/hsa/hsa_ext_amd.h /usr/bin/numactl /opt/rocm/bin/rocm-smi
[[ -z $(git status --porcelain) ]]
date -u +%FT%T.%NZ
