#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly fixture="${repo_root}/crates/fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco"
readonly fixture_sha256='3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab'
readonly element_count=1048576
readonly warmups="${FE2O3_RUNTIME_WARMUPS:-10}"
readonly samples="${FE2O3_RUNTIME_SAMPLES:-30}"
readonly launches="${FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE:-10}"
readonly gpu_index="${FE2O3_RUNTIME_GPU_INDEX:-0}"
readonly max_busy="${FE2O3_RUNTIME_MAX_BUSY_PERCENT:-5}"
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-gfx942.XXXXXX")"
readonly build_dir
cleanup() {
  [[ ! -e "${build_dir}" ]] || find "${build_dir}" -depth -delete
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

gpu_busy_percent() {
  "${rocm_path}/bin/rocm-smi" --showuse 2>/dev/null |
    awk -v gpu="GPU[${gpu_index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

require_idle_gpu() {
  local observed
  observed="$(gpu_busy_percent)"
  [[ "${observed}" =~ ^[0-9]+$ ]] || {
    printf 'could not observe GPU busy percentage\n' >&2
    exit 2
  }
  ((observed <= max_busy)) || {
    printf 'GPU %s is %s%% busy; release limit is %s%%\n' \
      "${gpu_index}" "${observed}" "${max_busy}" >&2
    exit 2
  }
  printf '%s' "${observed}"
}

[[ -c /dev/kfd ]] || {
  printf 'missing /dev/kfd\n' >&2
  exit 2
}
[[ -x "${rocm_path}/bin/hipcc" ]] || {
  printf 'missing hipcc under %s\n' "${rocm_path}" >&2
  exit 2
}
[[ -x "${rocm_path}/bin/rocm-smi" ]] || {
  printf 'missing rocm-smi under %s\n' "${rocm_path}" >&2
  exit 2
}
command -v cargo >/dev/null || {
  printf 'missing cargo\n' >&2
  exit 2
}
command -v rustc >/dev/null || {
  printf 'missing rustc\n' >&2
  exit 2
}
[[ "${gpu_index}" =~ ^[0-9]+$ ]] || {
  printf 'FE2O3_RUNTIME_GPU_INDEX must be a nonnegative integer\n' >&2
  exit 2
}
for value in "${warmups}" "${samples}" "${launches}"; do
  [[ "${value}" =~ ^[1-9][0-9]*$ ]] || {
    printf 'warmups, samples, and launches per sample must be positive integers\n' >&2
    exit 2
  }
done
if ! [[ "${max_busy}" =~ ^[0-9]+$ ]] || ((max_busy > 100)); then
  printf 'FE2O3_RUNTIME_MAX_BUSY_PERCENT must be an integer from 0 through 100\n' >&2
  exit 2
fi

rustc_release="$(rustc --version | awk '{ print $2 }')"
IFS=. read -r rustc_major rustc_minor _ <<<"${rustc_release}"
[[ "${rustc_major}" =~ ^[0-9]+$ && "${rustc_minor}" =~ ^[0-9]+$ ]] || {
  printf 'could not parse rustc version: %s\n' "${rustc_release}" >&2
  exit 2
}
if ((rustc_major < 1 || (rustc_major == 1 && rustc_minor < 94))); then
  printf 'runtime qualification requires rustc 1.94 or newer, observed %s\n' \
    "${rustc_release}" >&2
  exit 2
fi

[[ -z "$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ]] || {
  printf 'runtime qualification requires a clean checkout\n' >&2
  exit 2
}

printf '%s  %s\n' "${fixture_sha256}" "${fixture}" | sha256sum --check --status
"${repo_root}/crates/fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/build-and-verify.sh"

unique_id="$("${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
  awk -v gpu="GPU[${gpu_index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }')"
[[ "${unique_id}" =~ ^0x[0-9a-fA-F]+$ ]] || {
  printf 'could not resolve a unique ID for HIP GPU index %s\n' "${gpu_index}" >&2
  exit 2
}
rocm_version="unknown"
if [[ -r "${rocm_path}/.info/version" ]]; then
  IFS= read -r rocm_version < "${rocm_path}/.info/version"
fi
gpu_busy_before="$(require_idle_gpu)"
rustc_version="$(rustc --version | tr ' ' '_')"
cargo_version="$(cargo --version | tr ' ' '_')"

export CARGO_TARGET_DIR="${build_dir}/target"
cd "${repo_root}"
cargo build --locked --release -p fe2o3-runtime \
  --features hardware-qualification \
  --example gfx942-runtime-vecadd-benchmark
cargo test --locked --release -p fe2o3-hsa-runtime \
  --features hardware-qualification \
  --test gfx942_runtime_context_hardware --no-run
"${rocm_path}/bin/hipcc" -std=c++17 -O3 -Wall -Wextra -Werror \
  --offload-arch=gfx942 \
  "${repo_root}/benchmarks/runtime_gfx942/vecadd_module_hip.cpp" \
  -o "${build_dir}/vecadd-module-hip"

printf 'context schema=fe2o3.runtime-gfx942-benchmark.v1 git_commit=%s target=gfx942:xnack- gpu_index=%s unique_id=%s fixture_sha256=%s rocm_version=%s rustc=%s cargo=%s gpu_busy_before_percent=%s max_busy_percent=%s n=%s warmups=%s samples=%s launches_per_sample=%s\n' \
  "$(git rev-parse HEAD)" "${gpu_index}" "${unique_id}" "${fixture_sha256}" \
  "${rocm_version}" "${rustc_version}" "${cargo_version}" "${gpu_busy_before}" "${max_busy}" \
  "${element_count}" "${warmups}" "${samples}" "${launches}"

phase_busy="$(require_idle_gpu)"
printf 'context phase=kfd gpu_busy_start_percent=%s\n' "${phase_busy}"
"${build_dir}/target/release/examples/gfx942-runtime-vecadd-benchmark" \
  "${unique_id}" "${warmups}" "${samples}" "${launches}"
phase_busy="$(require_idle_gpu)"
printf 'context phase=kfd gpu_busy_end_percent=%s\n' "${phase_busy}"
phase_busy="$(require_idle_gpu)"
printf 'context phase=hsa gpu_busy_start_percent=%s\n' "${phase_busy}"
HIP_VISIBLE_DEVICES=0 \
ROCR_VISIBLE_DEVICES="${gpu_index}" \
FE2O3_RUN_GFX942_RUNTIME_HSA_QUALIFICATION=1 \
FE2O3_RUNTIME_WARMUPS="${warmups}" \
FE2O3_RUNTIME_SAMPLES="${samples}" \
FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE="${launches}" \
  cargo test --locked --release -p fe2o3-hsa-runtime \
    --features hardware-qualification \
    --test gfx942_runtime_context_hardware \
    qualification::gfx942_runtime_context_exact_fixture_executes_dependencies_wraps_and_times \
    -- --ignored --exact --nocapture --test-threads=1
phase_busy="$(require_idle_gpu)"
printf 'context phase=hsa gpu_busy_end_percent=%s\n' "${phase_busy}"
phase_busy="$(require_idle_gpu)"
printf 'context phase=hip gpu_busy_start_percent=%s\n' "${phase_busy}"
HIP_VISIBLE_DEVICES="${gpu_index}" "${build_dir}/vecadd-module-hip" \
  "${fixture}" "${element_count}" "${warmups}" "${samples}" "${launches}"
phase_busy="$(require_idle_gpu)"
printf 'context phase=hip gpu_busy_end_percent=%s\n' "${phase_busy}"
gpu_busy_after="$(require_idle_gpu)"
printf 'context gpu_busy_after_percent=%s\n' "${gpu_busy_after}"
