#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" && ! -e "$archive/qualification-audit.command" ]]
# Keep the audit outside raw/: the auditor requires every raw command closed.
printf '%q ' bash "$archive/audit-cpu.sh" > "$archive/qualification-audit.command"
printf '\n' >> "$archive/qualification-audit.command"
date -u +%FT%T.%NZ > "$archive/qualification-audit.started"
if bash "$archive/audit-cpu.sh" > "$archive/qualification-audit.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$archive/qualification-audit.finished"
printf '%s\n' "$status" > "$archive/qualification-audit.exit"
[[ $status == 0 ]]
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check SHA256SUMS
