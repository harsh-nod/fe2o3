#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /home/harsh/fe2o3-copy-diagnostic-20260917.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
cd "$owned/source"
[[ $(git rev-parse HEAD) == ed5b5d64bf95116c21e9bf350c30132ff2bbb524 ]]
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
    local clean_status=0
    local dirty
    dirty=$(git status --porcelain) || clean_status=$?
    [[ -z "$dirty" ]] || clean_status=1
    printf 'post_run_porcelain=%s\n' "$dirty"
    printf 'final occupancy observation\n'
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

phase() {
    local repetition=$1 backend=$2 status=0 post_status=0
    printf 'phase repetition=%s backend=%s\n' "$repetition" "$backend"
    admit
    case "$backend" in
        kfd)
            /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=0-47 --membind=0 "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" 0xab83d2ffef0d3cdf 268435456 3 10 || status=$?
            ;;
        hsa)
            HSA_XNACK=0 ROCR_VISIBLE_DEVICES=1 /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=0-47 --membind=0 "$owned/async-copy-hsa" 0 268435456 1 3 10 0xab83d2ffef0d3cdf || status=$?
            ;;
        hip)
            HSA_XNACK=0 HIP_VISIBLE_DEVICES=1 /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=0-47 --membind=0 "$owned/async-copy-hip" 0 268435456 1 3 10 0xab83d2ffef0d3cdf || status=$?
            ;;
        *) return 64 ;;
    esac
    printf 'completed repetition=%s backend=%s exit=%s\n' "$repetition" "$backend" "$status"
    # HSA's untimed allocation/free probe can leave deferred VRAM reclamation.
    sleep 20
    admit || post_status=$?
    printf 'postflight repetition=%s backend=%s exit=%s\n' "$repetition" "$backend" "$post_status"
    ((status == 0)) || return "$status"
    return "$post_status"
}

topology=$(python3 -I benchmarks/runtime_gfx942/r26-host-guard.py topology --gpu-index 1 --pci-bdf 0000:26:00.0 --unique-id 0xab83d2ffef0d3cdf)
printf '%s\n' "$topology"
[[ "$topology" == 'topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=1 pci_bdf=0000:26:00.0 unique_id=0xab83d2ffef0d3cdf numa_node=0 device_local_cpu_list=0-47 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=0-47 observer_cpu=95 kfd_node=3 kfd_gpu_id=23018 topology_sha256=fe9e5eb0bb02a2d405a0ee4b1cf729dd97c0705d8b726c5347611c5b76dbf98d' ]]
sha256sum /usr/bin/numactl
/usr/bin/numactl --version
/usr/bin/numactl --physcpubind=0-47 --membind=0 /usr/bin/numactl --show
printf 'context diagnostic=shared-host-numa-local-copy git_commit=ed5b5d64bf95116c21e9bf350c30132ff2bbb524 gpu=1 unique_id=0xab83d2ffef0d3cdf bytes=268435456 depth=1 warmups=3 samples=10 repetitions=3 optional_context_journal=disabled placement=cpu0-47-memory0\n'
phase 1 kfd
phase 1 hsa
phase 1 hip
phase 2 hsa
phase 2 hip
phase 2 kfd
phase 3 hip
phase 3 kfd
phase 3 hsa
