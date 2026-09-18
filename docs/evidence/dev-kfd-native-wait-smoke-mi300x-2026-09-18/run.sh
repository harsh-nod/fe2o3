#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-native-wait-20260918.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-native-wait-fcd5a89a113c6538338598bfcdb6769fb5c06042 ]]
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
source "$owned/guard.sh"
cd "$owned/source"
[[ $(git rev-parse HEAD) == fcd5a89a113c6538338598bfcdb6769fb5c06042 ]]
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
python3 -I benchmarks/runtime_gfx942/r26-host-guard.py topology --gpu-index 4 --pci-bdf 0000:85:00.0 --unique-id 0x54f88318ca05093d
[[ $(cat /sys/bus/pci/devices/0000:85:00.0/numa_node) == 1 ]]
[[ $(cat /sys/bus/pci/devices/0000:85:00.0/local_cpulist) == 48-95 ]]
/usr/bin/numactl --physcpubind=48-95 --membind=1 /usr/bin/numactl --show
printf 'context diagnostic=native-wait-smoke git_commit=fcd5a89a113c6538338598bfcdb6769fb5c06042 gpu=4 unique_id=0x54f88318ca05093d bytes=268435456 depth=1 warmups=3 samples=10 placement=cpu48-95-memory1 performance_comparison=none\n'
for cell in B C; do
    printf 'phase cell=%s\n' "$cell"
    admit
    case "$cell" in B) mode=diagnostic-native-sleep1ms ;; C) mode=diagnostic-native-sleep25us ;; esac
    status=0
    /usr/bin/timeout --signal=TERM --kill-after=5s 180s prlimit --core=0:0 -- /usr/bin/numactl --physcpubind=48-95 --membind=1 "$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" 0x54f88318ca05093d 268435456 3 10 "$mode" || status=$?
    printf 'completed cell=%s exit=%s\n' "$cell" "$status"
    sleep 20
    post_status=0
    admit || post_status=$?
    printf 'postflight cell=%s exit=%s\n' "$cell" "$post_status"
    ((status == 0)) || exit "$status"
    ((post_status == 0)) || exit "$post_status"
done
