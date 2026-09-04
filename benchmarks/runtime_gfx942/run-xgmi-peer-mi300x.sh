#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
readonly gpu_index="${FE2O3_XGMI_GPU_INDEX:-0}"
readonly peer_gpu_index="${FE2O3_XGMI_PEER_GPU_INDEX:-1}"
readonly bytes="${FE2O3_XGMI_BYTES:-1048576}"
readonly depths="${FE2O3_XGMI_DEPTHS:-1 16}"
readonly warmups="${FE2O3_XGMI_WARMUPS:-10}"
readonly samples="${FE2O3_XGMI_SAMPLES:-30}"
readonly max_busy="${FE2O3_XGMI_MAX_BUSY_PERCENT:-5}"
readonly phase_timeout="${FE2O3_XGMI_PHASE_TIMEOUT_SECONDS:-120}"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-xgmi-peer-gfx942.XXXXXX")"
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
    printf 'could not observe GPU %s busy percentage\n' "${index}" >&2
    exit 2
  }
  ((observed <= max_busy)) || {
    printf 'GPU %s is %s%% busy; release limit is %s%%\n' \
      "${index}" "${observed}" "${max_busy}" >&2
    exit 2
  }
  printf '%s' "${observed}"
}

unique_id_for_gpu() {
  local index="$1"
  "${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
    awk -v gpu="GPU[${index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

run_phase() {
  local backend="$1"
  local depth="$2"
  shift 2
  local busy_start peer_busy_start busy_end peer_busy_end result
  busy_start="$(require_idle_gpu "${gpu_index}")"
  peer_busy_start="$(require_idle_gpu "${peer_gpu_index}")"
  printf 'context phase=%s depth=%s gpu_busy_start_percent=%s,%s\n' \
    "${backend}" "${depth}" "${busy_start}" "${peer_busy_start}"
  result="$(timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" "$@")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  peer_busy_end="$(require_idle_gpu "${peer_gpu_index}")"
  printf 'context phase=%s depth=%s gpu_busy_end_percent=%s,%s\n%s\n' \
    "${backend}" "${depth}" "${busy_end}" "${peer_busy_end}" "${result}"
}

[[ -c /dev/kfd ]] || { printf 'missing /dev/kfd\n' >&2; exit 2; }
[[ -x "${rocm_path}/bin/hipcc" ]] || { printf 'missing hipcc\n' >&2; exit 2; }
[[ -x "${rocm_path}/bin/rocm-smi" ]] || { printf 'missing rocm-smi\n' >&2; exit 2; }
command -v cargo >/dev/null || { printf 'missing cargo\n' >&2; exit 2; }
command -v g++ >/dev/null || { printf 'missing g++\n' >&2; exit 2; }
command -v timeout >/dev/null || { printf 'missing timeout\n' >&2; exit 2; }

rustc_release="$(rustc --version | awk '{ print $2 }')"
IFS=. read -r rustc_major rustc_minor _ <<<"${rustc_release}"
[[ "${rustc_major}" =~ ^[0-9]+$ && "${rustc_minor}" =~ ^[0-9]+$ ]] || {
  printf 'could not parse rustc version: %s\n' "${rustc_release}" >&2
  exit 2
}
((rustc_major > 1 || (rustc_major == 1 && rustc_minor >= 94))) || {
  printf 'XGMI qualification requires rustc 1.94 or newer, observed %s\n' \
    "${rustc_release}" >&2
  exit 2
}
for value in "${gpu_index}" "${peer_gpu_index}" "${bytes}" "${warmups}" \
  "${samples}" "${max_busy}" "${phase_timeout}"; do
  [[ "${value}" =~ ^[0-9]+$ ]] || {
    printf 'benchmark controls must be nonnegative integers\n' >&2
    exit 2
  }
done
((gpu_index != peer_gpu_index)) || {
  printf 'the two GPU indices must be distinct\n' >&2
  exit 2
}
((bytes >= 1 && bytes <= 4194272 && warmups >= 1 && samples >= 1 && max_busy <= 100 && phase_timeout >= 1 && phase_timeout <= 3600)) || {
  printf 'copy size or statistical controls are out of range\n' >&2
  exit 2
}
for depth in ${depths}; do
  if ! [[ "${depth}" =~ ^[1-9][0-9]*$ ]] || ((depth > 32)); then
    printf 'every XGMI depth must be in 1 through 32\n' >&2
    exit 2
  fi
done
[[ -z "$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ]] || {
  printf 'XGMI qualification requires a clean checkout\n' >&2
  exit 2
}

unique_id="$(unique_id_for_gpu "${gpu_index}")"
peer_unique_id="$(unique_id_for_gpu "${peer_gpu_index}")"
[[ "${unique_id}" =~ ^0x[0-9a-fA-F]+$ && "${peer_unique_id}" =~ ^0x[0-9a-fA-F]+$ ]] || {
  printf 'could not resolve both selected GPU unique IDs\n' >&2
  exit 2
}
[[ "${unique_id}" != "${peer_unique_id}" ]] || {
  printf 'the selected GPUs reported the same unique ID\n' >&2
  exit 2
}

export CARGO_TARGET_DIR="${build_dir}/target"
cd "${repo_root}"
cargo build --locked --release -p fe2o3-runtime \
  --example gfx942-runtime-xgmi-peer-benchmark
"${rocm_path}/bin/hipcc" -std=c++17 -O3 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/xgmi_peer_hip.cpp -o "${build_dir}/xgmi-peer-hip"
g++ -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${rocm_path}/include" benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/xgmi-peer-hsa"

rocm_version=unknown
[[ ! -r "${rocm_path}/.info/version" ]] || IFS= read -r rocm_version < "${rocm_path}/.info/version"
printf 'context schema=fe2o3.xgmi-peer-benchmark.v1 git_commit=%s target=gfx942:xnack- gpu_indices=%s,%s unique_ids=%s,%s bytes=%s depths=%s warmups=%s samples=%s max_busy_percent=%s phase_timeout_seconds=%s rocm_version=%s rustc=%s kfd_surface=runtime-facade timing=submit-through-observed-completion setup_validation=outside-timing measurement=persistent-hot mapping_lifetime=persistent-no-host-access-between-timed-rounds\n' \
  "$(git rev-parse HEAD)" "${gpu_index}" "${peer_gpu_index}" \
  "${unique_id}" "${peer_unique_id}" "${bytes}" "${depths// /,}" \
  "${warmups}" "${samples}" "${max_busy}" "${phase_timeout}" \
  "${rocm_version}" "$(rustc --version | tr ' ' '_')"

for depth in ${depths}; do
  run_phase kfd "${depth}" \
    "${build_dir}/target/release/examples/gfx942-runtime-xgmi-peer-benchmark" \
    "${unique_id}" "${peer_unique_id}" "${bytes}" "${depth}" "${warmups}" "${samples}"
  run_phase hsa "${depth}" env HSA_XNACK=0 \
    ROCR_VISIBLE_DEVICES="${gpu_index},${peer_gpu_index}" \
    "${build_dir}/xgmi-peer-hsa" 0 1 "${bytes}" "${depth}" "${warmups}" \
    "${samples}" "${unique_id}" "${peer_unique_id}"
  run_phase hip "${depth}" env HSA_XNACK=0 \
    HIP_VISIBLE_DEVICES="${gpu_index},${peer_gpu_index}" \
    "${build_dir}/xgmi-peer-hip" 0 1 "${bytes}" "${depth}" "${warmups}" \
    "${samples}" "${unique_id}" "${peer_unique_id}"
done

busy_after="$(require_idle_gpu "${gpu_index}")"
peer_busy_after="$(require_idle_gpu "${peer_gpu_index}")"
printf 'context gpu_busy_after_percent=%s,%s\n' "${busy_after}" "${peer_busy_after}"
