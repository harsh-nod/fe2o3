#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
available=$(df -PB1 /tmp | awk 'NR == 2 {print $4}')
memory=$(awk '/^MemAvailable:/ {print $2 * 1024}' /proc/meminfo)
printf 'disk_available_bytes=%s mem_available_bytes=%s\n' "$available" "$memory"
[[ $available -ge 268435456 ]]
awk '/^MemAvailable:/ {exit !($2 >= 4194304)}' /proc/meminfo
owned=$(mktemp -d /tmp/fe2o3-copy-accounting-20260918.XXXXXXXX)
printf 'fe2o3-copy-accounting-8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89\n' > "$owned/owner"
printf 'owned_directory=%s\n' "$owned"
date -u +%FT%T.%NZ
