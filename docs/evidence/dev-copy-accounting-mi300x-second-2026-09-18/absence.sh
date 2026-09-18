#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-copy-accounting-corrected-20260918.W7d8ptkX
date -u +%FT%T.%NZ
[[ ! -e "$owned" && ! -L "$owned" ]]
found=0
for proc in /proc/[0-9]*; do
    for ref in exe cwd; do
        target=$(readlink "$proc/$ref" 2>/dev/null || true)
        case "$target" in
            "$owned"|"$owned"/*) printf 'owned_reference=%s/%s target=%s\n' "$proc" "$ref" "$target"; found=1 ;;
        esac
    done
done
[[ $found == 0 ]]
printf 'owned_directory_absent=true owned_exe_cwd_references=0 path=%s\n' "$owned"
date -u +%FT%T.%NZ
