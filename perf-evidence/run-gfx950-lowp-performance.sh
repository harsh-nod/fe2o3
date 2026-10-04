#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
GPU=${FE2O3_GFX950_GPU:-0}
export ROCR_VISIBLE_DEVICES=$GPU
export FE2O3_GFX950_LOWP_PERF=1

for runner in \
    run-fp4-gemm-gfx950.sh \
    run-fp8-gemm-gfx950.sh \
    run-fp4-attention-gfx950.sh \
    run-fp8-attention-gfx950.sh
do
    "$REPO_ROOT/examples/gfx950_low_precision/$runner"
done

if [[ ${FE2O3_RUN_HIPBLASLT_ALL:-0} == 1 ]]; then
    HIPBLASLT_BENCH=${HIPBLASLT_BENCH:-/opt/rocm/bin/hipblaslt-bench}
    env -u HIP_VISIBLE_DEVICES "$HIPBLASLT_BENCH" \
        -m 16 -n 16 -k 128 \
        --a_type f8_r --b_type f8_r --c_type f32_r --d_type f32_r \
        --compute_type f32_r --batch_count 16 \
        --stride_a 2048 --stride_b 2048 --stride_c 256 --stride_d 256 \
        --transA N --transB N --algo_method all \
        --iters 100 --cold_iters 20 --use_gpu_timer --verify --print_kernel_info
fi
