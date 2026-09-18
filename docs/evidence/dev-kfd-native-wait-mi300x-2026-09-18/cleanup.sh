#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-native-wait-20260918.LiBKebIz && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-native-wait-fcd5a89a113c6538338598bfcdb6769fb5c06042 ]]
date -u +%FT%T.%NZ
for process in /proc/[0-9]*; do
    [[ ${process##*/} != "$$" ]] || continue
    executable=$(readlink "$process/exe" || true)
    directory=$(readlink "$process/cwd" || true)
    case "$executable:$directory" in
        "$owned"/*:*|*:"$owned"|*:"$owned"/*)
            printf 'refusing cleanup: live process %s executable=%s cwd=%s\n' "$process" "$executable" "$directory" >&2
            exit 75 ;;
    esac
done
du -sh "$owned"
find "$owned" -xdev -depth -delete
[[ ! -e "$owned" && ! -L "$owned" ]]
printf 'removed_owned_directory=%s absent=yes\n' "$owned"
date -u +%FT%T.%NZ
