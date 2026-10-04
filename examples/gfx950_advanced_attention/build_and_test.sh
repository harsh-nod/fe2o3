#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROCM_PATH=${ROCM_PATH:-/opt/rocm}
HIPCC=${HIPCC:-$ROCM_PATH/bin/hipcc}
BUILD_DIR=${BUILD_DIR:-$EXAMPLE_DIR/build}
mkdir -p "$BUILD_DIR"

SOURCE="$EXAMPLE_DIR/gfx950_advanced_attention.hip"
BINARY="$BUILD_DIR/gfx950_advanced_attention"
BUNDLE="$BUILD_DIR/gfx950_advanced_attention.bundle"
CODE_OBJECT="$BUILD_DIR/gfx950_advanced_attention.hsaco"
"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 "$SOURCE" -o "$BINARY"
"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 --genco "$SOURCE" -o "$BUNDLE"

if "$ROCM_PATH/llvm/bin/llvm-readelf" -h "$BUNDLE" >/dev/null 2>&1; then
    cp -- "$BUNDLE" "$CODE_OBJECT"
else
    BUNDLE_TARGET=$(
        "$ROCM_PATH/llvm/bin/clang-offload-bundler" --list --type=o --input="$BUNDLE" \
            | awk '/amdgcn.*gfx950/ { print; exit }')
    if [[ -z $BUNDLE_TARGET ]]; then
        echo "gfx950 image not found in offload bundle" >&2
        exit 1
    fi
    "$ROCM_PATH/llvm/bin/clang-offload-bundler" --unbundle --type=o \
        --targets="$BUNDLE_TARGET" --input="$BUNDLE" --output="$CODE_OBJECT"
fi

"$EXAMPLE_DIR/check_isa.sh" "$CODE_OBJECT"
if [[ ${1:-} == --compile-only ]]; then
    exit 0
fi
RUNTIME_INFO=$("$ROCM_PATH/bin/rocminfo" 2>/dev/null || true)
if ! grep -q 'Name:.*gfx950' <<<"$RUNTIME_INFO"; then
    echo "No gfx950 runtime device is visible; compilation and ISA validation passed." >&2
    exit 2
fi
"$BINARY"
