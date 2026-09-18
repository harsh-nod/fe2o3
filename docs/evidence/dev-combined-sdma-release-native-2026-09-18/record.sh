#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
name=${1:?record name}
shift
[[ $name =~ ^[a-z0-9-]+$ ]]
mkdir -p -- "$archive/audit"
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/audit/$name.command" ]]
cd -- "$root"
printf '%q' "$1" > "$archive/audit/$name.command"
for arg in "${@:2}"; do printf ' %q' "$arg" >> "$archive/audit/$name.command"; done
printf '\n' >> "$archive/audit/$name.command"
date -u +%FT%T.%NZ > "$archive/audit/$name.started"
if "$@" > "$archive/audit/$name.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$archive/audit/$name.finished"
printf '%s\n' "$status" > "$archive/audit/$name.exit"
printf '%s exit=%s\n' "$name" "$status"
exit "$status"
