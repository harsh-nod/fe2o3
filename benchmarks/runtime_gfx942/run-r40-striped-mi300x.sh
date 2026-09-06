#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(/usr/bin/dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
readonly gpu_index=2
readonly expected_unique_id=0xd2e26fef80cf5c33
readonly depth=112
readonly warmups=10
readonly samples=30
readonly max_busy=5
readonly phase_timeout=180
readonly monitor_interval_us=2000
readonly monitor_maximum_gap_us=10000
readonly execution_environment=env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1
readonly build_environment=env-i-explicit-home-toolchain-path-cargo-incremental-0-private-target-v1
readonly system_path=/usr/sbin:/usr/bin:/sbin:/bin
readonly output_dir_input="${FE2O3_R40_OUTPUT_DIR:-}"
build_dir="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/fe2o3-r40-striped-qual.XXXXXX")"
readonly build_dir
readonly source_archive="${build_dir}/source.tar"
readonly source_tree="${build_dir}/source"
readonly snapshot_dir="${build_dir}/inputs"
readonly snapshot_manifest="${build_dir}/input-sha256.txt"
persist_staging=""
verify_tree=""
active_monitor_pid=""
artifact_dir=""
artifact_archive=""
artifact_archive_digest=""
publication_complete=0
publication_cleanup_armed=0

readonly -a qualification_env=(
  /usr/bin/env -i
  LANG=C
  LC_ALL=C
  PATH="${system_path}"
)

cleanup() {
  [[ -z "${active_monitor_pid}" ]] || {
    kill -TERM "${active_monitor_pid}" 2>/dev/null || true
    wait "${active_monitor_pid}" 2>/dev/null || true
    active_monitor_pid=""
  }
  [[ ! -e "${source_tree}" ]] || \
    "${qualification_env[@]}" /usr/bin/chmod -R u+rwX -- "${source_tree}"
  for owned_path in "${verify_tree}" "${persist_staging}" "${build_dir}"; do
    [[ -z "${owned_path}" || ! -e "${owned_path}" ]] || \
      "${qualification_env[@]}" /usr/bin/find "${owned_path}" -depth -delete
  done
  if ((publication_cleanup_armed == 1 && publication_complete == 0)); then
    for unpublished_path in \
      "${artifact_dir}" "${artifact_archive}" "${artifact_archive_digest}"; do
      [[ -z "${unpublished_path}" || ! -e "${unpublished_path}" ]] || \
        "${qualification_env[@]}" /usr/bin/find \
          "${unpublished_path}" -depth -delete
    done
  fi
}

forward_signal() {
  local signal_name="$1"
  local exit_code="$2"
  trap - HUP INT QUIT TERM
  if [[ "${active_monitor_pid}" =~ ^[1-9][0-9]*$ ]]; then
    kill -s "${signal_name}" "${active_monitor_pid}" 2>/dev/null || true
    wait "${active_monitor_pid}" 2>/dev/null || true
    active_monitor_pid=""
  fi
  exit "${exit_code}"
}

trap cleanup EXIT
trap 'forward_signal HUP 129' HUP
trap 'forward_signal INT 130' INT
trap 'forward_signal QUIT 131' QUIT
trap 'forward_signal TERM 143' TERM

sha256_file() {
  local path="$1"
  local digest
  IFS=' ' read -r digest _ < <(
    "${qualification_env[@]}" /usr/bin/sha256sum -- "${path}"
  )
  [[ "${digest}" =~ ^[0-9a-f]{64}$ ]] || {
    printf 'could not hash R40 artifact: %s\n' "${path}" >&2
    exit 2
  }
  printf '%s' "${digest}"
}

record_field() {
  local record="$1"
  local requested="$2"
  local token
  local found=""
  local -a tokens
  read -r -a tokens <<<"${record}"
  for token in "${tokens[@]}"; do
    case "${token}" in
      "${requested}="*)
        [[ -z "${found}" ]] || return 1
        found="${token#*=}"
        ;;
    esac
  done
  [[ -n "${found}" ]] || return 1
  printf '%s' "${found}"
}

sanitize_version() {
  "${qualification_env[@]}" /usr/bin/tr '[:space:]' '_'
}

gpu_busy_percent() {
  # shellcheck disable=SC2016
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" --showuse 2>/dev/null |
    "${qualification_env[@]}" /usr/bin/awk -v gpu="GPU[${gpu_index}]" \
      '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

require_gpu_load_at_most() {
  local maximum="$1"
  local observed
  observed="$(gpu_busy_percent)"
  [[ "${observed}" =~ ^[0-9]+$ ]] || {
    printf 'could not observe GPU 2 busy percentage\n' >&2
    exit 2
  }
  ((observed <= maximum)) || {
    printf 'GPU 2 is %s%% busy; limit is %s%%\n' "${observed}" "${maximum}" >&2
    exit 2
  }
  printf '%s' "${observed}"
}

resolve_unique_id() {
  # shellcheck disable=SC2016
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
    "${qualification_env[@]}" /usr/bin/awk -v gpu="GPU[${gpu_index}]" \
      '$1 == gpu { value = $NF } END { if (value != "") print value }' |
    "${qualification_env[@]}" /usr/bin/tr 'A-F' 'a-f'
}

capture_telemetry() {
  local phase_id="$1"
  local edge="$2"
  local busy="$3"
  local snapshot="${build_dir}/telemetry-${phase_id}-${edge}.txt"
  local digest
  local encoded
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" \
    --showuse --showclocks --showpower >"${snapshot}"
  [[ -s "${snapshot}" ]] || {
    printf 'empty telemetry for %s %s\n' "${phase_id}" "${edge}" >&2
    exit 2
  }
  "${qualification_env[@]}" /usr/bin/grep -Fq "GPU[${gpu_index}]" "${snapshot}" || {
    printf 'telemetry omits selected GPU 2\n' >&2
    exit 2
  }
  digest="$(sha256_file "${snapshot}")"
  encoded="$(
    "${qualification_env[@]}" /usr/bin/base64 <"${snapshot}" |
      "${qualification_env[@]}" /usr/bin/tr -d '\n'
  )"
  printf 'telemetry phase=%s edge=%s gpu_busy_percent=%s telemetry_sha256=%s telemetry_base64=%s\n' \
    "${phase_id}" "${edge}" "${busy}" "${digest}" "${encoded}"
}

for tool in \
  "${rocm_path}/bin/hipcc" \
  "${rocm_path}/bin/rocm-smi" \
  /usr/bin/awk /usr/bin/base64 /usr/bin/basename /usr/bin/cat /usr/bin/chmod \
  /usr/bin/cmp \
  /usr/bin/cp /usr/bin/dirname /usr/bin/env /usr/bin/find /usr/bin/g++ \
  /usr/bin/git /usr/bin/grep /usr/bin/ldd /usr/bin/mkdir /usr/bin/mktemp \
  /usr/bin/gzip /usr/bin/mv /usr/bin/numactl /usr/bin/python3 /usr/bin/readelf \
  /usr/bin/sed /usr/bin/sha256sum /usr/bin/tar /usr/bin/taskset \
  /usr/bin/tee /usr/bin/timeout /usr/bin/tr /usr/bin/true /usr/bin/zstd \
  /usr/sbin/modinfo; do
  [[ -x "${tool}" ]] || {
    printf 'missing executable: %s\n' "${tool}" >&2
    exit 2
  }
done
[[ -c /dev/kfd ]] || { printf 'missing /dev/kfd\n' >&2; exit 2; }
readonly build_home="${HOME:-}"
[[ "${build_home}" == /* && -d "${build_home}" ]] || {
  printf 'HOME must name one absolute Rust toolchain directory\n' >&2
  exit 2
}
readonly rust_tool_path="${build_home}/.cargo/bin"
readonly cargo_executable="${rust_tool_path}/cargo"
readonly rustc_executable="${rust_tool_path}/rustc"
[[ -x "${cargo_executable}" && -x "${rustc_executable}" ]] || {
  printf 'Rust toolchain shims are unavailable\n' >&2
  exit 2
}
for cargo_config in "${build_home}/.cargo/config" "${build_home}/.cargo/config.toml"; do
  [[ ! -e "${cargo_config}" ]] || {
    printf 'ambient Cargo configuration is not allowed: %s\n' "${cargo_config}" >&2
    exit 2
  }
done
readonly -a rust_build_env=(
  /usr/bin/env -i
  HOME="${build_home}"
  LANG=C
  LC_ALL=C
  PATH="${rust_tool_path}:${system_path}"
  CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR="${build_dir}/target"
)
readonly -a native_build_env=(
  /usr/bin/env -i
  LANG=C
  LC_ALL=C
  PATH="${rocm_path}/bin:${system_path}"
  ROCM_PATH="${rocm_path}"
)
[[ -n "${output_dir_input}" && -d "${output_dir_input}" && \
  -w "${output_dir_input}" ]] || {
  printf 'FE2O3_R40_OUTPUT_DIR must be an existing writable directory\n' >&2
  exit 2
}
output_dir="$(cd -- "${output_dir_input}" && pwd -P)"
readonly output_dir
case "${output_dir}/" in
  "${repo_root}/"*)
    printf 'R40 output directory must be outside the checkout\n' >&2
    exit 2
    ;;
esac
[[ -z "$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" status \
    --porcelain=v1 --untracked-files=all --ignore-submodules=none
)" ]] || {
  printf 'R40 qualification requires a clean checkout\n' >&2
  exit 2
}

git_commit="$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" rev-parse HEAD
)"
readonly git_commit
"${qualification_env[@]}" /usr/bin/mkdir -m 700 -- \
  "${snapshot_dir}" "${source_tree}"
"${qualification_env[@]}" /usr/bin/git -C "${repo_root}" archive \
  --format=tar "${git_commit}" >"${source_archive}"
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${source_archive}"
"${qualification_env[@]}" /usr/bin/tar --extract --file="${source_archive}" \
  --directory="${source_tree}" --no-same-owner --no-same-permissions
"${qualification_env[@]}" /usr/bin/chmod -R u-w,go-rwx -- "${source_tree}"
readonly archived_benchmark_dir="${source_tree}/benchmarks/runtime_gfx942"
readonly checker="${snapshot_dir}/check-r40-striped.py"
readonly r26_checker="${snapshot_dir}/check-parity.py"
readonly host_guard="${snapshot_dir}/r26-host-guard.py"
readonly system_identity_collector="${snapshot_dir}/r26-system-identity.py"
readonly hip_source="${snapshot_dir}/striped_copy_hip.cpp"
readonly hsa_source="${snapshot_dir}/striped_copy_hsa.cpp"
readonly common_header="${snapshot_dir}/striped_copy_benchmark_common.hpp"
readonly args_header="${snapshot_dir}/native_benchmark_args.hpp"
readonly pool_header="${snapshot_dir}/r26_hsa_pool_policy.hpp"
readonly runner_snapshot="${snapshot_dir}/run-r40-striped-mi300x.sh"
readonly -a snapshot_sources=(
  "${archived_benchmark_dir}/check-r40-striped.py"
  "${archived_benchmark_dir}/check-parity.py"
  "${archived_benchmark_dir}/r26-host-guard.py"
  "${archived_benchmark_dir}/r26-system-identity.py"
  "${archived_benchmark_dir}/striped_copy_hip.cpp"
  "${archived_benchmark_dir}/striped_copy_hsa.cpp"
  "${archived_benchmark_dir}/striped_copy_benchmark_common.hpp"
  "${archived_benchmark_dir}/native_benchmark_args.hpp"
  "${archived_benchmark_dir}/r26_hsa_pool_policy.hpp"
  "${archived_benchmark_dir}/run-r40-striped-mi300x.sh"
)
readonly -a snapshot_inputs=(
  "${checker}" "${r26_checker}" "${host_guard}"
  "${system_identity_collector}" "${hip_source}" "${hsa_source}"
  "${common_header}" "${args_header}" "${pool_header}" "${runner_snapshot}"
)
for index in "${!snapshot_sources[@]}"; do
  "${qualification_env[@]}" /usr/bin/cp -- \
    "${snapshot_sources[index]}" "${snapshot_inputs[index]}"
done
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${snapshot_inputs[@]}"
"${qualification_env[@]}" /usr/bin/sha256sum -- \
  "${snapshot_inputs[@]}" "${source_archive}" >"${snapshot_manifest}"
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${snapshot_manifest}"

verify_staged_inputs() {
  "${qualification_env[@]}" /usr/bin/sha256sum --check --status \
    "${snapshot_manifest}" || {
    printf 'private R40 input snapshot changed during qualification\n' >&2
    exit 2
  }
}
verify_staged_inputs

unique_id="$(resolve_unique_id)"
readonly unique_id
[[ "${unique_id}" == "${expected_unique_id}" ]] || {
  printf 'GPU 2 unique ID mismatch: expected %s, observed %s\n' \
    "${expected_unique_id}" "${unique_id}" >&2
  exit 2
}
readonly uuid="GPU-${unique_id#0x}"
counterbalance_seed="${build_dir}/counterbalance-seed.txt"
printf '%s\n%s\n%s\n' "${git_commit}" "${unique_id}" "${build_dir}" \
  >"${counterbalance_seed}"
counterbalance_set_id="$(sha256_file "${counterbalance_seed}")"
readonly counterbalance_set_id
artifact_dir="${output_dir}/r40-striped-${counterbalance_set_id}"
readonly artifact_dir
artifact_archive="${artifact_dir}.tar.gz"
readonly artifact_archive
artifact_archive_digest="${artifact_archive}.sha256"
readonly artifact_archive_digest
[[ ! -e "${artifact_dir}" && ! -e "${artifact_archive}" && \
  ! -e "${artifact_archive_digest}" ]] || {
  printf 'R40 evidence destination already exists\n' >&2
  exit 2
}
publication_cleanup_armed=1

(
  cd -- "${source_tree}"
  "${rust_build_env[@]}" "${cargo_executable}" build --locked --release \
    --manifest-path "${source_tree}/Cargo.toml" -p fe2o3-kfd \
    --features live-validation --example kfd-sdma-copy-benchmark
)
"${native_build_env[@]}" "${rocm_path}/bin/hipcc" \
  -std=c++17 -O3 -Wall -Wextra -Werror -I"${snapshot_dir}" \
  "${hip_source}" -o "${build_dir}/striped-copy-hip"
"${native_build_env[@]}" /usr/bin/g++ \
  -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${snapshot_dir}" -I"${rocm_path}/include" "${hsa_source}" \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/striped-copy-hsa"
readonly kfd_binary="${build_dir}/target/release/examples/kfd-sdma-copy-benchmark"
readonly hsa_binary="${build_dir}/striped-copy-hsa"
readonly hip_binary="${build_dir}/striped-copy-hip"
for binary in "${kfd_binary}" "${hsa_binary}" "${hip_binary}"; do
  [[ -s "${binary}" ]] || {
    printf 'missing R40 benchmark binary: %s\n' "${binary}" >&2
    exit 2
  }
done
"${qualification_env[@]}" /usr/bin/chmod 0500 -- \
  "${kfd_binary}" "${hsa_binary}" "${hip_binary}"
verify_staged_inputs

kfd_binary_sha256="$(sha256_file "${kfd_binary}")"
readonly kfd_binary_sha256
hsa_binary_sha256="$(sha256_file "${hsa_binary}")"
readonly hsa_binary_sha256
hip_binary_sha256="$(sha256_file "${hip_binary}")"
readonly hip_binary_sha256

collect_system_identity() {
  local edge="$1"
  "${qualification_env[@]}" /usr/bin/python3 "${system_identity_collector}" \
    --observation-edge "${edge}" --gpu-index "${gpu_index}" \
    --rocm-path "${rocm_path}" --kfd-binary "${kfd_binary}" \
    --hsa-binary "${hsa_binary}" --hip-binary "${hip_binary}"
}

system_identity_start="$(collect_system_identity start)"
readonly system_identity_start
[[ "${system_identity_start}" == 'context schema=fe2o3.r26-system-identity.v1 '* && \
  "${system_identity_start}" != *$'\n'* ]] || {
  printf 'system identity collector emitted a malformed start record\n' >&2
  exit 2
}
pci_bdf="$(record_field "${system_identity_start}" pci_bdf)"
readonly pci_bdf
identity_unique_id="$(record_field "${system_identity_start}" unique_id)"
readonly identity_unique_id
[[ "${identity_unique_id}" == "${unique_id}" ]] || {
  printf 'system identity unique ID mismatch\n' >&2
  exit 2
}

collect_host_topology() {
  "${qualification_env[@]}" /usr/bin/python3 "${host_guard}" topology \
    --gpu-index "${gpu_index}" --pci-bdf "${pci_bdf}" \
    --unique-id "${identity_unique_id}"
}

host_topology="$(collect_host_topology)"
readonly host_topology
[[ "${host_topology}" == 'topology schema=fe2o3.r26-host-topology.v1 '* && \
  "${host_topology}" != *$'\n'* ]] || {
  printf 'host guard emitted malformed topology\n' >&2
  exit 2
}
topology_numa_node="$(record_field "${host_topology}" numa_node)"
readonly topology_numa_node
measurement_cpu_list="$(record_field "${host_topology}" measurement_cpu_list)"
readonly measurement_cpu_list
observer_cpu="$(record_field "${host_topology}" observer_cpu)"
readonly observer_cpu
kfd_gpu_id="$(record_field "${host_topology}" kfd_gpu_id)"
readonly kfd_gpu_id
topology_sha256="$(record_field "${host_topology}" topology_sha256)"
readonly topology_sha256
[[ "${topology_numa_node}" =~ ^[0-9]+$ && \
  "${observer_cpu}" =~ ^[0-9]+$ && "${kfd_gpu_id}" =~ ^[1-9][0-9]*$ && \
  "${topology_sha256}" =~ ^[0-9a-f]{64}$ ]] || {
  printf 'host topology has malformed placement fields\n' >&2
  exit 2
}
"${qualification_env[@]}" /usr/bin/taskset --cpu-list "${observer_cpu}" \
  /usr/bin/taskset --cpu-list "${measurement_cpu_list}" \
  /usr/bin/numactl --physcpubind="${measurement_cpu_list}" \
  --membind="${topology_numa_node}" /usr/bin/true

capture_topology() {
  local slot="$1"
  local phase_id="$2"
  local edge="$3"
  local observed
  observed="$(collect_host_topology)"
  [[ "${observed}" == "${host_topology}" ]] || {
    printf 'host topology changed at %s %s\n' "${phase_id}" "${edge}" >&2
    exit 2
  }
  printf 'topology slot=%s phase=%s edge=%s %s\n' \
    "${slot}" "${phase_id}" "${edge}" "${observed#topology }"
}

rocm_version=unknown
[[ ! -r "${rocm_path}/.info/version" ]] || \
  IFS= read -r rocm_version <"${rocm_path}/.info/version"
rocm_version="$(printf '%s' "${rocm_version}" | sanitize_version)"
readonly rocm_version
rustc_version="$(
  cd -- "${source_tree}" &&
    "${rust_build_env[@]}" "${rustc_executable}" --version | sanitize_version
)"
readonly rustc_version
cargo_version="$(
  cd -- "${source_tree}" &&
    "${rust_build_env[@]}" "${cargo_executable}" --version | sanitize_version
)"
readonly cargo_version
hipcc_version="$(
  "${native_build_env[@]}" "${rocm_path}/bin/hipcc" --version |
    "${qualification_env[@]}" /usr/bin/sed -n '1p' | sanitize_version
)"
readonly hipcc_version
cxx_version="$(
  "${native_build_env[@]}" /usr/bin/g++ --version |
    "${qualification_env[@]}" /usr/bin/sed -n '1p' | sanitize_version
)"
readonly cxx_version

print_context() {
  local slot="$1"
  local backend_order="$2"
  local workload_order="$3"
  printf 'context schema=fe2o3.r40-striped-evidence.v1 git_commit=%s target=gfx942:xnack- gpu_index=2 unique_id=%s uuid=%s depth=112 warmups=10 samples=30 bytes_set=4096,1048576 logical_queue_counts=2,4,8,14,16 profiles=combined-striped2,combined-striped4,combined-striped8,combined-striped14,striped16 max_busy_percent=5 phase_timeout_seconds=180 rocm_version=%s rustc=%s cargo=%s hipcc=%s cxx=%s kfd_binary_sha256=%s hsa_binary_sha256=%s hip_binary_sha256=%s hsa_source_sha256=%s hip_source_sha256=%s common_header_sha256=%s checker_sha256=%s runner_sha256=%s host_guard_sha256=%s system_identity_collector_sha256=%s build_environment=%s execution_environment=%s telemetry_command=rocm-smi-showuse-showclocks-showpower placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 interference_monitor=selected-kfd-gpu-process-tree-census-v2 monitor_interval_us=%s monitor_maximum_gap_us=%s topology_sha256=%s counterbalance_design=cyclic-latin-square-3-backends-workload-forward-reverse-rotate5-v1 counterbalance_slots=3 counterbalance_slot=%s counterbalance_set_id=%s backend_order=%s workload_order=%s claim_scope=single-mi300x-gpu2-striped-copy-only\n' \
    "${git_commit}" "${unique_id}" "${uuid}" "${rocm_version}" \
    "${rustc_version}" "${cargo_version}" "${hipcc_version}" \
    "${cxx_version}" "${kfd_binary_sha256}" "${hsa_binary_sha256}" \
    "${hip_binary_sha256}" "$(sha256_file "${hsa_source}")" \
    "$(sha256_file "${hip_source}")" "$(sha256_file "${common_header}")" \
    "$(sha256_file "${checker}")" "$(sha256_file "${runner_snapshot}")" \
    "$(sha256_file "${host_guard}")" \
    "$(sha256_file "${system_identity_collector}")" \
    "${build_environment}" "${execution_environment}" \
    "${monitor_interval_us}" "${monitor_maximum_gap_us}" "${topology_sha256}" \
    "${slot}" "${counterbalance_set_id}" "${backend_order}" \
    "${workload_order}"
}

run_phase() {
  local slot="$1"
  local sequence="$2"
  local workload_id="$3"
  local backend="$4"
  local bytes="$5"
  local queue_count="$6"
  local profile="$7"
  local phase_id="${workload_id}.${backend}"
  local target_output="${build_dir}/target-slot-${slot}-sequence-${sequence}.out"
  local monitor_output="${build_dir}/monitor-slot-${slot}-sequence-${sequence}.out"
  local start_topology
  local start_telemetry
  local end_telemetry
  local end_topology
  local monitor_record
  local busy
  local monitor_status
  local -a command
  start_topology="$(capture_topology "${slot}" "${phase_id}" start)"
  busy="$(require_gpu_load_at_most "${max_busy}")"
  start_telemetry="$(capture_telemetry "${phase_id}" start "${busy}")"
  case "${backend}" in
    kfd)
      command=("${qualification_env[@]}"
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s
        "${phase_timeout}s" "${kfd_binary}" "${unique_id}" "${bytes}"
        "${depth}" "${warmups}" "${samples}" "${profile}" aggregate)
      ;;
    hsa)
      command=("${qualification_env[@]}" HSA_XNACK=0 ROCR_VISIBLE_DEVICES=2
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s
        "${phase_timeout}s" "${hsa_binary}" 0 "${unique_id}" "${bytes}"
        "${depth}" "${warmups}" "${samples}" "${queue_count}" "${profile}")
      ;;
    hip)
      command=("${qualification_env[@]}" HSA_XNACK=0 HIP_VISIBLE_DEVICES=2
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s
        "${phase_timeout}s" "${hip_binary}" 0 "${unique_id}" "${bytes}"
        "${depth}" "${warmups}" "${samples}" "${queue_count}" "${profile}")
      ;;
    *)
      printf 'unsupported backend: %s\n' "${backend}" >&2
      exit 2
      ;;
  esac
  "${qualification_env[@]}" /usr/bin/python3 "${host_guard}" monitor \
    --gpu-id "${kfd_gpu_id}" --observer-cpu "${observer_cpu}" \
    --target-output "${target_output}" -- "${command[@]}" >"${monitor_output}" &
  active_monitor_pid=$!
  if wait "${active_monitor_pid}"; then
    monitor_status=0
  else
    monitor_status=$?
  fi
  active_monitor_pid=""
  ((monitor_status == 0)) || return "${monitor_status}"
  monitor_record="$("${qualification_env[@]}" /usr/bin/cat -- "${monitor_output}")"
  [[ "${monitor_record}" == 'monitor schema=fe2o3.r26-kfd-queue-monitor.v2 '* && \
    "${monitor_record}" != *$'\n'* ]] || {
    printf 'host guard emitted malformed monitor record\n' >&2
    exit 2
  }
  if [[ "${backend}" == kfd ]]; then
    busy="$(require_gpu_load_at_most 0)"
  else
    busy="$(require_gpu_load_at_most "${max_busy}")"
  fi
  end_telemetry="$(capture_telemetry "${phase_id}" end "${busy}")"
  end_topology="$(capture_topology "${slot}" "${phase_id}" end)"
  "${qualification_env[@]}" /usr/bin/python3 - "${target_output}" <<'PY'
import pathlib
import sys

data = pathlib.Path(sys.argv[1]).read_bytes()
if not data.endswith(b"\n") or data.count(b"\n") != 1 or b"\0" in data:
    raise SystemExit("R40 target must emit exactly one LF-terminated text row")
PY
  printf 'phase slot=%s sequence=%s workload_id=%s backend=%s phase_id=%s\n' \
    "${slot}" "${sequence}" "${workload_id}" "${backend}" "${phase_id}"
  printf '%s\n%s\n' "${start_topology}" "${start_telemetry}"
  printf 'monitor slot=%s phase=%s %s\n' \
    "${slot}" "${phase_id}" "${monitor_record#monitor }"
  printf '%s\n%s\n' "${end_telemetry}" "${end_topology}"
  "${qualification_env[@]}" /usr/bin/cat -- "${target_output}"
}

readonly -a backend_orders=('kfd hsa hip' 'hsa hip kfd' 'hip kfd hsa')
readonly -a workload_forward=(
  bytes4096-q2-combined bytes4096-q4-combined bytes4096-q8-combined
  bytes4096-q14-combined bytes4096-q16-standalone
  bytes1048576-q2-combined bytes1048576-q4-combined
  bytes1048576-q8-combined bytes1048576-q14-combined
  bytes1048576-q16-standalone
)
readonly -a workload_reverse=(
  bytes1048576-q16-standalone bytes1048576-q14-combined
  bytes1048576-q8-combined bytes1048576-q4-combined
  bytes1048576-q2-combined bytes4096-q16-standalone
  bytes4096-q14-combined bytes4096-q8-combined bytes4096-q4-combined
  bytes4096-q2-combined
)
readonly -a workload_rotate5=(
  bytes1048576-q2-combined bytes1048576-q4-combined
  bytes1048576-q8-combined bytes1048576-q14-combined
  bytes1048576-q16-standalone bytes4096-q2-combined
  bytes4096-q4-combined bytes4096-q8-combined bytes4096-q14-combined
  bytes4096-q16-standalone
)

slot_logs=()
phase_count=0
for slot in 0 1 2; do
  slot_log="${build_dir}/slot-${slot}.log"
  slot_logs+=("${slot_log}")
  read -r -a backend_order <<<"${backend_orders[slot]}"
  case "${slot}" in
    0) workload_order=("${workload_forward[@]}") ;;
    1) workload_order=("${workload_reverse[@]}") ;;
    2) workload_order=("${workload_rotate5[@]}") ;;
  esac
  {
    print_context "${slot}" "${backend_orders[slot]// /,}" \
      "$(IFS=,; printf '%s' "${workload_order[*]}")"
    printf '%s\n' "${system_identity_start}"
    sequence=0
    for workload_id in "${workload_order[@]}"; do
      bytes="${workload_id#bytes}"
      bytes="${bytes%%-*}"
      queue_count="${workload_id#*-q}"
      queue_count="${queue_count%%-*}"
      if [[ "${queue_count}" == 16 ]]; then
        profile=striped16
      else
        profile="combined-striped${queue_count}"
      fi
      for backend in "${backend_order[@]}"; do
        run_phase "${slot}" "${sequence}" "${workload_id}" "${backend}" \
          "${bytes}" "${queue_count}" "${profile}"
        ((sequence += 1))
        ((phase_count += 1))
      done
    done
    ((sequence == 30)) || {
      printf 'R40 slot %s did not execute exactly 30 phases\n' "${slot}" >&2
      exit 2
    }
  } >"${slot_log}"
done
readonly -a slot_logs
((phase_count == 90)) || {
  printf 'R40 set did not execute exactly 90 phases\n' >&2
  exit 2
}

system_identity_end="$(collect_system_identity end)"
readonly system_identity_end
[[ "${system_identity_end}" == 'context schema=fe2o3.r26-system-identity.v1 '* && \
  "${system_identity_end}" != *$'\n'* ]] || {
  printf 'system identity collector emitted a malformed end record\n' >&2
  exit 2
}
for slot_log in "${slot_logs[@]}"; do
  printf '%s\n' "${system_identity_end}" >>"${slot_log}"
done
readonly set_report="${build_dir}/set-validation.txt"
"${qualification_env[@]}" /usr/bin/python3 "${checker}" \
  "${slot_logs[@]}" | "${qualification_env[@]}" /usr/bin/tee "${set_report}"
[[ "$(require_gpu_load_at_most 0)" == 0 ]] || exit 2

persist_staging="${output_dir}/.r40-striped-${counterbalance_set_id}.tmp.$$"
"${qualification_env[@]}" /usr/bin/mkdir -m 700 -- "${persist_staging}"
for slot in 0 1 2; do
  "${qualification_env[@]}" /usr/bin/cp -- \
    "${slot_logs[slot]}" "${persist_staging}/slot-${slot}.log"
done
"${qualification_env[@]}" /usr/bin/cp -- \
  "${set_report}" "${persist_staging}/set-validation.txt"
"${qualification_env[@]}" /usr/bin/cp -- \
  "${source_archive}" "${persist_staging}/source.tar"
(
  cd -- "${persist_staging}"
  "${qualification_env[@]}" /usr/bin/sha256sum -- \
    set-validation.txt slot-0.log slot-1.log slot-2.log source.tar \
    >evidence-sha256.txt
  "${qualification_env[@]}" /usr/bin/sha256sum --check --status \
    evidence-sha256.txt
)
verify_staged_inputs
"${qualification_env[@]}" /usr/bin/python3 "${checker}" \
  "${persist_staging}/slot-0.log" "${persist_staging}/slot-1.log" \
  "${persist_staging}/slot-2.log" >"${build_dir}/persisted-validation.txt"
"${qualification_env[@]}" /usr/bin/cmp --silent -- \
  "${set_report}" "${build_dir}/persisted-validation.txt" || {
  printf 'persisted R40 evidence failed exact revalidation\n' >&2
  exit 2
}
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${persist_staging}"/*
archive_staging="${build_dir}/$(/usr/bin/basename -- "${artifact_archive}")"
readonly archive_staging
readonly archive_digest_staging="${archive_staging}.sha256"
"${qualification_env[@]}" /usr/bin/tar --sort=name --mtime=@0 \
  --owner=0 --group=0 --numeric-owner --create --gzip \
  --file="${archive_staging}" --directory="${persist_staging}" .
archive_sha256="$(sha256_file "${archive_staging}")"
printf '%s  %s\n' "${archive_sha256}" \
  "$(/usr/bin/basename -- "${artifact_archive}")" >"${archive_digest_staging}"
verify_tree="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/fe2o3-r40-striped-verify.XXXXXX")"
"${qualification_env[@]}" /usr/bin/tar --extract --gzip \
  --file="${archive_staging}" --directory="${verify_tree}" \
  --no-same-owner --no-same-permissions
(
  cd -- "${verify_tree}"
  "${qualification_env[@]}" /usr/bin/sha256sum --check --status \
    evidence-sha256.txt
)
"${qualification_env[@]}" /usr/bin/find "${verify_tree}" -depth -delete
verify_tree=""
[[ "$(sha256_file "${archive_staging}")" == "${archive_sha256}" ]] || exit 2
"${qualification_env[@]}" /usr/bin/mv -- \
  "${archive_staging}" "${artifact_archive}"
"${qualification_env[@]}" /usr/bin/mv -- \
  "${archive_digest_staging}" "${artifact_archive_digest}"
"${qualification_env[@]}" /usr/bin/mv -T -- \
  "${persist_staging}" "${artifact_dir}"
persist_staging=""
[[ "$(sha256_file "${artifact_archive}")" == "${archive_sha256}" ]] || exit 2
[[ -z "$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" status \
    --porcelain=v1 --untracked-files=all --ignore-submodules=none
)" ]] || {
  printf 'checkout changed during R40 qualification\n' >&2
  exit 2
}
publication_complete=1
printf 'R40 evidence directory: %s\n' "${artifact_dir}"
printf 'R40 sealed archive: %s sha256=%s\n' \
  "${artifact_archive}" "${archive_sha256}"
