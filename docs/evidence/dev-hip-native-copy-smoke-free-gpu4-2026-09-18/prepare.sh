#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.EvupxcTh
[[ -d "$owned" && -O "$owned" && ! -L "$owned" ]]
[[ $(realpath -e "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-hip-smoke-1890a64e1911a2a346c5a9e70a8ff5ad45b19231 ]]
ulimit -c 0
export PATH=/opt/rocm/bin:/usr/bin:/bin
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
cd "$owned"
date -u +%FT%T.%NZ
available=$(df -PB1 "$owned" | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 2147483648 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
printf '%s  %s\n' 09f7fa62a1a9145c3806788538c4174d7068a8d0bbcbc018b33d7f1e79d84c6a source.tar d796342d5c9e8b28ea759e65acc5de878a270db0268ea1239659384f3deeafcf source-files.sha256 | sha256sum -c -
mkdir source results
tar --no-same-owner -xf source.tar -C source
cd source
sha256sum -c "$owned/source-files.sha256" > "$owned/results/source-before.log"
uname -a
readlink -f /opt/rocm
hipcc --version
hipconfig --full
g++ --version
dpkg-query -W amdgpu-dkms rocm-core rocm-smi-lib hip-runtime-amd hsa-rocr
command=(/usr/bin/timeout --signal=TERM --kill-after=5s 180s /usr/bin/prlimit --core=0 -- /usr/bin/taskset -c 48,49 /opt/rocm/bin/hipcc --offload-arch=gfx942 -std=c++17 -O3 -Wall -Wextra -Werror -MD -MF "$owned/results/hip.d" benchmarks/runtime_gfx942/async_copy_hip.cpp -o "$owned/async-copy-hip")
printf 'build_command='
printf '%q ' "${command[@]}"
printf '\n'
HIPCC_VERBOSE=7 "${command[@]}"
sha256sum "$owned/async-copy-hip" > "$owned/results/binary.sha256"
file "$owned/async-copy-hip"
readelf -h "$owned/async-copy-hip"
readelf -d "$owned/async-copy-hip" > "$owned/results/hip.dynamic"
ldd "$owned/async-copy-hip" > "$owned/results/hip.ldd"
python3 -B "$owned/dependency-snapshot.py"
sha256sum -c "$owned/source-files.sha256" > "$owned/results/source-built.log"
sha256sum -c "$owned/results/binary.sha256"
sha256sum -c "$owned/results/platform.sha256" > "$owned/results/platform-built.log"
cat "$owned/results/binary.sha256"
printf 'prepare_complete=true native_launched=false source_commit=1890a64e1911a2a346c5a9e70a8ff5ad45b19231 cpu_affinity=48,49 offload_arch=gfx942\n'
date -u +%FT%T.%NZ
