#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /home/harsh/fe2o3-engine-diagnostic-20260917.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
export CARGO_TARGET_DIR="$owned/target"
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 CARGO_TERM_COLOR=never
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
mkdir "$owned/results"
date -u +%FT%TZ
git clone --shared --no-checkout "$HOME/fe2o3" "$owned/source"
git -C "$owned/source" fetch --no-tags https://github.com/harsh-nod/fe2o3.git 04d9f3ca37cd17536901d4d7cab405bf06f54454
git -C "$owned/source" sparse-checkout set --no-cone /Cargo.toml /Cargo.lock /rust-toolchain.toml /crates/ /examples/
git -C "$owned/source" checkout --detach 04d9f3ca37cd17536901d4d7cab405bf06f54454
cd "$owned/source"
[[ -z $(git status --porcelain) ]]
git rev-parse HEAD
rustc --version --verbose
cargo --version --verbose
uname -a
nice -n 10 cargo build --locked --release -p fe2o3-kfd --example kfd-sdma-copy-benchmark
sha256sum "$owned/target/release/examples/kfd-sdma-copy-benchmark" /usr/bin/numactl > "$owned/results/binaries.sha256"
git ls-files -z Cargo.toml Cargo.lock rust-toolchain.toml crates examples | xargs -0 sha256sum > "$owned/results/source-files.sha256"
[[ -z $(git status --porcelain) ]]
date -u +%FT%TZ
