#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
name=${1:?name}
shift
[[ $name =~ ^[a-z0-9-]+$ ]]
mkdir -p "$archive/review"
[[ ! -e "$archive/review/$name.command" ]]
cd "$archive"
printf '%q ' "$@" > "$archive/review/$name.command"
printf '\n' >> "$archive/review/$name.command"
date -u +%FT%T.%NZ > "$archive/review/$name.started"
if "$@" > "$archive/review/$name.stdout" 2> "$archive/review/$name.stderr"; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$archive/review/$name.finished"
printf '%s\n' "$status" > "$archive/review/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
