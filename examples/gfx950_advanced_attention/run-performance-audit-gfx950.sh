#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || $1 != /* || -e $1 ]]; then
    printf 'usage: %s <new absolute evidence directory>\n' "$0" >&2
    exit 2
fi

EVIDENCE_DIR=$1
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd -P)
TARGET_DIR=${CARGO_TARGET_DIR:-$REPO_ROOT/target/attention-performance-audit}
GPU=${FE2O3_PERF_PHYSICAL_GPU:-4}
EXTRACTOR=${FE2O3_RUSTC_EXTRACTOR:-$TARGET_DIR/debug/fe2o3-rustc-extract}
CARGO_BIN=${CARGO:-$HOME/.cargo/bin/cargo}

mkdir -p -- "$EVIDENCE_DIR/artifacts" "$EVIDENCE_DIR/logs" "$TARGET_DIR"
: > "$EVIDENCE_DIR/samples.jsonl"

if [[ ! -x $EXTRACTOR ]]; then
    CARGO_TARGET_DIR=$TARGET_DIR "$CARGO_BIN" build --locked \
        --manifest-path "$REPO_ROOT/Cargo.toml" \
        -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
fi

run_case() {
    local feature=$1 variant=$2
    env -u HIP_VISIBLE_DEVICES \
        ROCR_VISIBLE_DEVICES=$GPU \
        CARGO_TARGET_DIR=$TARGET_DIR \
        FE2O3_ROOT_TARGET_DIR=$TARGET_DIR \
        FE2O3_RUSTC_EXTRACTOR=$EXTRACTOR \
        FE2O3_GFX950_ADVANCED_OUTPUT_DIR=$EVIDENCE_DIR/artifacts/$variant \
        FE2O3_GFX950_PRUNE_AMDGPU_TARGET=1 \
        FE2O3_GFX950_ADVANCED_PERF_OUTPUT=$EVIDENCE_DIR/samples.jsonl \
        FE2O3_GFX950_ADVANCED_PERF_CAMPAIGN_ID=attention-audit-gpu$GPU \
        FE2O3_GFX950_ADVANCED_PERF_IMPLEMENTATION_ID=fe2o3-rust \
        FE2O3_GFX950_ADVANCED_PERF_VARIANT_ID=$variant \
        FE2O3_GFX950_ADVANCED_PERF_PROCESS=0 \
        FE2O3_GFX950_ADVANCED_PERF_WARMUPS=${FE2O3_GFX950_ADVANCED_PERF_WARMUPS:-1000} \
        FE2O3_GFX950_ADVANCED_PERF_BLOCKS=${FE2O3_GFX950_ADVANCED_PERF_BLOCKS:-30} \
        FE2O3_GFX950_ADVANCED_PERF_SAMPLES_PER_BLOCK=${FE2O3_GFX950_ADVANCED_PERF_SAMPLES_PER_BLOCK:-100} \
        FE2O3_GFX950_ADVANCED_PERF_BLOCK_REWARM=${FE2O3_GFX950_ADVANCED_PERF_BLOCK_REWARM:-20} \
        "$SCRIPT_DIR/run-gfx950.sh" "$feature" > "$EVIDENCE_DIR/logs/$variant.log" 2>&1
}

run_case kernel-kda-decode-baseline-v1 kda-sequential
run_case kernel-kda-decode kda-wave16
run_case kernel-kda-prefill-baseline-v1 prefill-sequential
run_case kernel-kda-prefill prefill-wy
run_case kernel-content-sparse-attention-reciprocal-reuse-v1 content-reciprocal
run_case kernel-content-sparse-attention content-division
run_case kernel-deepseek-sparse-attention lane-parallel-exp
run_case kernel-deepseek-sparse-attention-leader-exp-v1 leader-exp-broadcast
run_case kernel-compressed-hybrid-attention-division-baseline-v1 hybrid-division
run_case kernel-compressed-hybrid-attention hybrid-reciprocal
run_case kernel-attnres-aggregate-explicit-reuse-v1 attnres-explicit
run_case kernel-attnres-aggregate attnres-loop
run_case kernel-four-branch-residual-explicit-v1 residual-explicit
run_case kernel-four-branch-residual residual-loop
run_case kernel-mhc-sinkhorn-mix-scalar-v1 mhc-scalar
run_case kernel-mhc-sinkhorn-mix mhc-wave16

python3 "$REPO_ROOT/perf-evidence/analyze.py" "$EVIDENCE_DIR/samples.jsonl" \
    --compare kda-sequential:kda-wave16 \
    --compare prefill-sequential:prefill-wy \
    --compare content-reciprocal:content-division \
    --compare lane-parallel-exp:leader-exp-broadcast \
    --compare hybrid-division:hybrid-reciprocal \
    --compare attnres-explicit:attnres-loop \
    --compare residual-explicit:residual-loop \
    --compare mhc-scalar:mhc-wave16 > "$EVIDENCE_DIR/summary.json"

printf 'PASS attention performance audit: %s\n' "$EVIDENCE_DIR"
