#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 2147483648 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
owned=$(mktemp -d /tmp/fe2o3-kfd-matched-9b9265c69-20260918.XXXXXXXX)
printf 'fe2o3-kfd-matched-9b9265c6919cb8dff9506f2c6ffa7b7f2538905f\n' > "$owned/owner"
printf 'owned_directory=%s\n' "$owned"
date -u +%FT%T.%NZ
