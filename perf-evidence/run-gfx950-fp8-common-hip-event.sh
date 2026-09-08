#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'usage: %s <absolute-fe2o3-fp8-hsaco> [raw.csv]\n' "$0" >&2
    exit 2
fi

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd -P)
ROCM_PATH=${ROCM_PATH:-/opt/rocm}
HIPCC=${HIPCC:-$ROCM_PATH/bin/hipcc}
HSACO=$1
RAW=${2:-$REPO_ROOT/perf-raw/gfx950-fp8-common-hip-event.csv}
BINARY=${FE2O3_GFX950_FP8_COMPARATOR_BINARY:-$REPO_ROOT/target/perf-evidence/gfx950-fp8-common-hip-event}

if [[ $HSACO != /* || ! -f $HSACO || -L $HSACO ]]; then
    printf 'HSACO must be an absolute regular non-symlink file: %s\n' "$HSACO" >&2
    exit 1
fi
if [[ ! -x $HIPCC ]]; then
    printf 'hipcc is unavailable: %s\n' "$HIPCC" >&2
    exit 1
fi

mkdir -p -- "$(dirname -- "$BINARY")" "$(dirname -- "$RAW")"
"$HIPCC" -O3 -std=c++17 \
    "$SCRIPT_DIR/gfx950-fp8-common-hip-event.cpp" \
    -lhipblaslt -o "$BINARY"
"$BINARY" "$HSACO" "$RAW"
printf 'RAW: %s\n' "$RAW"
