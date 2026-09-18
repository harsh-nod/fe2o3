#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-copy-accounting-20260918.HyOO64fz
cd "$owned"
date -u +%FT%T.%NZ
printf '%s  %s\n' 8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89 runtime-test 5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51 copy-host-observe.py | sha256sum -c -
sha256sum runtime-test copy-host-observe.py run.sh inspect.sh
file runtime-test
readelf -h runtime-test
readelf -l runtime-test
if readelf -l runtime-test | grep -q INTERP; then exit 20; fi
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 268435456 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
bdf=/sys/bus/pci/devices/0000:85:00.0
for name in unique_id numa_node local_cpulist; do
    printf '%s=' "$name"
    cat "$bdf/$name"
done
[[ $(cat "$bdf/unique_id") == 54f88318ca05093d ]]
[[ $(cat "$bdf/numa_node") == 1 ]]
[[ $(cat "$bdf/local_cpulist") == 48-95 ]]
readlink -f "$bdf"
ls -ld "$bdf"/drm/*
command -v python3 prlimit timeout numactl
numactl --hardware
date -u +%FT%T.%NZ
