#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROCM_PATH=${ROCM_PATH:-/opt/rocm}
HIPCC=${HIPCC:-$ROCM_PATH/bin/hipcc}
BUILD_DIR=${BUILD_DIR:-$EXAMPLE_DIR/build}
SOURCE=$EXAMPLE_DIR/gfx950_advanced_systems.hip
mkdir -p "$BUILD_DIR"

"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 \
    "$SOURCE" -o "$BUILD_DIR/gfx950_advanced_systems"

GFX950_BUNDLE=$BUILD_DIR/gfx950_advanced_systems.bundle
GFX950_HSACO=$BUILD_DIR/gfx950_advanced_systems.hsaco
"$HIPCC" -std=c++17 -O2 --offload-arch=gfx950 --genco \
    "$SOURCE" -o "$GFX950_BUNDLE"
if "$ROCM_PATH/llvm/bin/llvm-readelf" -h "$GFX950_BUNDLE" >/dev/null 2>&1; then
    cp -- "$GFX950_BUNDLE" "$GFX950_HSACO"
else
    BUNDLE_TARGET=$(
        "$ROCM_PATH/llvm/bin/clang-offload-bundler" --list --type=o \
            --input="$GFX950_BUNDLE" | awk '/amdgcn.*gfx950/ { print; exit }')
    if [[ -z $BUNDLE_TARGET ]]; then
        echo "gfx950 image not found in offload bundle" >&2
        exit 1
    fi
    "$ROCM_PATH/llvm/bin/clang-offload-bundler" --unbundle --type=o \
        --targets="$BUNDLE_TARGET" --input="$GFX950_BUNDLE" --output="$GFX950_HSACO"
fi
"$EXAMPLE_DIR/check_isa.sh" "$GFX950_HSACO"

NEGATIVE_LOG=$BUILD_DIR/gfx942-rejection.log
if "$HIPCC" -std=c++17 -O2 --offload-arch=gfx942 --genco \
    "$SOURCE" -o "$BUILD_DIR/invalid-gfx942.bundle" >"$NEGATIVE_LOG" 2>&1; then
    echo "gfx942 unexpectedly accepted the gfx950-only builtins" >&2
    exit 1
fi
if ! grep -Eq 'not supported|needs target feature|cannot compile|unsupported' "$NEGATIVE_LOG"; then
    echo "gfx942 compilation failed without the expected target rejection diagnostic" >&2
    cat "$NEGATIVE_LOG" >&2
    exit 1
fi
echo "TARGET PASS gfx942 rejects gfx950 scaled MFMA"

echo "SOURCE_SHA256 $(sha256sum "$SOURCE" | awk '{print $1}')"
echo "HSACO_SHA256 $(sha256sum "$GFX950_HSACO" | awk '{print $1}')"

if [[ ${1:-} == --compile-only ]]; then
    exit 0
fi

RUNTIME_AGENTS=$("$ROCM_PATH/bin/rocminfo" 2>/dev/null || true)
if ! grep -q 'Name:.*gfx950' <<<"$RUNTIME_AGENTS"; then
    echo "No gfx950 runtime device is visible; compilation, rejection, and ISA checks passed." >&2
    echo "Re-run on a host exposing MI350, or pass --compile-only." >&2
    exit 2
fi

"$BUILD_DIR/gfx950_advanced_systems"
