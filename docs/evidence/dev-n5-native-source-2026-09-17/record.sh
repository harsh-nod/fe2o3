#!/usr/bin/env bash
set -euo pipefail

archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
out="$archive/${1:?output subdirectory}"
name=${2:?record name}
shift 2
mkdir -p -- "$out"
[[ ! -e "$out/$name.command" ]]
cd -- "$root"
printf '%q ' "$@" > "$out/$name.command"
printf '\n' >> "$out/$name.command"
date -u +%FT%T.%NZ > "$out/$name.started"
if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$out/$name.finished"
printf '%s\n' "$status" > "$out/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
