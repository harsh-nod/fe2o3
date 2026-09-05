#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(/usr/bin/dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly rocm_path="${ROCM_PATH:-/opt/rocm}"
readonly gpu_index="${FE2O3_R26_GPU_INDEX:-0}"
readonly max_busy="${FE2O3_R26_MAX_BUSY_PERCENT:-5}"
readonly phase_timeout="${FE2O3_R26_PHASE_TIMEOUT_SECONDS:-180}"
readonly bytes=1048576
readonly elements=262144
readonly workgroup=256
readonly warmups=10
readonly samples=30
readonly iterations_per_sample=10
readonly execution_environment=env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1
readonly build_environment=env-i-explicit-home-toolchain-path-cargo-incremental-0-private-target-v1
readonly system_path=/usr/sbin:/usr/bin:/sbin:/bin
readonly -a qualification_env=(
  /usr/bin/env -i
  LANG=C
  LC_ALL=C
  PATH="${system_path}"
)
readonly monitor_interval_us=2000
readonly monitor_maximum_gap_us=10000
readonly output_dir_input="${FE2O3_R26_OUTPUT_DIR:-}"
build_dir="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/fe2o3-r26-inplace-gfx942.XXXXXX")"
readonly build_dir
readonly snapshot_dir="${build_dir}/inputs"
readonly snapshot_manifest="${build_dir}/input-sha256.txt"
readonly source_archive="${build_dir}/source.tar"
readonly source_tree="${build_dir}/source"
persist_staging=""
active_monitor_pid=""

cleanup() {
  [[ ! -e "${source_tree}" ]] || \
    "${qualification_env[@]}" /usr/bin/chmod -R u+rwX -- "${source_tree}"
  [[ ! -e "${build_dir}" ]] || \
    "${qualification_env[@]}" /usr/bin/find "${build_dir}" -depth -delete
  [[ -z "${persist_staging}" || ! -e "${persist_staging}" ]] || \
    "${qualification_env[@]}" /usr/bin/find "${persist_staging}" -depth -delete
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

sanitize_version() {
  "${qualification_env[@]}" /usr/bin/tr '[:space:]' '_'
}

sha256_file() {
  local path="$1"
  local digest
  IFS=' ' read -r digest _ < <(
    "${qualification_env[@]}" /usr/bin/sha256sum -- "${path}"
  )
  [[ "${digest}" =~ ^[0-9a-f]{64}$ ]] || {
    printf 'could not hash R26 input: %s\n' "${path}" >&2
    exit 2
  }
  printf '%s' "${digest}"
}

gpu_busy_percent() {
  local index="$1"
  # shellcheck disable=SC2016
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" --showuse 2>/dev/null |
    "${qualification_env[@]}" /usr/bin/awk -v gpu="GPU[${index}]" \
      '$1 == gpu { value = $NF } END { if (value != "") print value }'
}

require_idle_gpu() {
  local observed
  observed="$(gpu_busy_percent "${gpu_index}")"
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

resolve_unique_id() {
  # shellcheck disable=SC2016
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" --showuniqueid 2>/dev/null |
    "${qualification_env[@]}" /usr/bin/awk -v gpu="GPU[${gpu_index}]" \
      '$1 == gpu { value = $NF } END { if (value != "") print value }' |
    "${qualification_env[@]}" /usr/bin/tr 'A-F' 'a-f'
}

capture_telemetry() {
  local slot="$1"
  local phase="$2"
  local edge="$3"
  local busy="$4"
  local snapshot="${build_dir}/telemetry-slot-${slot}-${phase}-${edge}.txt"
  "${qualification_env[@]}" "${rocm_path}/bin/rocm-smi" \
    --showuse --showclocks --showpower >"${snapshot}"
  [[ -s "${snapshot}" ]] || {
    printf 'empty ROCm telemetry snapshot for %s %s\n' "${phase}" "${edge}" >&2
    exit 2
  }
  "${qualification_env[@]}" /usr/bin/grep -Fq "GPU[${gpu_index}]" "${snapshot}" || {
    printf 'telemetry snapshot omits selected GPU %s\n' "${gpu_index}" >&2
    exit 2
  }
  local digest
  local encoded
  digest="$(sha256_file "${snapshot}")"
  encoded="$(
    "${qualification_env[@]}" /usr/bin/base64 <"${snapshot}" |
      "${qualification_env[@]}" /usr/bin/tr -d '\n'
  )"
  printf 'context phase=%s gpu_busy_%s_percent=%s telemetry_%s_sha256=%s telemetry_%s_base64=%s\n' \
    "${phase}" "${edge}" "${busy}" "${edge}" "${digest}" "${edge}" "${encoded}"
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

for tool in \
  "${rocm_path}/bin/hipcc" \
  "${rocm_path}/bin/rocm-smi"; do
  [[ -x "${tool}" ]] || {
    printf 'missing executable: %s\n' "${tool}" >&2
    exit 2
  }
done
[[ -c /dev/kfd ]] || { printf 'missing /dev/kfd\n' >&2; exit 2; }
for tool in /usr/bin/awk /usr/bin/base64 /usr/bin/cat /usr/bin/chmod /usr/bin/cmp \
  /usr/bin/cp /usr/bin/dirname /usr/bin/env /usr/bin/find /usr/bin/g++ /usr/bin/git \
  /usr/bin/grep /usr/bin/ldd /usr/bin/mkdir /usr/bin/mktemp /usr/bin/mv \
  /usr/bin/numactl /usr/bin/python3 /usr/bin/readelf /usr/bin/sed \
  /usr/bin/sha256sum /usr/bin/tar /usr/bin/taskset /usr/bin/tee /usr/bin/timeout \
  /usr/bin/tr /usr/bin/true /usr/bin/zstd /usr/sbin/modinfo; do
  [[ -x "${tool}" ]] || {
    printf 'missing fixed executable: %s\n' "${tool}" >&2
    exit 2
  }
done
readonly build_home="${HOME:-}"
[[ "${build_home}" == /* && -d "${build_home}" ]] || {
  printf 'HOME must name one absolute directory for the Rust toolchain\n' >&2
  exit 2
}
readonly rust_tool_path="${build_home}/.cargo/bin"
readonly cargo_executable="${rust_tool_path}/cargo"
readonly rustc_executable="${rust_tool_path}/rustc"
[[ -x "${cargo_executable}" && -x "${rustc_executable}" ]] || {
  printf 'pinned HOME Rust toolchain shims are unavailable\n' >&2
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
[[ -n "${output_dir_input}" ]] || {
  printf 'FE2O3_R26_OUTPUT_DIR must name an existing output directory\n' >&2
  exit 2
}
[[ -d "${output_dir_input}" && -w "${output_dir_input}" ]] || {
  printf 'R26 output directory must exist and be writable: %s\n' \
    "${output_dir_input}" >&2
  exit 2
}
output_dir="$(cd -- "${output_dir_input}" && pwd -P)"
readonly output_dir
case "${output_dir}/" in
  "${repo_root}/"*)
    printf 'R26 output directory must be outside the checkout\n' >&2
    exit 2
    ;;
esac
for value in "${gpu_index}" "${max_busy}" "${phase_timeout}"; do
  [[ "${value}" =~ ^[0-9]+$ ]] || {
    printf 'R26 controls must be nonnegative integers\n' >&2
    exit 2
  }
done
((max_busy <= 100 && phase_timeout >= 1 && phase_timeout <= 3600)) || {
  printf 'R26 load or timeout control is out of range\n' >&2
  exit 2
}
[[ -z "$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" status \
    --porcelain=v1 --untracked-files=all --ignore-submodules=none
)" ]] || {
  printf 'R26 qualification requires a clean checkout\n' >&2
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
readonly archived_fixture_dir="${source_tree}/crates/fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1"
readonly archived_benchmark_dir="${source_tree}/benchmarks/runtime_gfx942"
readonly hsaco="${snapshot_dir}/inplace_transform.hsaco"
readonly kernel_source="${snapshot_dir}/inplace_transform.ll"
readonly kernel_policy="${snapshot_dir}/policy-v1.txt"
readonly fixture_recipe="${snapshot_dir}/build-and-verify.sh"
readonly checker="${snapshot_dir}/check-parity.py"
readonly host_guard="${snapshot_dir}/r26-host-guard.py"
readonly system_identity_collector="${snapshot_dir}/r26-system-identity.py"
readonly hsa_source="${snapshot_dir}/inplace_transform_hsa.cpp"
readonly hip_source="${snapshot_dir}/inplace_transform_hip.cpp"
readonly binary_reader="${snapshot_dir}/bounded_binary_file_reader.hpp"
readonly common_header="${snapshot_dir}/inplace_benchmark_common.hpp"
readonly runner_snapshot="${snapshot_dir}/run-r26-inplace-mi300x.sh"
readonly -a snapshot_sources=(
  "${archived_fixture_dir}/inplace_transform.hsaco"
  "${archived_fixture_dir}/inplace_transform.ll"
  "${archived_fixture_dir}/policy-v1.txt"
  "${archived_fixture_dir}/build-and-verify.sh"
  "${archived_benchmark_dir}/check-parity.py"
  "${archived_benchmark_dir}/r26-host-guard.py"
  "${archived_benchmark_dir}/r26-system-identity.py"
  "${archived_benchmark_dir}/inplace_transform_hsa.cpp"
  "${archived_benchmark_dir}/inplace_transform_hip.cpp"
  "${archived_benchmark_dir}/bounded_binary_file_reader.hpp"
  "${archived_benchmark_dir}/inplace_benchmark_common.hpp"
  "${archived_benchmark_dir}/run-r26-inplace-mi300x.sh"
)
readonly -a copied_snapshot_inputs=(
  "${hsaco}"
  "${kernel_source}"
  "${kernel_policy}"
  "${fixture_recipe}"
  "${checker}"
  "${host_guard}"
  "${system_identity_collector}"
  "${hsa_source}"
  "${hip_source}"
  "${binary_reader}"
  "${common_header}"
  "${runner_snapshot}"
)
for index in "${!snapshot_sources[@]}"; do
  "${qualification_env[@]}" /usr/bin/cp -- \
    "${snapshot_sources[index]}" "${copied_snapshot_inputs[index]}"
done
readonly -a snapshot_inputs=("${copied_snapshot_inputs[@]}" "${source_archive}")
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${snapshot_inputs[@]}"
"${qualification_env[@]}" /usr/bin/sha256sum -- "${snapshot_inputs[@]}" \
  >"${snapshot_manifest}"
"${qualification_env[@]}" /usr/bin/chmod 0400 -- "${snapshot_manifest}"
verify_staged_inputs() {
  "${qualification_env[@]}" /usr/bin/sha256sum --check --status \
    "${snapshot_manifest}" || {
    printf 'private R26 input snapshot changed during qualification\n' >&2
    exit 2
  }
}
verify_staged_inputs

unique_id="$(resolve_unique_id)"
readonly unique_id
[[ "${unique_id}" =~ ^0x[0-9a-f]{16}$ ]] || {
  printf 'selected GPU does not have a canonical nonzero unique ID\n' >&2
  exit 2
}
[[ "${unique_id}" != 0x0000000000000000 ]] || {
  printf 'selected GPU has a zero unique ID\n' >&2
  exit 2
}
readonly uuid="GPU-${unique_id#0x}"
counterbalance_seed="${build_dir}/counterbalance-seed.txt"
printf '%s\n%s\n%s\n' "${git_commit}" "${unique_id}" "${build_dir}" \
  >"${counterbalance_seed}"
counterbalance_set_id="$(sha256_file "${counterbalance_seed}")"
readonly counterbalance_set_id
readonly counterbalance_design=cyclic-latin-square-3
readonly -a counterbalance_orders=(
  'kfd hsa hip'
  'hsa hip kfd'
  'hip kfd hsa'
)
readonly artifact_dir="${output_dir}/r26-inplace-${counterbalance_set_id}"
[[ ! -e "${artifact_dir}" ]] || {
  printf 'R26 output set already exists: %s\n' "${artifact_dir}" >&2
  exit 2
}

printf '%s  %s\n' \
  '8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9' \
  "${hsaco}" | "${qualification_env[@]}" /usr/bin/sha256sum --check --status
printf '%s  %s\n' \
  '1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5' \
  "${kernel_source}" | "${qualification_env[@]}" /usr/bin/sha256sum --check --status
printf '%s  %s\n' \
  'c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585' \
  "${kernel_policy}" | "${qualification_env[@]}" /usr/bin/sha256sum --check --status
printf '%s  %s\n' \
  '29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5' \
  "${fixture_recipe}" | "${qualification_env[@]}" /usr/bin/sha256sum --check --status
(
  cd "${source_tree}"
  "${rust_build_env[@]}" "${cargo_executable}" build --locked --release \
    --manifest-path "${source_tree}/Cargo.toml" \
    -p fe2o3-runtime --features hardware-qualification \
    --example gfx942-runtime-r26-inplace-benchmark
)
"${native_build_env[@]}" "${rocm_path}/bin/hipcc" \
  -std=c++17 -O3 -Wall -Wextra -Werror "${hip_source}" \
  -o "${build_dir}/inplace-transform-hip"
"${native_build_env[@]}" /usr/bin/g++ -std=c++17 -O3 -Wall -Wextra -Werror \
  -I"${snapshot_dir}" -I"${rocm_path}/include" "${hsa_source}" \
  -L"${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lhsa-runtime64 \
  -o "${build_dir}/inplace-transform-hsa"

readonly kfd_binary="${build_dir}/target/release/examples/gfx942-runtime-r26-inplace-benchmark"
readonly hsa_binary="${build_dir}/inplace-transform-hsa"
readonly hip_binary="${build_dir}/inplace-transform-hip"
for artifact in "${kfd_binary}" "${hsa_binary}" "${hip_binary}" "${hsaco}"; do
  [[ -s "${artifact}" ]] || {
    printf 'missing R26 artifact: %s\n' "${artifact}" >&2
    exit 2
  }
done
"${qualification_env[@]}" /usr/bin/chmod 0500 -- \
  "${kfd_binary}" "${hsa_binary}" "${hip_binary}"
verify_staged_inputs

collect_system_identity() {
  local edge="$1"
  "${qualification_env[@]}" /usr/bin/python3 "${system_identity_collector}" \
    --observation-edge "${edge}" \
    --gpu-index "${gpu_index}" \
    --rocm-path "${rocm_path}" \
    --kfd-binary "${kfd_binary}" \
    --hsa-binary "${hsa_binary}" \
    --hip-binary "${hip_binary}"
}

system_identity_start="$(collect_system_identity start)"
readonly system_identity_start
[[ "${system_identity_start}" == 'context schema=fe2o3.r26-system-identity.v1 '* && \
  "${system_identity_start}" != *$'\n'* ]] || {
  printf 'R26 system identity collector emitted a malformed record\n' >&2
  exit 2
}

pci_bdf="$(record_field "${system_identity_start}" pci_bdf)" || {
  printf 'R26 system identity omits one exact PCI BDF\n' >&2
  exit 2
}
readonly pci_bdf
identity_unique_id="$(record_field "${system_identity_start}" unique_id)" || {
  printf 'R26 system identity omits one exact unique ID\n' >&2
  exit 2
}
readonly identity_unique_id
[[ "${identity_unique_id}" == "${unique_id}" ]] || {
  printf 'R26 system identity unique ID changed after selection\n' >&2
  exit 2
}
collect_host_topology() {
  "${qualification_env[@]}" /usr/bin/python3 "${host_guard}" topology \
    --gpu-index "${gpu_index}" \
    --pci-bdf "${pci_bdf}" \
    --unique-id "${identity_unique_id}"
}

host_topology="$(collect_host_topology)"
readonly host_topology
[[ "${host_topology}" == 'topology schema=fe2o3.r26-host-topology.v1 '* && \
  "${host_topology}" != *$'\n'* ]] || {
  printf 'R26 host guard emitted a malformed topology record\n' >&2
  exit 2
}
topology_numa_node="$(record_field "${host_topology}" numa_node)" || exit 2
readonly topology_numa_node
[[ "${topology_numa_node}" =~ ^[0-9]+$ ]] || {
  printf 'R26 host topology has no usable NUMA node\n' >&2
  exit 2
}
measurement_cpu_list="$(record_field "${host_topology}" measurement_cpu_list)" || exit 2
readonly measurement_cpu_list
observer_cpu="$(record_field "${host_topology}" observer_cpu)" || exit 2
readonly observer_cpu
kfd_gpu_id="$(record_field "${host_topology}" kfd_gpu_id)" || exit 2
readonly kfd_gpu_id
topology_sha256="$(record_field "${host_topology}" topology_sha256)" || exit 2
readonly topology_sha256
[[ "${observer_cpu}" =~ ^[0-9]+$ && "${kfd_gpu_id}" =~ ^[1-9][0-9]*$ && \
  "${topology_sha256}" =~ ^[0-9a-f]{64}$ ]] || {
  printf 'R26 host topology has malformed placement fields\n' >&2
  exit 2
}
"${qualification_env[@]}" /usr/bin/taskset --cpu-list "${observer_cpu}" \
  /usr/bin/taskset --cpu-list "${measurement_cpu_list}" \
  /usr/bin/numactl \
  --physcpubind="${measurement_cpu_list}" \
  --membind="${topology_numa_node}" /usr/bin/true

capture_topology() {
  local slot="$1"
  local phase="$2"
  local edge="$3"
  local observed
  observed="$(collect_host_topology)"
  [[ "${observed}" == "${host_topology}" ]] || {
    printf 'R26 host topology changed before %s %s\n' "${phase}" "${edge}" >&2
    exit 2
  }
  printf 'topology slot=%s phase=%s edge=%s %s\n' \
    "${slot}" "${phase}" "${edge}" "${observed#topology }"
}

rocm_version=unknown
[[ ! -r "${rocm_path}/.info/version" ]] || \
  IFS= read -r rocm_version <"${rocm_path}/.info/version"
rocm_version="$(printf '%s' "${rocm_version}" | sanitize_version)"
readonly rocm_version
cargo_version="$(
  "${rust_build_env[@]}" "${cargo_executable}" --version | sanitize_version
)"
readonly cargo_version
rustc_version="$(
  "${rust_build_env[@]}" "${rustc_executable}" --version | sanitize_version
)"
readonly rustc_version
hipcc_version="$(
  "${native_build_env[@]}" "${rocm_path}/bin/hipcc" --version |
    "${qualification_env[@]}" /usr/bin/sed -n '1p' |
    sanitize_version
)"
readonly hipcc_version
cxx_version="$(
  "${native_build_env[@]}" /usr/bin/g++ --version |
    "${qualification_env[@]}" /usr/bin/sed -n '1p' |
    sanitize_version
)"
readonly cxx_version

hsaco_sha256="$(sha256_file "${hsaco}")"
readonly hsaco_sha256
kernel_source_sha256="$(sha256_file "${kernel_source}")"
readonly kernel_source_sha256
kernel_policy_sha256="$(sha256_file "${kernel_policy}")"
readonly kernel_policy_sha256
fixture_recipe_sha256="$(sha256_file "${fixture_recipe}")"
readonly fixture_recipe_sha256
kfd_binary_sha256="$(sha256_file "${kfd_binary}")"
readonly kfd_binary_sha256
hsa_binary_sha256="$(sha256_file "${hsa_binary}")"
readonly hsa_binary_sha256
hip_binary_sha256="$(sha256_file "${hip_binary}")"
readonly hip_binary_sha256
hsa_source_sha256="$(sha256_file "${hsa_source}")"
readonly hsa_source_sha256
hip_source_sha256="$(sha256_file "${hip_source}")"
readonly hip_source_sha256
binary_reader_sha256="$(sha256_file "${binary_reader}")"
readonly binary_reader_sha256
common_header_sha256="$(sha256_file "${common_header}")"
readonly common_header_sha256
checker_sha256="$(sha256_file "${checker}")"
readonly checker_sha256
runner_sha256="$(sha256_file "${runner_snapshot}")"
readonly runner_sha256
host_guard_sha256="$(sha256_file "${host_guard}")"
readonly host_guard_sha256
system_identity_collector_sha256="$(sha256_file "${system_identity_collector}")"
readonly system_identity_collector_sha256

print_context() {
  local slot="$1"
  local backend_order="$2"
  printf 'context schema=fe2o3.r26-inplace-benchmark.v1 git_commit=%s target=gfx942:xnack- gpu_index=%s unique_id=%s uuid=%s bytes=%s elements=%s workgroup=%s warmups=%s samples=%s iterations_per_sample=%s kernel=inplace_transform max_busy_percent=%s phase_timeout_seconds=%s rocm_version=%s rustc=%s cargo=%s hipcc=%s cxx=%s hsaco_sha256=%s kernel_source_sha256=%s kernel_policy_sha256=%s fixture_recipe_sha256=%s fixture_producer_clang=AMD_clang_version_22.0.0git_(https://github.com/RadeonOpenCompute/llvm-project_roc-7.2.0_26014_7b800a19466229b8479a78de19143dc33c3ab9b5) fixture_rebuild=not-run-on-measurement-host kfd_binary_sha256=%s hsa_binary_sha256=%s hip_binary_sha256=%s hsa_source_sha256=%s hip_source_sha256=%s binary_reader_sha256=%s common_header_sha256=%s checker_sha256=%s runner_sha256=%s host_guard_sha256=%s system_identity_collector_sha256=%s build_environment=%s execution_environment=%s telemetry_command=rocm-smi-showuse-showclocks-showpower placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 interference_monitor=selected-kfd-gpu-process-tree-census-v2 monitor_interval_us=%s monitor_maximum_gap_us=%s topology_sha256=%s counterbalance_design=%s counterbalance_slots=3 counterbalance_slot=%s counterbalance_set_id=%s backend_order=%s\n' \
    "${git_commit}" "${gpu_index}" "${unique_id}" "${uuid}" \
    "${bytes}" "${elements}" "${workgroup}" "${warmups}" "${samples}" \
    "${iterations_per_sample}" "${max_busy}" "${phase_timeout}" \
    "${rocm_version}" "${rustc_version}" "${cargo_version}" \
    "${hipcc_version}" "${cxx_version}" "${hsaco_sha256}" \
    "${kernel_source_sha256}" "${kernel_policy_sha256}" \
    "${fixture_recipe_sha256}" "${kfd_binary_sha256}" "${hsa_binary_sha256}" \
    "${hip_binary_sha256}" "${hsa_source_sha256}" "${hip_source_sha256}" \
    "${binary_reader_sha256}" "${common_header_sha256}" "${checker_sha256}" \
    "${runner_sha256}" \
    "${host_guard_sha256}" "${system_identity_collector_sha256}" \
    "${build_environment}" "${execution_environment}" "${monitor_interval_us}" \
    "${monitor_maximum_gap_us}" "${topology_sha256}" \
    "${counterbalance_design}" "${slot}" "${counterbalance_set_id}" \
    "${backend_order}"
  printf '%s\n' "${system_identity_start}"
}

run_backend() {
  local slot="$1"
  local backend="$2"
  local busy
  local start_topology
  local start_telemetry
  local monitor_record
  local monitor_record_output="${build_dir}/monitor-slot-${slot}-${backend}.out"
  local monitor_status
  local end_telemetry
  local end_topology
  local target_output="${build_dir}/target-slot-${slot}-${backend}.out"
  local -a command
  start_topology="$(capture_topology "${slot}" "${backend}" start)"
  busy="$(require_idle_gpu)"
  start_telemetry="$(capture_telemetry "${slot}" "${backend}" start "${busy}")"
  case "${backend}" in
    kfd)
      command=("${qualification_env[@]}"
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s
        "${phase_timeout}s" "${kfd_binary}" "${unique_id}")
      ;;
    hsa)
      command=("${qualification_env[@]}"
        HSA_XNACK=0 ROCR_VISIBLE_DEVICES="${gpu_index}"
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s"
        "${hsa_binary}" "${hsaco}" 0 "${unique_id}")
      ;;
    hip)
      command=("${qualification_env[@]}"
        HSA_XNACK=0 HIP_VISIBLE_DEVICES="${gpu_index}"
        /usr/bin/taskset --cpu-list "${measurement_cpu_list}"
        /usr/bin/numactl --physcpubind="${measurement_cpu_list}"
        --membind="${topology_numa_node}"
        /usr/bin/timeout --foreground --signal=TERM --kill-after=5s "${phase_timeout}s"
        "${hip_binary}" "${hsaco}" 0 "${unique_id}")
      ;;
    *)
      printf 'unsupported R26 backend: %s\n' "${backend}" >&2
      exit 2
      ;;
  esac
  "${qualification_env[@]}" /usr/bin/python3 "${host_guard}" monitor \
    --gpu-id "${kfd_gpu_id}" \
    --observer-cpu "${observer_cpu}" \
    --target-output "${target_output}" \
    -- "${command[@]}" >"${monitor_record_output}" &
  active_monitor_pid=$!
  if wait "${active_monitor_pid}"; then
    monitor_status=0
  else
    monitor_status=$?
  fi
  active_monitor_pid=""
  ((monitor_status == 0)) || return "${monitor_status}"
  monitor_record="$(
    "${qualification_env[@]}" /usr/bin/cat -- "${monitor_record_output}"
  )"
  [[ "${monitor_record}" == 'monitor schema=fe2o3.r26-kfd-queue-monitor.v2 '* && \
    "${monitor_record}" != *$'\n'* ]] || {
    printf 'R26 host guard emitted a malformed monitor record\n' >&2
    exit 2
  }
  busy="$(require_idle_gpu)"
  end_telemetry="$(capture_telemetry "${slot}" "${backend}" end "${busy}")"
  end_topology="$(capture_topology "${slot}" "${backend}" end)"
  "${qualification_env[@]}" /usr/bin/python3 - "${target_output}" <<'PY'
import pathlib
import sys

data = pathlib.Path(sys.argv[1]).read_bytes()
if not data.endswith(b"\n") or data.count(b"\n") != 1 or b"\0" in data:
    raise SystemExit("R26 target must emit exactly one newline-terminated text row")
PY
  printf '%s\n' "${start_topology}" "${start_telemetry}"
  printf 'monitor slot=%s phase=%s %s\n' \
    "${slot}" "${backend}" "${monitor_record#monitor }"
  printf '%s\n' "${end_telemetry}" "${end_topology}"
  "${qualification_env[@]}" /usr/bin/cat -- "${target_output}"
}

slot_logs=()
for slot in 0 1 2; do
  slot_log="${build_dir}/r26-inplace-slot-${slot}.log"
  slot_logs+=("${slot_log}")
  read -r -a backend_order <<<"${counterbalance_orders[slot]}"
  backend_order_csv="${counterbalance_orders[slot]// /,}"
  {
    print_context "${slot}" "${backend_order_csv}"
    for backend in "${backend_order[@]}"; do
      run_backend "${slot}" "${backend}"
    done
  } >"${slot_log}"
  "${qualification_env[@]}" /usr/bin/cat -- "${slot_log}"
done
readonly -a slot_logs

system_identity_end="$(collect_system_identity end)"
readonly system_identity_end
[[ "${system_identity_end}" == 'context schema=fe2o3.r26-system-identity.v1 '* && \
  "${system_identity_end}" != *$'\n'* ]] || {
  printf 'R26 end system identity collector emitted a malformed record\n' >&2
  exit 2
}
for slot_log in "${slot_logs[@]}"; do
  printf '%s\n' "${system_identity_end}" >>"${slot_log}"
done

readonly set_report="${build_dir}/r26-inplace-set-validation.txt"
"${qualification_env[@]}" /usr/bin/python3 \
  "${checker}" "${slot_logs[@]}" \
  --schema fe2o3.r26-inplace-benchmark.v1 --r26-counterbalance-set |
  "${qualification_env[@]}" /usr/bin/tee "${set_report}"
printf 'context gpu_busy_after_percent=%s\n' "$(require_idle_gpu)"

persist_staging="${output_dir}/.r26-inplace-${counterbalance_set_id}.tmp.$$"
[[ ! -e "${persist_staging}" ]] || {
  printf 'R26 output staging path already exists: %s\n' "${persist_staging}" >&2
  exit 2
}
"${qualification_env[@]}" /usr/bin/mkdir -- "${persist_staging}"
for slot in 0 1 2; do
  "${qualification_env[@]}" /usr/bin/cp -- \
    "${slot_logs[slot]}" "${persist_staging}/slot-${slot}.log"
done
"${qualification_env[@]}" /usr/bin/cp -- \
  "${set_report}" "${persist_staging}/set-validation.txt"
readonly persisted_validation="${build_dir}/persisted-set-validation.txt"
"${qualification_env[@]}" /usr/bin/python3 \
  "${checker}" \
  "${persist_staging}/slot-0.log" \
  "${persist_staging}/slot-1.log" \
  "${persist_staging}/slot-2.log" \
  --schema fe2o3.r26-inplace-benchmark.v1 --r26-counterbalance-set \
  >"${persisted_validation}"
if [[ ! -s "${persist_staging}/set-validation.txt" ]] || \
  ! "${qualification_env[@]}" /usr/bin/cmp --silent -- \
    "${persisted_validation}" "${persist_staging}/set-validation.txt"; then
  printf 'persisted R26 logs/report failed exact revalidation\n' >&2
  exit 2
fi
verify_staged_inputs
[[ "$(sha256_file "${kfd_binary}")" == "${kfd_binary_sha256}" && \
  "$(sha256_file "${hsa_binary}")" == "${hsa_binary_sha256}" && \
  "$(sha256_file "${hip_binary}")" == "${hip_binary_sha256}" ]] || {
  printf 'R26 benchmark binary changed during qualification\n' >&2
  exit 2
}
[[ "$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" rev-parse HEAD
)" == "${git_commit}" && -z "$(
  "${qualification_env[@]}" /usr/bin/git -C "${repo_root}" status \
    --porcelain=v1 --untracked-files=all --ignore-submodules=none
)" ]] || {
  printf 'R26 checkout changed during qualification\n' >&2
  exit 2
}
"${qualification_env[@]}" /usr/bin/mv -T -- \
  "${persist_staging}" "${artifact_dir}"
persist_staging=""
printf 'R26 evidence set: %s\n' "${artifact_dir}"
