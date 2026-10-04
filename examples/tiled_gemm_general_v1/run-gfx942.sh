#!/usr/bin/env bash
set -euo pipefail

FE2O3_EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
FE2O3_EXAMPLE_CRATE=fe2o3_tiled_gemm_general_v1
FE2O3_EXAMPLE_STEM=tiled_gemm_general_v1
FE2O3_EXAMPLE_HOST_BIN=fe2o3-tiled-gemm-general-v1
FE2O3_EXAMPLE_HSACO_ENV=FE2O3_GENERAL_GEMM_HSACO
FE2O3_EXAMPLE_LINK_DEVICE_LIBS=0

source "$FE2O3_EXAMPLE_DIR/../run-gfx942-common.sh"
fe2o3_run_gfx942
