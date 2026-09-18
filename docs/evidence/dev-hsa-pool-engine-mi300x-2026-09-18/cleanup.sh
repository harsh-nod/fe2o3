#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-hsa-pool-engine-20260918.wBkvxkrs && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-pool-engine-3da2d25ac965afa9845b8ab1246e5fdf13c5d821 ]]
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
