#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 2147483648 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
owned=$(mktemp -d /tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.XXXXXXXX)
printf 'fe2o3-hip-smoke-1890a64e1911a2a346c5a9e70a8ff5ad45b19231\n' > "$owned/owner"
printf 'owned_directory=%s\n' "$owned"
date -u +%FT%T.%NZ
