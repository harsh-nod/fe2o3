#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
name=${1:?record name}
shift
[[ $name =~ ^[a-z0-9-]+$ ]]
mkdir -p -- "$archive/raw"
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/raw/$name.command" ]]
cd -- "$root"
printf '%q' "$1" > "$archive/raw/$name.command"
for arg in "${@:2}"; do printf ' %q' "$arg" >> "$archive/raw/$name.command"; done
printf '\n' >> "$archive/raw/$name.command"
date -u +%FT%T.%NZ > "$archive/raw/$name.started"
if "$@" > "$archive/raw/$name.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$archive/raw/$name.finished"
printf '%s\n' "$status" > "$archive/raw/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
