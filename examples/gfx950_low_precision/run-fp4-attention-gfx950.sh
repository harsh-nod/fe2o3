#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 0 ]]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
exec "$SCRIPT_DIR/run-attention-gfx950.sh" fp4
