#!/usr/bin/env bash
set -euo pipefail
owned=/tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS
date -u +%FT%T.%NZ
[[ ! -e "$owned" && ! -L "$owned" ]]
for process in /proc/[0-9]*; do
    executable=$(readlink "$process/exe" || true)
    directory=$(readlink "$process/cwd" || true)
    case "$executable:$directory" in
        "$owned"/*:*|*:"$owned"|*:"$owned"/*)
            printf 'owned_process_reference=%s\n' "${process##*/}" >&2
            exit 75 ;;
    esac
done
printf 'owned_directory_absent=yes owned_process_references=0\n'
date -u +%FT%T.%NZ
