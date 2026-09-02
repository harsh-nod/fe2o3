#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <gfx950-code-object>" >&2
    exit 2
fi

ROCM_PATH=${ROCM_PATH:-/opt/rocm}
OBJDUMP=${OBJDUMP:-$ROCM_PATH/llvm/bin/llvm-objdump}
CODE_OBJECT=$1
DISASSEMBLY=$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx950-advanced-isa.XXXXXX")
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
    if [[ -z $body ]] || ! grep -Fq -- "$mnemonic" <<<"$body"; then
        echo "missing '$mnemonic' in $kernel" >&2
        exit 1
    fi
    echo "ISA PASS $kernel: $mnemonic"
}

require_count() {
    local kernel=$1
    local mnemonic=$2
    local expected=$3
    local body count
    body=$(kernel_body "$kernel")
    count=$(grep -Fc -- "$mnemonic" <<<"$body" || true)
    if [[ $count -ne $expected ]]; then
        echo "expected $expected '$mnemonic' in $kernel, found $count" >&2
        exit 1
    fi
}

require_before() {
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

for kernel in gfx950_content_sparse_attention gfx950_compressed_hybrid_attention; do
    require_in_kernel "$kernel" "v_mfma_f32_16x16x128_f8f6f4"
    require_in_kernel "$kernel" "ds_read_b64_tr_b8"
    require_count "$kernel" "v_mfma_f32_16x16x128_f8f6f4" 1
    require_count "$kernel" "ds_read_b64_tr_b8" 4
    require_before "$kernel" "ds_read_b64_tr_b8" "v_mfma_f32_16x16x128_f8f6f4"
done

require_in_kernel gfx950_kda_decode "ds_bpermute_b32"
require_in_kernel gfx950_kda_chunkwise_prefill "ds_bpermute_b32"
require_in_kernel gfx950_attnres_aggregate "v_exp_f32"
require_in_kernel gfx950_four_branch_residual "v_exp_f32"
require_in_kernel gfx950_mhc_sinkhorn_mix "v_exp_f32"
