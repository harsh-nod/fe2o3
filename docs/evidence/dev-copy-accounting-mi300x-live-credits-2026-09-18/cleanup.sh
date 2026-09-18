#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-copy-accounting-live-credits-20260918.UYReiiVM
marker=fe2o3-copy-accounting-f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe
date -u +%FT%T.%NZ
[[ -d "$owned" && ! -L "$owned" && $(cat "$owned/owner") == "$marker" ]]
[[ $(stat -c %u "$owned") == $(id -u) ]]
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
printf 'owned_exe_cwd_references=0\n'
du -sh "$owned"
rm -rf -- "$owned"
[[ ! -e "$owned" && ! -L "$owned" ]]
printf 'owned_directory_removed=%s\n' "$owned"
date -u +%FT%T.%NZ
