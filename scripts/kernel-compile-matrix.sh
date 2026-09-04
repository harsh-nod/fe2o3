#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
if (($# > 1)); then
  printf '%s\n' 'kernel compile matrix: expected at most one target argument' >&2
  exit 2
fi
readonly TARGET="${1:-gfx942}"
readonly -a GFX942_CASES=(
  "tiled-gemm|examples/tiled_gemm_general_v1/run-gfx942.sh|tiled_gemm_general_v1.hsaco"
  "row-softmax|examples/row_softmax_general_v1/run-gfx942.sh|row_softmax_general_v1.hsaco"
  "flash-attention|examples/flash_attention_general_v1/run-gfx942.sh|flash_attention_general_v1.hsaco"
  "grouped-expert-moe|examples/moe_grouped_expert_general_v1/run-gfx942.sh|moe_grouped_expert_general_v1.hsaco"
  "gemm-autoresearch|examples/gemm_autoresearch_v1/run-gfx942.sh|gemm_autoresearch_v1.hsaco"
)
readonly -a GFX950_CASES=(
  "fp4-gemm|examples/gfx950_low_precision/run-fp4-gemm-gfx950.sh|gfx950-fp4-gemm.hsaco"
  "fp8-gemm|examples/gfx950_low_precision/run-fp8-gemm-gfx950.sh|gfx950-fp8-gemm.hsaco"
  "fp4-attention|examples/gfx950_low_precision/run-fp4-attention-gfx950.sh|gfx950-fp4-attention.hsaco"
  "fp8-attention|examples/gfx950_low_precision/run-fp8-attention-gfx950.sh|gfx950-fp8-attention.hsaco"
  "kda-decode|examples/gfx950_advanced_attention/run-kda-decode-gfx950.sh|kernel-kda-decode.hsaco"
  "kda-prefill|examples/gfx950_advanced_attention/run-kda-chunkwise-prefill-gfx950.sh|kernel-kda-prefill.hsaco"
  "content-sparse-attention|examples/gfx950_advanced_attention/run-content-sparse-attention-gfx950.sh|kernel-content-sparse-attention.hsaco"
  "deepseek-sparse-attention|examples/gfx950_advanced_attention/run-deepseek-sparse-attention-gfx950.sh|kernel-deepseek-sparse-attention.hsaco"
  "compressed-hybrid-attention|examples/gfx950_advanced_attention/run-compressed-hybrid-attention-gfx950.sh|kernel-compressed-hybrid-attention.hsaco"
  "attnres-aggregate|examples/gfx950_advanced_attention/run-attnres-aggregate-gfx950.sh|kernel-attnres-aggregate.hsaco"
  "four-branch-residual|examples/gfx950_advanced_attention/run-four-branch-residual-gfx950.sh|kernel-four-branch-residual.hsaco"
  "mhc-sinkhorn-mix|examples/gfx950_advanced_attention/run-mhc-sinkhorn-mix-gfx950.sh|kernel-mhc-sinkhorn-mix.hsaco"
  "moe-route|examples/gfx950_advanced_systems/run-moe-route-gfx950.sh|kernel-moe-route.hsaco"
  "moe-expert-rank|examples/gfx950_advanced_systems/run-moe-expert-rank-gfx950.sh|kernel-moe-expert-rank.hsaco"
  "combine-expert-ranks|examples/gfx950_advanced_systems/run-combine-expert-ranks-gfx950.sh|kernel-combine-expert-ranks.hsaco"
  "speculative-transaction|examples/gfx950_advanced_systems/run-speculative-transaction-gfx950.sh|kernel-speculative-transaction.hsaco"
  "qwen-ngram-gather|examples/gfx950_advanced_systems/run-qwen-ngram-gather-gfx950.sh|kernel-qwen-ngram-gather.hsaco"
  "stage-gradient-shard|examples/gfx950_advanced_systems/run-stage-gradient-shard-gfx950.sh|kernel-stage-gradient-shard.hsaco"
  "muon-update|examples/gfx950_advanced_systems/run-muon-update-gfx950.sh|kernel-muon-update.hsaco"
  "gpt-oss-decode|examples/gfx950_gpt_oss_decode/run-gfx950.sh|kernel-gpt-oss-decode.hsaco"
)
declare -a CASES=()

usage() {
  cat <<'EOF'
Usage: scripts/kernel-compile-matrix.sh [gfx942|gfx950]

Compile a fixed fe2o3-kernels production-source matrix through the
fe2o3 extractor and ROCm finalizer without executing any kernel.

The gfx950 matrix requires the exact manifest-pinned ROCm 7.2.1 Clang, LLD,
and device-library closure. Neither target covers proof-only examples, the
basic Cargo regression manifest, hardware behavior, or numerical results.
EOF
}

case "${TARGET}" in
  -h | --help | help)
    usage
    exit 0
    ;;
  gfx942)
    CASES=("${GFX942_CASES[@]}")
    ;;
  gfx950)
    CASES=("${GFX950_CASES[@]}")
    ;;
  *)
    printf 'kernel compile matrix: unsupported target: %s\n' "${TARGET}" >&2
    usage >&2
    exit 2
    ;;
esac

printf 'kernel compile matrix: target=%s mode=compile-only kernels=%d hardware_observed=false\n' \
  "${TARGET}" "${#CASES[@]}"
if [[ ${TARGET} == gfx950 ]]; then
  printf '%s\n' \
    'MATRIX PREREQUISITE target=gfx950 exact manifest-pinned ROCm 7.2.1 Clang/LLD/device-library closure required'
fi

MATRIX_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-kernel-compile-matrix.XXXXXXXX")"
readonly MATRIX_ROOT
cleanup() {
  rm -rf -- "${MATRIX_ROOT:?}"
}
trap cleanup EXIT

readonly BUILD_TARGET="${MATRIX_ROOT}/cargo-target"
readonly CASE_ROOT="${MATRIX_ROOT}/cases"
mkdir -p -- "${BUILD_TARGET}" "${CASE_ROOT}"

readonly TOOLCHAIN="${FE2O3_RUST_TOOLCHAIN:-nightly-2026-04-03}"
readonly EXTRACTOR="${BUILD_TARGET}/debug/fe2o3-rustc-extract"
CARGO_TARGET_DIR="${BUILD_TARGET}" rustup run "${TOOLCHAIN}" cargo build \
  --locked --manifest-path "${REPO_ROOT}/Cargo.toml" \
  -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
if [[ ! -f "${EXTRACTOR}" || -L "${EXTRACTOR}" || ! -x "${EXTRACTOR}" ]]; then
  printf 'kernel compile matrix: extractor build did not produce a regular executable: %s\n' \
    "${EXTRACTOR}" >&2
  exit 1
fi

passed=0
for record in "${CASES[@]}"; do
  IFS='|' read -r name runner artifact <<<"${record}"
  case_dir="${CASE_ROOT}/${name}"
  output_dir="${case_dir}/artifacts"
  temporary_dir="${case_dir}/tmp"
  runner_path="${REPO_ROOT}/${runner}"
  mkdir -p -- "${output_dir}" "${temporary_dir}"
  if [[ ! -f "${runner_path}" || -L "${runner_path}" ]]; then
    printf 'kernel compile matrix: missing regular runner for %s: %s\n' \
      "${name}" "${runner_path}" >&2
    exit 1
  fi

  printf 'CASE %s target=%s status=RUNNING\n' "${name}" "${TARGET}"
  env \
    CARGO_TARGET_DIR="${BUILD_TARGET}" \
    FE2O3_EXAMPLE_COMPILE_ONLY=1 \
    FE2O3_ROOT_TARGET_DIR="${BUILD_TARGET}" \
    FE2O3_RUSTC_EXTRACTOR="${EXTRACTOR}" \
    FE2O3_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_FP4_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_FP8_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_FP4_ATTENTION_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_FP8_ATTENTION_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_ADVANCED_OUTPUT_DIR="${output_dir}" \
    FE2O3_GFX950_PRUNE_AMDGPU_TARGET=1 \
    TMPDIR="${temporary_dir}" \
    bash "${runner_path}"

  artifact_paths=()
  mapfile -d '' -t artifact_paths < <(
    find "${output_dir}" -type f -name "${artifact}" -print0
  )
  if ((${#artifact_paths[@]} != 1)); then
    printf 'kernel compile matrix: %s produced %d valid candidates for expected HSACO %s\n' \
      "${name}" "${#artifact_paths[@]}" "${artifact}" >&2
    exit 1
  fi
  artifact_path=${artifact_paths[0]}
  if [[ -L ${artifact_path} || ! -s ${artifact_path} ]]; then
    printf 'kernel compile matrix: %s produced an invalid expected HSACO: %s\n' \
      "${name}" "${artifact_path}" >&2
    exit 1
  fi
  passed=$((passed + 1))
  printf 'CASE %s target=%s status=PASS hardware_observed=false\n' \
    "${name}" "${TARGET}"
done

printf 'MATRIX PASS target=%s compiled=%d hardware_executed=0 artifacts=temporary\n' \
  "${TARGET}" "${passed}"
if [[ ${TARGET} == gfx942 ]]; then
  printf '%s\n' \
    'MATRIX LIMITATION gfx950 requires its separate exact ROCm 7.2.1 matrix; source-model-only, proof-only, and basic Cargo examples are not covered'
else
  printf '%s\n' \
    'MATRIX LIMITATION gfx950 ablations, HIP comparators, hardware behavior, and numerical results are not covered'
fi
