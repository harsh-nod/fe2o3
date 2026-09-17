#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /home/harsh/fe2o3-engine-diagnostic-20260917.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
cd "$owned/source"
[[ $(git rev-parse HEAD) == 04d9f3ca37cd17536901d4d7cab405bf06f54454 ]]
[[ -z $(git status --porcelain) ]]
sha256sum --check "$owned/results/source-files.sha256" > "$owned/results/source-before.log"
sha256sum --check "$owned/results/binaries.sha256"

admit() {
    local status pids admissible=0
    date -u +%FT%T.%NZ
    status=$(/usr/bin/timeout --kill-after=5s 20s rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --json) || return $?
    printf '%s\n' "$status"
    jq -e '.card1 | .["Unique ID"] == "0xab83d2ffef0d3cdf" and .["PCI Bus"] == "0000:26:00.0" and (.["GPU use (%)"] | tonumber) == 0 and (.["VRAM Total Used Memory (B)"] | tonumber) < 536870912' <<< "$status" > /dev/null || admissible=$?
    pids=$(/usr/bin/timeout --kill-after=5s 20s rocm-smi --showpidgpus) || return $?
    printf '%s\n' "$pids"
    awk '
        /^PID [0-9]+ is using [0-9]+ DRM device\(s\)/ {
            count=$5; seen++;
            if (count > 0) {
                if (getline <= 0 || NF != count) { bad=1; exit }
                for (i=1; i<=NF; i++) if ($i !~ /^[0-9]+$/ || $i == 1) { bad=1; exit }
            }
            next
        }
        /^=|^[[:space:]]*$/ { next }
        { bad=1; exit }
        END { if (bad || seen == 0) exit 1 }
    ' <<< "$pids" || admissible=$?
    ((admissible == 0)) || return "$admissible"
    printf 'admitted gpu=1 uid=0xab83d2ffef0d3cdf bdf=0000:26:00.0\n'
}

finish() {
    local status=$?
    trap - EXIT
    set +e
    sha256sum --check "$owned/results/source-files.sha256" > "$owned/results/source-after.log"
    local source_status=$?
    sha256sum --check "$owned/results/binaries.sha256"
    local binary_status=$?
    local clean_status=0 dirty
    dirty=$(git status --porcelain) || clean_status=$?
    [[ -z "$dirty" ]] || clean_status=1
    printf 'post_run_porcelain=%s\n' "$dirty"
    admit
    local final_guard_status=$?
    date -u +%FT%T.%NZ
    ((status != 0)) || status=$((source_status || binary_status || clean_status || final_guard_status))
    printf 'finished exit=%s source_after_exit=%s binaries_after_exit=%s clean_exit=%s occupancy_exit=%s\n' "$status" "$source_status" "$binary_status" "$clean_status" "$final_guard_status"
    exit "$status"
}
trap finish EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

[[ $(cat /sys/bus/pci/devices/0000:26:00.0/numa_node) == 0 ]]
printf 'gpu_numa_node=0 gpu_local_cpulist='
cat /sys/bus/pci/devices/0000:26:00.0/local_cpulist
/usr/bin/numactl --physcpubind=0-47 --membind=0 /usr/bin/numactl --show
phase() {
    local repetition=$1 profile=$2 status=0 post_status=0
    printf 'phase repetition=%s profile=%s\n' "$repetition" "$profile"
    admit
    /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- \
        /usr/bin/numactl --physcpubind=0-47 --membind=0 \
        "$owned/target/release/examples/kfd-sdma-copy-benchmark" 0xab83d2ffef0d3cdf 4194272 1 3 10 "$profile" || status=$?
    printf 'completed repetition=%s profile=%s exit=%s\n' "$repetition" "$profile" "$status"
    sleep 20
    admit || post_status=$?
    printf 'postflight repetition=%s profile=%s exit=%s\n' "$repetition" "$profile" "$post_status"
    ((status == 0)) || return "$status"
    return "$post_status"
}
printf 'context diagnostic=single-packet-engine-policy git_commit=04d9f3ca37cd17536901d4d7cab405bf06f54454 gpu=1 unique_id=0xab83d2ffef0d3cdf bytes=4194272 depth=1 warmups=3 samples=10 repetitions=4 cpu_affinity=0-47 memory_node=0\n'
phase 1 engine0
phase 1 engine1
phase 2 engine1
phase 2 engine0
phase 3 engine0
phase 3 engine1
phase 4 engine1
phase 4 engine0
