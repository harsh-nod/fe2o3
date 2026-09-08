#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$EXAMPLE_DIR/../.." && pwd)
ROOT_TARGET=${FE2O3_ROOT_TARGET_DIR:-$REPO_ROOT/target}
HIP_BINARY=$(mktemp "${TMPDIR:-/tmp}/fe2o3-equivalent-hip.XXXXXX")
trap 'rm -f -- "$HIP_BINARY"' EXIT
PROCESSOR=${FE2O3_GENERAL_GEMM_TARGET:-gfx942}
if [[ "$PROCESSOR" != gfx942 && "$PROCESSOR" != gfx950 ]]; then
    printf 'FE2O3_GENERAL_GEMM_TARGET must be gfx942 or gfx950\n' >&2
    exit 2
fi

FE2O3_BENCHMARK=1 FE2O3_ROOT_TARGET_DIR="$ROOT_TARGET" \
    "$EXAMPLE_DIR/run-$PROCESSOR.sh"
/opt/rocm/bin/hipcc -O3 --offload-arch="$PROCESSOR" \
    "$EXAMPLE_DIR/benchmark_hip.cpp" -o "$HIP_BINARY"
"$HIP_BINARY"
