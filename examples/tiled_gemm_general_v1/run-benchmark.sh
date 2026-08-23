#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$EXAMPLE_DIR/../.." && pwd)
ROOT_TARGET=${FE2O3_ROOT_TARGET_DIR:-$REPO_ROOT/target}
HIP_BINARY=$(mktemp "${TMPDIR:-/tmp}/fe2o3-equivalent-hip.XXXXXX")
trap 'rm -f -- "$HIP_BINARY"' EXIT

FE2O3_BENCHMARK=1 FE2O3_ROOT_TARGET_DIR="$ROOT_TARGET" \
    "$EXAMPLE_DIR/run-gfx942.sh"
/opt/rocm/bin/hipcc -O3 --offload-arch=gfx942 \
    "$EXAMPLE_DIR/benchmark_hip.cpp" -o "$HIP_BINARY"
"$HIP_BINARY"
