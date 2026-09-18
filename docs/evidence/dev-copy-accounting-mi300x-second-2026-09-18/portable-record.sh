#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
name=${1:?record name}
shift
[[ $name =~ ^[a-z0-9-]+$ ]]
mkdir -p -- "$archive/portable"
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/portable/$name.command" ]]
cd -- "$archive"
printf '%q ' "$@" > "$archive/portable/$name.command"
printf '\n' >> "$archive/portable/$name.command"
date -u +%FT%T.%NZ > "$archive/portable/$name.started"
if "$@" > "$archive/portable/$name.stdout" 2> "$archive/portable/$name.stderr"; then
    status=0
else
    status=$?
fi
date -u +%FT%T.%NZ > "$archive/portable/$name.finished"
printf '%s\n' "$status" > "$archive/portable/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
