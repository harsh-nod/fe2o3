#!/usr/bin/env bash
set -Eeuo pipefail

# A compile entry point, not a hardware launch adapter.
if [[ ${FE2O3_EXAMPLE_COMPILE_ONLY:-1} != 1 ]]; then
    printf '%s\n' 'this runner requires FE2O3_EXAMPLE_COMPILE_ONLY=1' >&2
    exit 2
fi
FE2O3_EXAMPLE_COMPILE_ONLY=1
FE2O3_EXAMPLE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
FE2O3_EXAMPLE_CRATE=fe2o3_wave64_collectives_v1
FE2O3_EXAMPLE_STEM=wave64_collectives_v1
FE2O3_EXAMPLE_HOST_BIN=fe2o3-wave64-collectives-v1
FE2O3_EXAMPLE_HSACO_ENV=FE2O3_WAVE64_COLLECTIVES_HSACO
FE2O3_EXAMPLE_LINK_DEVICE_LIBS=0
source "$FE2O3_EXAMPLE_DIR/../run-gfx942-common.sh"
fe2o3_run_gfx942
