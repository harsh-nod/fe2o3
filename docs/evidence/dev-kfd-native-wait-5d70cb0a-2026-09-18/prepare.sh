#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-native-wait-5d70cb0a6e16fb265fe224690274fdb0be2b0055 ]]
[[ $(cat "$owned/source.commit") == 5d70cb0a6e16fb265fe224690274fdb0be2b0055 ]]
[[ $(sha256sum "$owned/source.tar" | cut -d ' ' -f 1) == abb3ee3a4be431584a1303bda58828a2c89160d1d1c9461cebad95d64f7eb9b4 ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
export CARGO_TARGET_DIR="$owned/target"
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_TERM_COLOR=never
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
mkdir "$owned/results" "$owned/source"
date -u +%FT%T.%NZ
available=$(df -PB1 "$owned" | awk 'NR == 2 { print $4 }')
[[ $available =~ ^[0-9]+$ && $available -ge 2147483648 ]]
tar -xf "$owned/source.tar" -C "$owned/source"
cd "$owned/source"
[[ -z $(find . -type l -print) ]]
find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$owned/results/source-files.sha256"
[[ $(wc -l < "$owned/results/source-files.sha256") == 5538 ]]
cat "$owned/source.commit"
sha256sum "$owned/source.tar" "$owned/source-tree.txt" "$owned/source.commit"
rustc --version --verbose
cargo --version --verbose
g++ --version
uname -a
readlink -f /opt/rocm
df -B1 "$owned"
cargo build --frozen --release -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark
g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -isystem /opt/rocm/include benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -o "$owned/hsa-copy-pool-engine"
sha256sum "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" "$owned/hsa-copy-pool-engine" > "$owned/results/binaries.sha256"
find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > "$owned/results/source-build-after.sha256"
cmp "$owned/results/source-files.sha256" "$owned/results/source-build-after.sha256"
ldd "$owned/hsa-copy-pool-engine"
ldd "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark"
sha256sum /opt/rocm/lib/libhsa-runtime64.so /usr/bin/g++ /opt/rocm/include/hsa/hsa.h /opt/rocm/include/hsa/hsa_ext_amd.h /usr/bin/numactl /opt/rocm/bin/rocm-smi
date -u +%FT%T.%NZ
