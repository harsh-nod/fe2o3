#!/usr/bin/env bash
set -Eeuo pipefail

# A compile entry point, not a hardware launch adapter.
if [[ ${FE2O3_EXAMPLE_COMPILE_ONLY:-1} != 1 ]]; then
    printf '%s\n' 'this runner requires FE2O3_EXAMPLE_COMPILE_ONLY=1' >&2
    exit 2
fi
FE2O3_EXAMPLE_COMPILE_ONLY=1
FE2O3_EXAMPLE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
FE2O3_EXAMPLE_CRATE=fe2o3_moe_top2_v1
FE2O3_EXAMPLE_STEM=moe_top2_v1
FE2O3_EXAMPLE_HOST_BIN=fe2o3-moe-top2-v1
FE2O3_EXAMPLE_HSACO_ENV=FE2O3_MOE_TOP2_HSACO
FE2O3_EXAMPLE_LINK_DEVICE_LIBS=0
source "$FE2O3_EXAMPLE_DIR/../run-gfx942-common.sh"
fe2o3_run_gfx942
