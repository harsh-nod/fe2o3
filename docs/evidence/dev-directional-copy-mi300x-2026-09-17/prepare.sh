#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /home/harsh/fe2o3-copy-diagnostic-20260917.* && -d "$owned" && ! -L "$owned" ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
export CARGO_TARGET_DIR="$owned/target"
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=4
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TERM_COLOR=never
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
mkdir "$owned/results"
date -u +%FT%TZ
git clone --shared --no-checkout "$HOME/fe2o3" "$owned/source"
git -C "$owned/source" fetch --no-tags https://github.com/harsh-nod/fe2o3.git ed5b5d64bf95116c21e9bf350c30132ff2bbb524
git -C "$owned/source" sparse-checkout set --no-cone /Cargo.toml /Cargo.lock /rust-toolchain.toml /crates/ /examples/ /benchmarks/runtime_gfx942/
git -C "$owned/source" checkout --detach ed5b5d64bf95116c21e9bf350c30132ff2bbb524
cd "$owned/source"
[[ -z $(git status --porcelain) ]]
git rev-parse HEAD
rustc --version --verbose
cargo --version --verbose
/opt/rocm/bin/hipcc --version
g++ --version
uname -a
cargo build --locked --release -p fe2o3-runtime --example gfx942-runtime-directional-window-benchmark
cargo test --locked -p fe2o3-kfd --lib persistent_directional_sdma::tests::window_manifest_digest_is_frozen -- --exact
/opt/rocm/bin/hipcc -std=c++17 -O3 -Wall -Wextra -Werror benchmarks/runtime_gfx942/async_copy_hip.cpp -o "$owned/async-copy-hip"
g++ -std=c++17 -O3 -Wall -Wextra -Werror -I/opt/rocm/include benchmarks/runtime_gfx942/async_copy_hsa.cpp -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -o "$owned/async-copy-hsa"
sha256sum "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" "$owned/async-copy-hip" "$owned/async-copy-hsa" > "$owned/results/binaries.sha256"
git ls-files -z Cargo.toml Cargo.lock crates examples benchmarks/runtime_gfx942 | xargs -0 sha256sum > "$owned/results/source-files.sha256"
[[ -z $(git status --porcelain) ]]
date -u +%FT%TZ
