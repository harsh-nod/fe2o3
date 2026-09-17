#!/usr/bin/env bash
set -euo pipefail
owned=/home/harsh/fe2o3-engine-diagnostic-20260917.vNTF2iSX
[[ $# == 0 && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ -d "$owned/source/.git" && -d "$owned/results" ]]
date -u +%FT%T.%NZ
for process in /proc/[0-9]*; do
    [[ ${process##*/} != "$$" ]] || continue
    executable=$(readlink "$process/exe" || true)
    directory=$(readlink "$process/cwd" || true)
    case "$executable:$directory" in
        "$owned"/*:*|*:"$owned"|*:"$owned"/*)
            printf 'refusing cleanup: live process %s executable=%s cwd=%s\n' "$process" "$executable" "$directory" >&2
            exit 75
            ;;
    esac
done
du -sh -- "$owned"
find "$owned" -xdev -depth -delete
[[ ! -e "$owned" && ! -L "$owned" ]]
printf 'removed_owned_path=%s\n' "$owned"
date -u +%FT%T.%NZ
