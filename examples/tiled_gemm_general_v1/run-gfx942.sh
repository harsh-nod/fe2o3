#!/usr/bin/env bash
if ! source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)/scripts/tutorial-hardware-runner.sh"; then
    exit 1
fi
fe2o3_tutorial_hardware_entry "$0" "$@" || exit
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export FE2O3_GENERAL_GEMM_TARGET=gfx942

source "$EXAMPLE_DIR/run-target.sh"
fe2o3_run_general_gemm_target
