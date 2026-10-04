#!/usr/bin/env bash
set -euo pipefail

# Builds one ordinary Rust GEMM kernel through the production extractor, proves
# the retained gfx950 LLVM/ISA profile, then runs the corresponding digest-
# pinned numerical HSA test. Host paths can be overridden for SSH runs.

if [[ $# -ne 1 ]]; then
    printf 'usage: %s <fp4|fp8>\n' "$0" >&2
    exit 2
fi

PRECISION=$1
case $PRECISION in
    fp4)
        FEATURE=kernel-fp4-gemm
        SYMBOL=gfx950_fp4_gemm_rust
        LABEL='FP4 GEMM'
        SELECTORS='i32 4, i32 4, i32 0, i32 0, i32 0, i32 0)'
        WRONG_SELECTORS='i32 0, i32 0, i32 0, i32 0, i32 0, i32 0)'
        OUTPUT_ENV=FE2O3_GFX950_FP4_OUTPUT_DIR
        RUN_ENV=FE2O3_RUN_GFX950_FP4_GEMM_HARDWARE
        HSACO_ENV=FE2O3_GFX950_FP4_GEMM_HSACO
        SHA256_ENV=FE2O3_GFX950_FP4_GEMM_SHA256
        HARDWARE_TEST_TARGET=gfx950_fp4_gemm_hardware
        HARDWARE_TEST=gfx950_fp4_gemm_rust_cov6_runs_multigrid_and_matches_every_cpu_reference_output
        FORBIDDEN_INSTRUCTIONS=(v_cvt_f32_fp4 v_fma_f32 v_dot)
        ;;
    fp8)
        FEATURE=kernel-fp8-gemm
        SYMBOL=gfx950_fp8_gemm_rust
        LABEL='FP8 GEMM'
        SELECTORS='i32 0, i32 0, i32 0, i32 0, i32 0, i32 0)'
        WRONG_SELECTORS=
        OUTPUT_ENV=FE2O3_GFX950_FP8_OUTPUT_DIR
        RUN_ENV=FE2O3_RUN_GFX950_FP8_GEMM_HARDWARE
        HSACO_ENV=FE2O3_GFX950_FP8_GEMM_HSACO
        SHA256_ENV=FE2O3_GFX950_FP8_GEMM_SHA256
        HARDWARE_TEST_TARGET=gfx950_fp8_gemm_hardware
        HARDWARE_TEST=gfx950_fp8_gemm_rust_cov6_runs_multigrid_and_matches_every_cpu_reference_output
        FORBIDDEN_INSTRUCTIONS=(v_cvt_f32_fp4 v_cvt_f32_fp8 v_fma_f32 v_dot)
        ;;
    *)
        printf 'unsupported GEMM precision: %s (expected fp4 or fp8)\n' "$PRECISION" >&2
        exit 2
        ;;
esac

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=${FE2O3_REPO_ROOT:-$(cd -- "$SCRIPT_DIR/../.." && pwd -P)}
TOOLCHAIN=${FE2O3_RUST_TOOLCHAIN:-nightly-2026-04-03}
ROOT_TARGET_DIR=${FE2O3_ROOT_TARGET_DIR:-$REPO_ROOT/target}
OUTPUT_ROOT=${!OUTPUT_ENV:-$SCRIPT_DIR/target/fe2o3-gfx950-$PRECISION-gemm}
ROCM_PATH=${ROCM_PATH:-/opt/rocm}
RUSTUP=${RUSTUP:-rustup}
CARGO_BIN=${CARGO:-cargo}
CLANG=${CLANG:-$ROCM_PATH/llvm/bin/clang}
LD_LLD=${LD_LLD:-$ROCM_PATH/llvm/bin/ld.lld}
OBJDUMP=${OBJDUMP:-$ROCM_PATH/llvm/bin/llvm-objdump}
READOBJ=${READOBJ:-$ROCM_PATH/llvm/bin/llvm-readobj}
SHA256SUM=${SHA256SUM:-sha256sum}
COMPILE_ONLY=${FE2O3_EXAMPLE_COMPILE_ONLY:-0}

if [[ $COMPILE_ONLY != 0 && $COMPILE_ONLY != 1 ]]; then
    printf 'FE2O3_EXAMPLE_COMPILE_ONLY must be 0 or 1\n' >&2
    exit 2
fi

for executable in "$RUSTUP" "$CLANG" "$LD_LLD" "$OBJDUMP" "$READOBJ" "$SHA256SUM"; do
    if [[ ! -x $executable ]] && ! command -v -- "$executable" >/dev/null 2>&1; then
        printf 'required executable is unavailable: %s\n' "$executable" >&2
        exit 1
    fi
done
if [[ ! -f $REPO_ROOT/Cargo.toml || ! -f $SCRIPT_DIR/Cargo.toml ]]; then
    printf 'FE2O3_REPO_ROOT does not identify this fe2o3 checkout: %s\n' "$REPO_ROOT" >&2
    exit 1
fi

mkdir -p -- "$ROOT_TARGET_DIR" "$OUTPUT_ROOT"
ROOT_TARGET_DIR=$(cd -- "$ROOT_TARGET_DIR" && pwd -P)
OUTPUT_ROOT=$(cd -- "$OUTPUT_ROOT" && pwd -P)
ATTEMPT_DIR=$(mktemp -d "$OUTPUT_ROOT/attempt.XXXXXX")
chmod 700 "$ATTEMPT_DIR"

LLVM_IR=$ATTEMPT_DIR/gfx950-$PRECISION-gemm.ll
OBJECT=$ATTEMPT_DIR/gfx950-$PRECISION-gemm.o
HSACO=$ATTEMPT_DIR/gfx950-$PRECISION-gemm.hsaco
DISASSEMBLY=$ATTEMPT_DIR/gfx950-$PRECISION-gemm.isa
NOTES=$ATTEMPT_DIR/gfx950-$PRECISION-gemm.notes
BINDING_PATH=$ATTEMPT_DIR/crate-binding-v1
AMD_TARGET_DIR=$ATTEMPT_DIR/amdgpu-target

if [[ -n ${FE2O3_RUSTC_EXTRACTOR:-} ]]; then
    EXTRACTOR=$FE2O3_RUSTC_EXTRACTOR
else
    EXTRACTOR=$ROOT_TARGET_DIR/debug/fe2o3-rustc-extract
    CARGO_TARGET_DIR=$ROOT_TARGET_DIR \
        "$RUSTUP" run "$TOOLCHAIN" "$CARGO_BIN" build \
        --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
        -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
fi
if [[ ! -f $EXTRACTOR || -L $EXTRACTOR || ! -x $EXTRACTOR ]]; then
    printf 'generic rustc extractor must be a regular executable: %s\n' "$EXTRACTOR" >&2
    exit 1
fi

SYSROOT=$("$RUSTUP" run "$TOOLCHAIN" rustc --print sysroot)
(
    cd -- "$SCRIPT_DIR"
    FE2O3_EXTRACT_CRATE_V1=fe2o3_gfx950_low_precision \
    FE2O3_EXTRACT_CRATE_BINDING_PATH_V1=$BINDING_PATH \
    FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1=$LLVM_IR \
    RUSTC_WORKSPACE_WRAPPER=$EXTRACTOR \
    CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS='-Zalways-encode-mir -Ctarget-cpu=gfx950 -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack' \
    LD_LIBRARY_PATH="$ROOT_TARGET_DIR/debug/deps:$SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$RUSTUP" run "$TOOLCHAIN" "$CARGO_BIN" check \
        --release --locked -Zbuild-std=core \
        --target amdgcn-amd-amdhsa --target-dir "$AMD_TARGET_DIR" \
        --no-default-features --features "$FEATURE" --lib
)

if [[ ! -f $BINDING_PATH || -L $BINDING_PATH ]]; then
    printf 'compiler did not publish a regular crate-binding handoff\n' >&2
    exit 1
fi
if [[ $(stat -c '%a:%h:%F' "$BINDING_PATH") != '600:1:regular file' ]]; then
    printf 'compiler published an insecure crate-binding handoff\n' >&2
    exit 1
fi
if [[ $(wc -c < "$BINDING_PATH") -ne 65 ]]; then
    printf 'compiler published a malformed crate-binding handoff length\n' >&2
    exit 1
fi
IFS= read -r CRATE_BINDING < "$BINDING_PATH"
if [[ ! $CRATE_BINDING =~ ^[0-9a-f]{64}$ ]]; then
    printf 'compiler published a noncanonical crate-binding handoff\n' >&2
    exit 1
fi

require_llvm_line_count() {
    local needle=$1
    local expected=$2
    local description=$3
    local actual
    actual=$(awk -v needle="$needle" 'index($0, needle) { count += 1 } END { print count + 0 }' "$LLVM_IR")
    if [[ $actual -ne $expected ]]; then
        printf 'LLVM validation failed: expected %s %s, found %s\n' \
            "$expected" "$description" "$actual" >&2
        exit 1
    fi
}

require_llvm_line_count 'target triple = "amdgcn-amd-amdhsa"' 1 'AMDGPU HSA target triple'
require_llvm_line_count 'define amdgpu_kernel' 1 'kernel definition'
require_llvm_line_count "define amdgpu_kernel void @$SYMBOL(" 1 "$SYMBOL definition"
require_llvm_line_count '"target-cpu"="gfx950"' 1 'gfx950 function target binding'
require_llvm_line_count '"target-features"="-wavefrontsize32,+wavefrontsize64,-xnack"' 1 \
    'exact Wave64/xnack- function target binding'
require_llvm_line_count \
    'call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(' \
    1 "scaled $LABEL MFMA call"
require_llvm_line_count "$SELECTORS" 1 "$LABEL selectors and disabled scaling controls"
if [[ -n $WRONG_SELECTORS ]]; then
    require_llvm_line_count "$WRONG_SELECTORS" 0 "wrong low-precision selectors in $LABEL"
fi
require_llvm_line_count \
    'declare <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(' \
    1 "scaled $LABEL MFMA declaration"

"$CLANG" -O3 -nogpulib -x ir \
    --target=amdgcn-amd-amdhsa -mcpu=gfx950:xnack- \
    -mcode-object-version=6 -c "$LLVM_IR" -o "$OBJECT"
"$LD_LLD" -shared "$OBJECT" -o "$HSACO"

"$READOBJ" --file-headers --notes "$HSACO" > "$NOTES"
for required in \
    'Format: elf64-amdgpu' \
    'Machine: EM_AMDGPU' \
    'EF_AMDGPU_MACH_AMDGCN_GFX950' \
    'EF_AMDGPU_FEATURE_XNACK_OFF_V4'; do
    if ! grep -Fq -- "$required" "$NOTES"; then
        printf 'HSACO metadata validation failed: missing %s\n' "$required" >&2
        exit 1
    fi
done
for required in \
    "^[[:space:]]*amdhsa.target:[[:space:]]+'amdgcn-amd-amdhsa--gfx950:xnack-'[[:space:]]*$" \
    "^[[:space:]]*\\.name:[[:space:]]+$SYMBOL[[:space:]]*$" \
    "^[[:space:]]*\\.symbol:[[:space:]]+$SYMBOL[.]kd[[:space:]]*$"; do
    if ! grep -Eq -- "$required" "$NOTES"; then
        printf 'HSACO metadata validation failed: missing exact pattern %s\n' "$required" >&2
        exit 1
    fi
done
if ! awk '
    /amdhsa.version:/ { version = 1; next }
    version == 1 && /^[[:space:]]*-[[:space:]]+1[[:space:]]*$/ { version = 2; next }
    version == 2 && /^[[:space:]]*-[[:space:]]+2[[:space:]]*$/ { found = 1 }
    END { exit(found ? 0 : 1) }
' "$NOTES"; then
    printf 'HSACO metadata validation failed: missing COV6 metadata version 1.2\n' >&2
    exit 1
fi

"$OBJDUMP" --disassemble --mcpu=gfx950 "$HSACO" > "$DISASSEMBLY"
KERNEL_ISA=$(awk -v symbol="$SYMBOL" '
    index($0, "<" symbol ">:") { capture = 1 }
    capture && /^[[:xdigit:]]+ <[^>]+>:/ && index($0, "<" symbol ">:") == 0 { exit }
    capture { print }
' "$DISASSEMBLY")
if [[ -z $KERNEL_ISA ]]; then
    printf 'ISA validation failed: %s is absent\n' "$SYMBOL" >&2
    exit 1
fi
MFMA_COUNT=$(grep -Fc -- 'v_mfma_f32_16x16x128_f8f6f4' <<< "$KERNEL_ISA" || true)
if [[ $MFMA_COUNT -ne 1 ]]; then
    printf 'ISA validation failed: expected one gfx950 scaled %s MFMA, found %s\n' \
        "$LABEL" "$MFMA_COUNT" >&2
    exit 1
fi
for forbidden in "${FORBIDDEN_INSTRUCTIONS[@]}"; do
    if grep -Fq -- "$forbidden" <<< "$KERNEL_ISA"; then
        printf 'ISA validation failed: scalar/vector fallback instruction is present: %s\n' \
            "$forbidden" >&2
        exit 1
    fi
done

HSACO=$(cd -- "$(dirname -- "$HSACO")" && pwd -P)/$(basename -- "$HSACO")
if [[ $COMPILE_ONLY == 1 ]]; then
    printf 'COMPILE PASS: %s reached validated gfx950:xnack- HSACO; hardware execution skipped\n' "$SYMBOL"
    printf 'HSACO: %s\n' "$HSACO"
    exit 0
fi
HSACO_SHA256=$("$SHA256SUM" -- "$HSACO" | awk '{ print $1 }')
(
    cd -- "$REPO_ROOT"
    env \
        "$RUN_ENV=1" \
        "$HSACO_ENV=$HSACO" \
        "$SHA256_ENV=$HSACO_SHA256" \
        CARGO_TARGET_DIR="$ROOT_TARGET_DIR" \
        "$RUSTUP" run "$TOOLCHAIN" "$CARGO_BIN" test --locked \
            -p fe2o3-hsa-runtime --features hardware-qualification \
            --test "$HARDWARE_TEST_TARGET" \
            "$HARDWARE_TEST" \
            -- --ignored --exact --nocapture
)

printf 'PASS fe2o3 gfx950 %s production build and numerical run\n' "$LABEL"
printf 'LLVM:  %s\n' "$LLVM_IR"
printf 'HSACO: %s\n' "$HSACO"
printf 'SHA256: %s\n' "$HSACO_SHA256"
printf 'ISA:   %s\n' "$DISASSEMBLY"
