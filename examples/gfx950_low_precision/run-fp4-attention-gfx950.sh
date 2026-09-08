#!/usr/bin/env bash
if ! source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)/scripts/tutorial-hardware-runner.sh"; then
    exit 1
fi
fe2o3_tutorial_hardware_entry "$0" "$@" || exit
set -euo pipefail

if [[ $# -ne 0 ]]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
exec "$SCRIPT_DIR/run-attention-gfx950.sh" fp4
