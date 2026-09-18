#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 268435456 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
owned=$(mktemp -d /tmp/fe2o3-copy-accounting-corrected-20260918.XXXXXXXX)
printf 'fe2o3-copy-accounting-84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312\n' > "$owned/owner"
printf 'owned_directory=%s\n' "$owned"
date -u +%FT%T.%NZ
