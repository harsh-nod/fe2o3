#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-native-wait-matched-20260918.tS8fnkzm && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-native-wait-ecad7245fa9fa051daf3a9cb3cd82724da5446eb ]]
export PATH="$HOME/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin"
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
source "$owned/guard.sh"
cd "$owned/source"
[[ $(git rev-parse HEAD) == ecad7245fa9fa051daf3a9cb3cd82724da5446eb ]]
[[ -z $(git status --porcelain) ]]
sha256sum --check "$owned/results/source-files.sha256" > "$owned/results/source-before.log"
sha256sum --check "$owned/results/binaries.sha256"
finish() {
    local status=$?
    trap - EXIT
    set +e
    sha256sum --check "$owned/results/source-files.sha256" > "$owned/results/source-after.log"
    local source_status=$?
    sha256sum --check "$owned/results/binaries.sha256"
    local binary_status=$?
    local dirty clean_status=0
    dirty=$(git status --porcelain) || clean_status=$?
    [[ -z "$dirty" ]] || clean_status=1
    printf 'post_run_porcelain=%s\n' "$dirty"
    admit
    local guard_status=$?
    date -u +%FT%T.%NZ
    ((status != 0)) || status=$((source_status || binary_status || clean_status || guard_status))
    printf 'finished exit=%s source_after_exit=%s binaries_after_exit=%s clean_exit=%s occupancy_exit=%s\n' "$status" "$source_status" "$binary_status" "$clean_status" "$guard_status"
    exit "$status"
}
trap finish EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
phase() {
    local repetition=$1 cell=$2 status=0 post_status=0 policy
    printf 'phase repetition=%s cell=%s\n' "$repetition" "$cell"
    admit
    case "$cell" in
        A|B|C)
            case "$cell" in
                A) policy=diagnostic-slice50us ;;
                B) policy=diagnostic-native-sleep1ms ;;
                C) policy=diagnostic-native-sleep25us ;;
            esac
            /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=48-95 --membind=1 "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" 0x54f88318ca05093d 268435456 3 10 "$policy" || status=$?
            ;;
        D)
            HSA_XNACK=0 ROCR_VISIBLE_DEVICES=4 /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=48-95 --membind=1 "$owned/hsa-copy-pool-engine" 0 1 268435456 3 10 0x54f88318ca05093d fine engine1 || status=$?
            ;;
        *) return 64 ;;
    esac
    printf 'completed repetition=%s cell=%s exit=%s\n' "$repetition" "$cell" "$status"
    sleep 20
    admit || post_status=$?
    printf 'postflight repetition=%s cell=%s exit=%s\n' "$repetition" "$cell" "$post_status"
    ((status == 0)) || return "$status"
    return "$post_status"
}
python3 -I benchmarks/runtime_gfx942/r26-host-guard.py topology --gpu-index 4 --pci-bdf 0000:85:00.0 --unique-id 0x54f88318ca05093d
[[ $(cat /sys/bus/pci/devices/0000:85:00.0/numa_node) == 1 ]]
[[ $(cat /sys/bus/pci/devices/0000:85:00.0/local_cpulist) == 48-95 ]]
/usr/bin/numactl --physcpubind=48-95 --membind=1 /usr/bin/numactl --show
printf 'context diagnostic=shared-host-kfd-native-wait git_commit=ecad7245fa9fa051daf3a9cb3cd82724da5446eb gpu=4 unique_id=0x54f88318ca05093d bytes=268435456 depth=1 warmups=3 samples=10 repetitions=4 optional_context_journal=disabled placement=cpu48-95-memory1\n'
for repetition in 1 2 3 4; do
    case "$repetition" in 1) order=(A B D C) ;; 2) order=(B C A D) ;; 3) order=(C D B A) ;; 4) order=(D A C B) ;; esac
    for cell in "${order[@]}"; do phase "$repetition" "$cell"; done
done
