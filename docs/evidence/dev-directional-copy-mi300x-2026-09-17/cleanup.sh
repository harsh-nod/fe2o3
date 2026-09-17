#!/usr/bin/env bash
set -euo pipefail
owned=/home/harsh/fe2o3-copy-diagnostic-20260917.6CF3d70A
[[ -d "$owned" && -O "$owned" && ! -L "$owned" ]]
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
du -sh "$owned"
find "$owned" -xdev -depth -delete
[[ ! -e "$owned" && ! -L "$owned" ]]
printf 'removed_owned_directory=%s absent=yes\n' "$owned"
date -u +%FT%T.%NZ
