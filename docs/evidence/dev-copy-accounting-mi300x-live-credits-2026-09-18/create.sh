#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
printf 'disk_available_bytes=%s\n' "$available"
[[ $available -ge 268435456 ]]
awk '/^MemAvailable:/ {print; exit !($2 >= 4194304)}' /proc/meminfo
owned=$(mktemp -d /tmp/fe2o3-copy-accounting-live-credits-20260918.XXXXXXXX)
printf 'fe2o3-copy-accounting-f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe\n' > "$owned/owner"
printf 'owned_directory=%s\n' "$owned"
date -u +%FT%T.%NZ
