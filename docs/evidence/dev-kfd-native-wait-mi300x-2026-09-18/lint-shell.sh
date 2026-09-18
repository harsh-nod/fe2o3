#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
for script in "$archive"/*.sh; do
    bash -n "$script"
    printf 'parsed=%s\n' "${script##*/}"
done
