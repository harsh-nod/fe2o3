#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <gfx950-code-object>" >&2
    exit 2
fi

ROCM_PATH=${ROCM_PATH:-/opt/rocm}
OBJDUMP=${OBJDUMP:-$ROCM_PATH/llvm/bin/llvm-objdump}
CODE_OBJECT=$1
DISASSEMBLY=$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx950-isa.XXXXXX")
trap 'rm -f -- "$DISASSEMBLY"' EXIT

"$OBJDUMP" --disassemble --mcpu=gfx950 "$CODE_OBJECT" >"$DISASSEMBLY"

kernel_body() {
    local kernel=$1
    awk -v marker="<${kernel}>:" '
        index($0, marker) { capture = 1 }
        capture && /^[[:xdigit:]]+ <[^>]+>:/ && !index($0, marker) { exit }
        capture { print }
    ' "$DISASSEMBLY"
}

require_in_kernel() {
    local kernel=$1
    local mnemonic=$2
    local body
    body=$(kernel_body "$kernel")
    if [[ -z "$body" ]] || ! grep -Fq -- "$mnemonic" <<<"$body"; then
        echo "missing '$mnemonic' in $kernel" >&2
        exit 1
    fi
    echo "ISA PASS $kernel: $mnemonic"
}

require_in_kernel gfx950_fp4_gemm "v_mfma_f32_16x16x128_f8f6f4"
require_in_kernel gfx950_fp4_gemm "cbsz:4 blgp:4"
require_in_kernel gfx950_fp8_gemm "v_mfma_f32_16x16x128_f8f6f4"
require_in_kernel gfx950_fp4_flash_attention "v_mfma_f32_16x16x128_f8f6f4"
require_in_kernel gfx950_fp4_flash_attention "cbsz:4 blgp:4"
require_in_kernel gfx950_fp4_flash_attention "ds_read_b64_tr_b4"
require_in_kernel gfx950_fp8_flash_attention "v_mfma_f32_16x16x128_f8f6f4"
require_in_kernel gfx950_fp8_flash_attention "ds_read_b64_tr_b8"

reject_in_kernel() {
    local kernel=$1
    local mnemonic=$2
    local body
    body=$(kernel_body "$kernel")
    if grep -Fq -- "$mnemonic" <<<"$body"; then
        echo "unexpected '$mnemonic' in $kernel" >&2
        exit 1
    fi
}

reject_in_kernel gfx950_fp8_gemm "cbsz:4"
reject_in_kernel gfx950_fp8_flash_attention "cbsz:4"

require_count_in_kernel() {
    local kernel=$1
    local mnemonic=$2
    local expected=$3
    local body count
    body=$(kernel_body "$kernel")
    count=$(grep -Fc -- "$mnemonic" <<<"$body" || true)
    if [[ $count -ne $expected ]]; then
        echo "expected $expected '$mnemonic' instructions in $kernel, found $count" >&2
        exit 1
    fi
}

require_before_in_kernel() {
    local kernel=$1
    local first=$2
    local second=$3
    local body first_line second_line
    body=$(kernel_body "$kernel")
    first_line=$(awk -v needle="$first" 'index($0, needle) { print NR; exit }' <<<"$body")
    second_line=$(awk -v needle="$second" 'index($0, needle) { print NR; exit }' <<<"$body")
    if [[ -z $first_line || -z $second_line || $first_line -ge $second_line ]]; then
        echo "expected '$first' before '$second' in $kernel" >&2
        exit 1
    fi
}

for kernel in \
    gfx950_fp4_gemm \
    gfx950_fp8_gemm \
    gfx950_fp4_flash_attention \
    gfx950_fp8_flash_attention; do
    require_count_in_kernel "$kernel" "v_mfma_f32_16x16x128_f8f6f4" 1
done
require_count_in_kernel gfx950_fp4_flash_attention "ds_read_b64_tr_b4" 2
require_count_in_kernel gfx950_fp8_flash_attention "ds_read_b64_tr_b8" 4
require_before_in_kernel \
    gfx950_fp4_flash_attention "ds_read_b64_tr_b4" "v_mfma_f32_16x16x128_f8f6f4"
require_before_in_kernel \
    gfx950_fp8_flash_attention "ds_read_b64_tr_b8" "v_mfma_f32_16x16x128_f8f6f4"

# GEMM output must come from the one MFMA above, not a scalar fallback loop.
for kernel in gfx950_fp4_gemm gfx950_fp8_gemm; do
    reject_in_kernel "$kernel" "v_fma_f32"
    reject_in_kernel "$kernel" "v_fmac_f32"
    reject_in_kernel "$kernel" "v_dot"
done
