#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
readonly gpu_index="${FE2O3_DIRECTIONAL_WINDOW_GPU_INDEX:-0}"
readonly second_gpu_index="${FE2O3_DIRECTIONAL_WINDOW_SECOND_GPU_INDEX:-1}"
readonly bytes="${FE2O3_DIRECTIONAL_WINDOW_BYTES:-268435456}"
readonly warmups="${FE2O3_DIRECTIONAL_WINDOW_WARMUPS:-3}"
readonly samples="${FE2O3_DIRECTIONAL_WINDOW_SAMPLES:-10}"
readonly max_busy="${FE2O3_DIRECTIONAL_WINDOW_MAX_BUSY_PERCENT:-5}"
readonly phase_timeout="${FE2O3_DIRECTIONAL_WINDOW_PHASE_TIMEOUT_SECONDS:-180}"
readonly min_r22_bytes=264239137
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-directional-window-gfx942.XXXXXX")"
readonly build_dir

cleanup() {
  [[ ! -e "${build_dir}" ]] || find "${build_dir}" -depth -delete
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

gpu_busy_percent() {
  local index="$1"
  "${rocm_path}/bin/rocm-smi" --showuse 2>/dev/null |
    awk -v gpu="GPU[${index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

require_idle_gpu() {
  local index="$1"
  local observed
  observed="$(gpu_busy_percent "${index}")"
  [[ "${observed}" =~ ^[0-9]+$ ]] || {
    printf 'could not observe GPU busy percentage\n' >&2
    exit 2
  }
  ((observed <= max_busy)) || {
    printf 'GPU %s is %s%% busy; release limit is %s%%\n' \
      "${index}" "${observed}" "${max_busy}" >&2
    exit 2
  }
  printf '%s' "${observed}"
}

for tool in "${rocm_path}/bin/hipcc" "${rocm_path}/bin/rocm-smi"; do
  [[ -x "${tool}" ]] || { printf 'missing executable: %s\n' "${tool}" >&2; exit 2; }
done
[[ -c /dev/kfd ]] || { printf 'missing /dev/kfd\n' >&2; exit 2; }
for tool in cargo g++ timeout; do
  command -v "${tool}" >/dev/null || { printf 'missing command: %s\n' "${tool}" >&2; exit 2; }
done
for value in "${gpu_index}" "${second_gpu_index}" "${bytes}" "${warmups}" "${samples}" "${max_busy}" "${phase_timeout}"; do
  [[ "${value}" =~ ^[0-9]+$ ]] || { printf 'benchmark controls must be nonnegative integers\n' >&2; exit 2; }
done
((gpu_index != second_gpu_index)) || { printf 'GPU indices must be distinct\n' >&2; exit 2; }
((bytes >= min_r22_bytes && bytes <= 268435456 && warmups >= 1 && samples >= 1)) || {
  printf 'R22 benchmark requires 264239137..268435456 bytes and nonzero statistics\n' >&2
  exit 2
}
((max_busy <= 100 && phase_timeout >= 1 && phase_timeout <= 3600)) || {
  printf 'load or timeout control is out of range\n' >&2
  exit 2
}
[[ -z "$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ]] || {
  printf 'directional-window qualification requires a clean checkout\n' >&2
  exit 2
}

resolve_unique_id() {
  local index="$1"
  "${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
    awk -v gpu="GPU[${index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

unique_id="$(resolve_unique_id "${gpu_index}")"
second_unique_id="$(resolve_unique_id "${second_gpu_index}")"
[[ "${unique_id}" =~ ^0x[0-9a-fA-F]+$ && "${second_unique_id}" =~ ^0x[0-9a-fA-F]+$ ]] || {
  printf 'could not resolve both GPU unique IDs\n' >&2
  exit 2
}

export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="${build_dir}/target"
cd "${repo_root}"
cargo build --locked --release -p fe2o3-runtime \
  --example gfx942-runtime-directional-window-benchmark
cargo test --locked -p fe2o3-kfd --lib \
  persistent_directional_sdma::tests::window_manifest_digest_is_frozen -- --exact
"${rocm_path}/bin/hipcc" -std=c++17 -O3 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/async_copy_hip.cpp -o "${build_dir}/async-copy-hip"
g++ -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${rocm_path}/include" benchmarks/runtime_gfx942/async_copy_hsa.cpp \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/async-copy-hsa"

rocm_version=unknown
[[ ! -r "${rocm_path}/.info/version" ]] || IFS= read -r rocm_version < "${rocm_path}/.info/version"
printf 'context schema=fe2o3.async-copy-benchmark.v1 git_commit=%s target=gfx942:xnack- gpu_indices=%s,%s unique_ids=%s,%s bytes=%s depths=1 warmups=%s samples=%s kfd_profile=directional kfd_multi_profile=directional max_busy_percent=%s phase_timeout_seconds=%s rocm_version=%s rustc=%s sdma_manifest_sha256=bea5fe674dc25ebb82532770c1bf53b2e3b68ea99940470dee6362e812b579d3 directional_window_manifest_sha256=44821351a14664f9be3db9fc406ee9f4961d4f40a4346fdb085886ecfc84c2aa measurement=runtime-facade-r22-directional-window\n' \
  "$(git rev-parse HEAD)" "${gpu_index}" "${second_gpu_index}" \
  "${unique_id}" "${second_unique_id}" "${bytes}" "${warmups}" "${samples}" \
  "${max_busy}" "${phase_timeout}" "${rocm_version}" "$(rustc --version | tr ' ' '_')"

busy_start="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=kfd depth=1 gpu_busy_start_percent=%s\n' "${busy_start}"
result="$(timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
  "${build_dir}/target/release/examples/gfx942-runtime-directional-window-benchmark" \
  "${unique_id}" "${bytes}" "${warmups}" "${samples}")"
busy_end="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=kfd depth=1 gpu_busy_end_percent=%s\n%s\n' "${busy_end}" "${result}"

busy_start="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=hsa depth=1 gpu_busy_start_percent=%s\n' "${busy_start}"
result="$(HSA_XNACK=0 ROCR_VISIBLE_DEVICES="${gpu_index}" \
  timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
  "${build_dir}/async-copy-hsa" 0 "${bytes}" 1 "${warmups}" "${samples}" "${unique_id}")"
busy_end="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=hsa depth=1 gpu_busy_end_percent=%s\n%s\n' "${busy_end}" "${result}"

busy_start="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=hip depth=1 gpu_busy_start_percent=%s\n' "${busy_start}"
result="$(HSA_XNACK=0 HIP_VISIBLE_DEVICES="${gpu_index}" \
  timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
  "${build_dir}/async-copy-hip" 0 "${bytes}" 1 "${warmups}" "${samples}" "${unique_id}")"
busy_end="$(require_idle_gpu "${gpu_index}")"
printf 'context phase=hip depth=1 gpu_busy_end_percent=%s\n%s\n' "${busy_end}" "${result}"

printf 'context gpu_busy_after_percent=%s,%s\n' \
  "$(require_idle_gpu "${gpu_index}")" "$(require_idle_gpu "${second_gpu_index}")"
