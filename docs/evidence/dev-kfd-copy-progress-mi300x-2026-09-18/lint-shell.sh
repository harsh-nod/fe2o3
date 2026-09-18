#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
for name in create.sh prepare.sh guard.sh run.sh inspect.sh cleanup.sh record.sh seal.sh lint-shell.sh; do
    bash -n "$archive/$name"
    printf 'syntax_ok=%s\n' "$name"
done
