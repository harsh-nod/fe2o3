#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROCM_PATH=${ROCM_PATH:-/opt/rocm}
HIPCC=${HIPCC:-$ROCM_PATH/bin/hipcc}
BUILD_DIR=${BUILD_DIR:-$EXAMPLE_DIR/build}
mkdir -p "$BUILD_DIR"

"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 \
    "$EXAMPLE_DIR/gfx950_low_precision.hip" -o "$BUILD_DIR/gfx950_low_precision"
GFX950_BUNDLE="$BUILD_DIR/gfx950_low_precision.bundle"
"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 --genco \
    "$EXAMPLE_DIR/gfx950_low_precision.hip" -o "$GFX950_BUNDLE"
"$ROCM_PATH/llvm/bin/clang-offload-bundler" --unbundle --type=o \
    --targets=hipv4-amdgcn-amd-amdhsa--gfx950 \
    --input="$GFX950_BUNDLE" --output="$BUILD_DIR/gfx950_low_precision.hsaco"
"$EXAMPLE_DIR/check_isa.sh" "$BUILD_DIR/gfx950_low_precision.hsaco"

if [[ ${1:-} == --compile-only ]]; then
    exit 0
fi

RUNTIME_AGENTS=$("$ROCM_PATH/bin/rocminfo" 2>/dev/null)
if ! grep -q 'Name:.*gfx950' <<<"$RUNTIME_AGENTS"; then
    echo "No gfx950 runtime device is visible; compilation and ISA validation passed." >&2
    echo "Re-run on a host/container exposing the MI350 GPU, or pass --compile-only." >&2
    exit 2
fi

"$BUILD_DIR/gfx950_low_precision"
