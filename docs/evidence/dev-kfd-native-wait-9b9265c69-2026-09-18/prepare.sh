#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD
[[ -d "$owned" && -O "$owned" && ! -L "$owned" ]]
[[ $(realpath -e "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-kfd-matched-9b9265c6919cb8dff9506f2c6ffa7b7f2538905f ]]
ulimit -c 0
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
export RUSTUP_TOOLCHAIN=nightly-2026-04-03
export CARGO_TARGET_DIR="$owned/target" CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never CARGO_NET_OFFLINE=true
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
cd "$owned"
date -u +%FT%T.%NZ
available=$(df -PB1 "$owned" | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 2147483648 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
printf '%s  %s\n' b88884491326ade19a864f5538bf1f806f069eb2951ac134f9f6d094238ca1cd source.tar 9d15e3613cc69f3a2f1437ec23c48788248b9244e81f2953da6d9db46e86c16d source-files.sha256 | sha256sum -c -
mkdir source results
tar --no-same-owner -xf source.tar -C source
cd source
sha256sum -c "$owned/source-files.sha256" > "$owned/results/source-before.log"
rustc --version --verbose
cargo --version --verbose
g++ --version
uname -a
readlink -f /opt/rocm
printf 'cargo_command='
printf '%q ' cargo build --frozen --release --jobs 2 -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark
printf '\n'
cargo build --frozen --release --jobs 2 -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark
printf 'hsa_command='
printf '%q ' g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -isystem /opt/rocm/include benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -o "$owned/hsa-copy-pool-engine"
printf '\n'
g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -isystem /opt/rocm/include benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -o "$owned/hsa-copy-pool-engine"
sha256sum "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" "$owned/hsa-copy-pool-engine" > "$owned/results/binaries.sha256"
file "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" "$owned/hsa-copy-pool-engine"
readelf -h "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark"
readelf -h "$owned/hsa-copy-pool-engine"
ldd "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" > "$owned/results/kfd.ldd"
ldd "$owned/hsa-copy-pool-engine" > "$owned/results/hsa.ldd"
mapfile -t libraries < <(awk '/=> \/|^[[:space:]]*\// {for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' "$owned/results/kfd.ldd" "$owned/results/hsa.ldd" | LC_ALL=C sort -u)
[[ ${#libraries[@]} -gt 0 ]]
sha256sum "${libraries[@]}" /opt/rocm/lib/libhsa-runtime64.so /opt/rocm/include/hsa/hsa.h /opt/rocm/include/hsa/hsa_ext_amd.h /usr/bin/g++ /usr/bin/ld /usr/bin/numactl /usr/bin/timeout /usr/bin/prlimit /usr/bin/python3 /opt/rocm/bin/rocm-smi "$(rustup which rustc)" "$(rustup which cargo)" > "$owned/results/platform.sha256"
sha256sum -c "$owned/source-files.sha256" > "$owned/results/source-built.log"
sha256sum -c "$owned/results/binaries.sha256"
sha256sum -c "$owned/results/platform.sha256"
cat "$owned/results/binaries.sha256"
cat "$owned/results/platform.sha256"
printf 'prepare_complete=true native_launched=false source_commit=9b9265c6919cb8dff9506f2c6ffa7b7f2538905f cpu_build_jobs=2\n'
date -u +%FT%T.%NZ
