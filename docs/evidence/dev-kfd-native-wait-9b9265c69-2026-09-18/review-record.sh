#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "$0")" && pwd)
name=${1:?name}
shift
[[ "$name" =~ ^[a-z0-9-]+$ ]]
mkdir -p "$root/review"
[[ ! -e "$root/review/$name.command" ]]
cd "$root"
printf '%q ' "$@" > "$root/review/$name.command"
printf '\n' >> "$root/review/$name.command"
date -u +%FT%T.%NZ > "$root/review/$name.started"
if "$@" > "$root/review/$name.stdout" 2> "$root/review/$name.stderr"; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$root/review/$name.finished"
printf '%s\n' "$status" > "$root/review/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
