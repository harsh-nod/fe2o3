#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
name=${1:?name}
shift
[[ $name =~ ^[a-z0-9-]+$ && ! -e "$archive/SHA256SUMS" ]]
mkdir -p "$archive/review"
[[ ! -e "$archive/review/$name.command" ]]
printf '%q ' "$@" > "$archive/review/$name.command"
printf '\n' >> "$archive/review/$name.command"
date -u +%FT%T.%NZ > "$archive/review/$name.started"
if "$@" > "$archive/review/$name.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$archive/review/$name.finished"
printf '%s\n' "$status" > "$archive/review/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
