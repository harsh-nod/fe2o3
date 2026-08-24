#!/bin/bash
set -euo pipefail

SCRIPT_DIRECTORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIRECTORY

export FE2O3_RETAINED_RUNTIME_PROFILE=functional-refinement-v1
exec "$SCRIPT_DIRECTORY/general-gemm-verus-runtime-v2.sh" "$@"
