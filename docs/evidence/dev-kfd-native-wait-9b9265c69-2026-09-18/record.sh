#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
name=${1:?name}
shift
[[ $name =~ ^[a-z0-9-]+$ ]]
mkdir -p "$stage/raw"
[[ ! -e "$stage/raw/$name.command" ]]
cd "$stage"
printf '%q ' "$@" > "$stage/raw/$name.command"
printf '\n' >> "$stage/raw/$name.command"
date -u +%FT%T.%NZ > "$stage/raw/$name.started"
if "$@" > "$stage/raw/$name.stdout" 2> "$stage/raw/$name.stderr"; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$stage/raw/$name.finished"
printf '%s\n' "$status" > "$stage/raw/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
