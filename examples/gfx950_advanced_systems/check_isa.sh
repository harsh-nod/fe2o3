#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <gfx950-code-object>" >&2
    exit 2
fi

ROCM_PATH=${ROCM_PATH:-/opt/rocm}
OBJDUMP=${OBJDUMP:-$ROCM_PATH/llvm/bin/llvm-objdump}
CODE_OBJECT=$1
DISASSEMBLY=$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx950-systems-isa.XXXXXX")
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

require_symbol() {
    local kernel=$1
    if [[ -z $(kernel_body "$kernel") ]]; then
        echo "missing kernel symbol '$kernel'" >&2
        exit 1
    fi
    echo "ISA PASS $kernel: symbol"
}

require_in_kernel() {
    local kernel=$1
    local mnemonic=$2
    local body
    body=$(kernel_body "$kernel")
    if [[ -z $body ]] || ! grep -Fq -- "$mnemonic" <<<"$body"; then
        echo "missing '$mnemonic' in $kernel" >&2
        exit 1
    fi
    echo "ISA PASS $kernel: $mnemonic"
}

for kernel in \
    gfx950_fused_fp4_fp8_moe \
    gfx950_expert_parallel_rank \
    gfx950_combine_expert_ranks \
    gfx950_speculative_transaction \
    gfx950_qwen_ngram_gather \
    gfx950_stage_gradient_shard \
    gfx950_muon_update; do
    require_symbol "$kernel"
done

require_in_kernel gfx950_fused_fp4_fp8_moe \
    "v_mfma_f32_16x16x128_f8f6f4"
require_in_kernel gfx950_fused_fp4_fp8_moe "cbsz:4"

MFMA_COUNT=$(kernel_body gfx950_fused_fp4_fp8_moe \
    | grep -Fc -- "v_mfma_f32_16x16x128_f8f6f4" || true)
if [[ $MFMA_COUNT -ne 1 ]]; then
    echo "expected exactly one scaled MFMA in fused MoE, found $MFMA_COUNT" >&2
    exit 1
fi
echo "ISA PASS gfx950_fused_fp4_fp8_moe: mfma_count=1"
