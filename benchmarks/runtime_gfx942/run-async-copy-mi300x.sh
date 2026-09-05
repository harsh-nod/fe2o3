#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
readonly gpu_index="${FE2O3_ASYNC_COPY_GPU_INDEX:-0}"
readonly second_gpu_index="${FE2O3_ASYNC_COPY_SECOND_GPU_INDEX:-1}"
readonly bytes="${FE2O3_ASYNC_COPY_BYTES:-1048576}"
readonly depths="${FE2O3_ASYNC_COPY_DEPTHS:-1 16}"
readonly warmups="${FE2O3_ASYNC_COPY_WARMUPS:-10}"
readonly samples="${FE2O3_ASYNC_COPY_SAMPLES:-30}"
readonly kfd_profile="${FE2O3_ASYNC_COPY_KFD_PROFILE:-directional}"
readonly max_busy="${FE2O3_ASYNC_COPY_MAX_BUSY_PERCENT:-5}"
readonly phase_timeout="${FE2O3_ASYNC_COPY_PHASE_TIMEOUT_SECONDS:-120}"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-async-copy-gfx942.XXXXXX")"
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
  printf 'async-copy qualification requires rustc 1.94 or newer, observed %s\n' \
    "${rustc_release}" >&2
  exit 2
}
for value in "${gpu_index}" "${second_gpu_index}" "${bytes}" "${warmups}" "${samples}" "${max_busy}" "${phase_timeout}"; do
  [[ "${value}" =~ ^[0-9]+$ ]] || {
    printf 'benchmark controls must be nonnegative integers\n' >&2
    exit 2
  }
done
((gpu_index != second_gpu_index)) || {
  printf 'the two GPU indices must be distinct\n' >&2
  exit 2
}
((bytes >= 1 && bytes <= 4194272 && warmups >= 1 && samples >= 1 && max_busy <= 100 && phase_timeout >= 1 && phase_timeout <= 3600)) || {
  printf 'copy size or statistical controls are out of range\n' >&2
  exit 2
}
for depth in ${depths}; do
  if ! [[ "${depth}" =~ ^[1-9][0-9]*$ ]] || ((depth > 63)); then
    printf 'every depth must be in 1 through 63\n' >&2
    exit 2
  fi
done
case "${kfd_profile}" in
  generic|directional|engine0|engine1) ;;
  striped2|striped4|striped6|striped8|striped10|striped12|striped14|striped16) ;;
  *)
    printf 'KFD profile must be generic, directional, engine0, engine1, or an admitted even striped2..striped16 profile\n' >&2
    exit 2
    ;;
esac
[[ -z "$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ]] || {
  printf 'async-copy qualification requires a clean checkout\n' >&2
  exit 2
}

unique_id="$("${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
  awk -v gpu="GPU[${gpu_index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }')"
[[ "${unique_id}" =~ ^0x[0-9a-fA-F]+$ ]] || {
  printf 'could not resolve the selected GPU unique ID\n' >&2
  exit 2
}
second_unique_id="$("${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
  awk -v gpu="GPU[${second_gpu_index}]" '$1 == gpu { value = $NF } END { if (value != "") print value }')"
[[ "${second_unique_id}" =~ ^0x[0-9a-fA-F]+$ ]] || {
  printf 'could not resolve the second GPU unique ID\n' >&2
  exit 2
}

export CARGO_TARGET_DIR="${build_dir}/target"
cd "${repo_root}"
cargo build --locked --release -p fe2o3-kfd --features live-validation \
  --example kfd-sdma-copy-benchmark --example kfd-sdma-multi-device-benchmark
cargo test --locked -p fe2o3-kfd --lib \
  sdma::tests::sdma_copy_manifest_digest_is_frozen -- --exact
"${rocm_path}/bin/hipcc" -std=c++17 -O3 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/async_copy_hip.cpp -o "${build_dir}/async-copy-hip"
"${rocm_path}/bin/hipcc" -std=c++17 -O3 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/async_copy_multi_hip.cpp \
  -o "${build_dir}/async-copy-multi-hip"
g++ -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${rocm_path}/include" benchmarks/runtime_gfx942/async_copy_hsa.cpp \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/async-copy-hsa"
g++ -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${rocm_path}/include" benchmarks/runtime_gfx942/async_copy_multi_hsa.cpp \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/async-copy-multi-hsa"

rocm_version=unknown
[[ ! -r "${rocm_path}/.info/version" ]] || IFS= read -r rocm_version < "${rocm_path}/.info/version"
printf 'context schema=fe2o3.async-copy-benchmark.v1 git_commit=%s target=gfx942:xnack- gpu_indices=%s,%s unique_ids=%s,%s bytes=%s depths=%s warmups=%s samples=%s kfd_profile=%s kfd_multi_profile=directional max_busy_percent=%s phase_timeout_seconds=%s rocm_version=%s rustc=%s sdma_manifest_sha256=bea5fe674dc25ebb82532770c1bf53b2e3b68ea99940470dee6362e812b579d3\n' \
  "$(git rev-parse HEAD)" "${gpu_index}" "${second_gpu_index}" \
  "${unique_id}" "${second_unique_id}" "${bytes}" \
  "${depths// /,}" "${warmups}" "${samples}" "${kfd_profile}" "${max_busy}" "${phase_timeout}" \
  "${rocm_version}" "$(rustc --version | tr ' ' '_')"

for depth in ${depths}; do
  busy_start="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=kfd depth=%s gpu_busy_start_percent=%s\n' "${depth}" "${busy_start}"
  result="$(timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/target/release/examples/kfd-sdma-copy-benchmark" \
    "${unique_id}" "${bytes}" "${depth}" "${warmups}" "${samples}" "${kfd_profile}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=kfd depth=%s gpu_busy_end_percent=%s\n%s\n' \
    "${depth}" "${busy_end}" "${result}"
  busy_start="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=hsa depth=%s gpu_busy_start_percent=%s\n' "${depth}" "${busy_start}"
  result="$(HSA_XNACK=0 ROCR_VISIBLE_DEVICES="${gpu_index}" \
    timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/async-copy-hsa" \
    0 "${bytes}" "${depth}" "${warmups}" "${samples}" "${unique_id}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=hsa depth=%s gpu_busy_end_percent=%s\n%s\n' \
    "${depth}" "${busy_end}" "${result}"
  busy_start="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=hip depth=%s gpu_busy_start_percent=%s\n' "${depth}" "${busy_start}"
  result="$(HSA_XNACK=0 HIP_VISIBLE_DEVICES="${gpu_index}" \
    timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/async-copy-hip" \
    0 "${bytes}" "${depth}" "${warmups}" "${samples}" "${unique_id}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  printf 'context phase=hip depth=%s gpu_busy_end_percent=%s\n%s\n' \
    "${depth}" "${busy_end}" "${result}"
done

for depth in ${depths}; do
  busy_start="$(require_idle_gpu "${gpu_index}")"
  second_busy_start="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=kfd-multi depth_per_device=%s gpu_busy_start_percent=%s,%s\n' \
    "${depth}" "${busy_start}" "${second_busy_start}"
  result="$(timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/target/release/examples/kfd-sdma-multi-device-benchmark" \
    "${unique_id}" "${second_unique_id}" "${bytes}" "${depth}" "${warmups}" "${samples}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  second_busy_end="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=kfd-multi depth_per_device=%s gpu_busy_end_percent=%s,%s\n%s\n' \
    "${depth}" "${busy_end}" "${second_busy_end}" "${result}"
  busy_start="$(require_idle_gpu "${gpu_index}")"
  second_busy_start="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=hsa-multi depth_per_device=%s gpu_busy_start_percent=%s,%s\n' \
    "${depth}" "${busy_start}" "${second_busy_start}"
  result="$(HSA_XNACK=0 ROCR_VISIBLE_DEVICES="${gpu_index},${second_gpu_index}" \
    timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/async-copy-multi-hsa" 0 1 "${bytes}" "${depth}" "${warmups}" \
    "${samples}" "${unique_id}" "${second_unique_id}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  second_busy_end="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=hsa-multi depth_per_device=%s gpu_busy_end_percent=%s,%s\n%s\n' \
    "${depth}" "${busy_end}" "${second_busy_end}" "${result}"
  busy_start="$(require_idle_gpu "${gpu_index}")"
  second_busy_start="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=hip-multi depth_per_device=%s gpu_busy_start_percent=%s,%s\n' \
    "${depth}" "${busy_start}" "${second_busy_start}"
  result="$(HSA_XNACK=0 HIP_VISIBLE_DEVICES="${gpu_index},${second_gpu_index}" \
    timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s" \
    "${build_dir}/async-copy-multi-hip" 0 1 "${bytes}" "${depth}" "${warmups}" \
    "${samples}" "${unique_id}" "${second_unique_id}")"
  busy_end="$(require_idle_gpu "${gpu_index}")"
  second_busy_end="$(require_idle_gpu "${second_gpu_index}")"
  printf 'context phase=hip-multi depth_per_device=%s gpu_busy_end_percent=%s,%s\n%s\n' \
    "${depth}" "${busy_end}" "${second_busy_end}" "${result}"
done
busy_after="$(require_idle_gpu "${gpu_index}")"
second_busy_after="$(require_idle_gpu "${second_gpu_index}")"
printf 'context gpu_busy_after_percent=%s,%s\n' \
  "${busy_after}" "${second_busy_after}"
