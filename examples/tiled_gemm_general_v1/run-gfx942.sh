#!/usr/bin/env bash
set -euo pipefail

EXAMPLE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$EXAMPLE_DIR/../.." && pwd)
TOOLCHAIN=${FE2O3_RUST_TOOLCHAIN:-nightly-2026-04-03}
ROOT_TARGET=${FE2O3_ROOT_TARGET_DIR:-$REPO_ROOT/target}
OUTPUT_DIR=${FE2O3_OUTPUT_DIR:-$EXAMPLE_DIR/target/fe2o3-gfx942}
ROCM_DIR=${ROCM_PATH:-/opt/rocm}
WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-gfx942.XXXXXX")
trap 'rm -rf -- "$WORK_DIR"' EXIT

SYSROOT=$(rustup run "$TOOLCHAIN" rustc --print sysroot)
EXTRACTOR="$ROOT_TARGET/debug/fe2o3-rustc-extract"
LLVM_IR="$WORK_DIR/tiled_gemm_general_v1.ll"
OBJECT="$WORK_DIR/tiled_gemm_general_v1.o"
HSACO="$OUTPUT_DIR/tiled_gemm_general_v1.hsaco"
AMD_TARGET="$WORK_DIR/amd-target"

mkdir -p -- "$OUTPUT_DIR"
CARGO_TARGET_DIR="$ROOT_TARGET" rustup run "$TOOLCHAIN" cargo build \
    --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
    -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract

(
    cd -- "$EXAMPLE_DIR"
    FE2O3_EXTRACT_CRATE_V1=fe2o3_tiled_gemm_general_v1 \
    FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2=1111111111111111111111111111111111111111111111111111111111111111 \
    FE2O3_EXTRACT_GFX942_LLVM_PATH_V1="$LLVM_IR" \
    RUSTC_WORKSPACE_WRAPPER="$EXTRACTOR" \
    CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS='-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32' \
    LD_LIBRARY_PATH="$ROOT_TARGET/debug/deps:$SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    rustup run "$TOOLCHAIN" cargo check --release --locked -Zbuild-std=core \
        --target amdgcn-amd-amdhsa --target-dir "$AMD_TARGET" --lib
)

"$ROCM_DIR/llvm/bin/clang" -nogpulib -x ir \
    --target=amdgcn-amd-amdhsa -mcpu=gfx942 -mno-xnack \
    -c "$LLVM_IR" -o "$OBJECT"
"$ROCM_DIR/llvm/bin/ld.lld" -shared "$OBJECT" -o "$HSACO"

(
    cd -- "$EXAMPLE_DIR"
    FE2O3_GENERAL_GEMM_HSACO="$HSACO" \
    CARGO_TARGET_DIR="$ROOT_TARGET" \
    rustup run "$TOOLCHAIN" cargo run --release --locked --bin fe2o3-tiled-gemm-general-v1
)

printf 'HSACO: %s\n' "$HSACO"
