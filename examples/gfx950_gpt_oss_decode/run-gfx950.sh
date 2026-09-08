#!/usr/bin/env bash
if ! source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)/scripts/tutorial-hardware-runner.sh"; then
    exit 1
fi
fe2o3_tutorial_hardware_entry "$0" "$@" || exit
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
COMMON=$SCRIPT_DIR/../gfx950_advanced_attention/run-gfx950.sh
if [[ ! -x $COMMON ]]; then
    printf 'shared production gfx950 runner is unavailable: %s\n' "$COMMON" >&2
    exit 1
fi
FE2O3_ADVANCED_SUITE=gpt_oss FE2O3_ADVANCED_SCRIPT_DIR=$SCRIPT_DIR exec "$COMMON" kernel-gpt-oss-decode
