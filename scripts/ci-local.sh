#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly LOG_DIR="${CI_LOG_DIR:-${REPO_ROOT}/target/ci-logs}"
readonly RUSTC_CODEGEN_TEST_PACKAGE="rustc-codegen-fe2o3"

readonly CPU_TEST_PACKAGES=(
  cargo-fe2o3
  dialect-amdgcn
  dialect-mir
  fe2o3-amd-target
  fe2o3-artifact-transaction
  fe2o3-completion
  fe2o3-artifacts
  fe2o3-contracts
  fe2o3-device
  fe2o3-differential
  fe2o3-hsaco
  fe2o3-hsaco-finalize
  fe2o3-host
  fe2o3-kernel-analysis
  fe2o3-kernel-descriptor
  fe2o3-kernel-ir
  fe2o3-macros
  fe2o3-rustc-front
  fe2o3-rustc-invocation
  fe2o3-verifier
  fe2o3-worker-v2-bundle
  reserved-fe2o3-symbols
)

usage() {
  cat <<'EOF'
Usage: scripts/ci-local.sh <command>

Commands:
  generic         Run all validation suitable for a machine without ROCm/GPU
  format          Check Rust formatting
  check           Check every workspace target, including example binaries
  test            Run unit tests that do not link or load the HIP runtime
  workspace-test  Run every workspace test target; may require ROCm libraries
  backend         Build the rustc codegen backend dylib
  parity-evidence Run parity, signed-attestation, and queue shell tests
  verus           Run positive and negative Verus proof fixtures; requires Verus
  rocm-compile    Compile every example to host code and HSACO; requires ROCm
  hardware-smoke  Build and run every example; requires an AMD GPU and opt-in
EOF
}

run_step() {
  local name="$1"
  shift
  local log_file="${LOG_DIR}/${name}.log"

  printf '\n==> %s\n' "${name}"
  printf '   command:'
  printf ' %q' "$@"
  printf '\n   log: %s\n' "${log_file}"

  set +e
  "$@" 2>&1 | tee "${log_file}"
  local status=${PIPESTATUS[0]}
  set -e

  if ((status != 0)); then
    printf 'step %s failed with status %d\n' "${name}" "${status}" >&2
    return "${status}"
  fi
}

load_example_packages() {
  local lane="$1"
  local destination_name="$2"
  local output
  local -n destination="${destination_name}"

  output="$(
    cargo run --quiet --locked -p cargo-fe2o3 -- examples list "${lane}"
  )"
  destination=()
  if [[ -n "${output}" ]]; then
    mapfile -t destination <<<"${output}"
  fi
}

run_format() {
  run_step format cargo fmt --all -- --check
}

run_check() {
  local -a all_examples rustc_examples cargo_args
  local -A rustc_example_set=()
  local package
  load_example_packages all all_examples
  load_example_packages rustc-check rustc_examples
  for package in "${rustc_examples[@]}"; do
    rustc_example_set["${package}"]=1
  done

  cargo_args=(check --workspace --all-targets --locked)
  for package in "${all_examples[@]}"; do
    if [[ -z "${rustc_example_set[${package}]+selected}" ]]; then
      cargo_args+=(--exclude "${package}")
    fi
  done

  # `cargo check` does not link libamdhip64, so all host-facing examples are safe
  # to validate on a generic runner.
  run_step workspace-check cargo "${cargo_args[@]}"
}

run_tests() {
  local cargo_args=(test --locked)
  local -a rustc_examples rocm_examples
  local -A rocm_example_set=()
  local package
  for package in "${CPU_TEST_PACKAGES[@]}"; do
    cargo_args+=(-p "${package}")
  done
  load_example_packages rustc-check rustc_examples
  load_example_packages rocm-compile rocm_examples
  for package in "${rocm_examples[@]}"; do
    rocm_example_set["${package}"]=1
  done
  for package in "${rustc_examples[@]}"; do
    if [[ -z "${rocm_example_set[${package}]+selected}" ]]; then
      cargo_args+=(-p "${package}")
    fi
  done
  # Keep the generic test lane independent of whether the host happens to have
  # ROCm installed. The raw HIP crate supplies a fail-closed no-runtime ABI.
  run_step cpu-tests env FE2O3_HIP_SYS_DISABLE=1 cargo "${cargo_args[@]}"
  run_rustc_codegen_tests
  # fe2o3-core unit tests link HIP, but its compile-fail doctests do not.
  run_step core-doc-tests cargo test --locked --doc -p fe2o3-core
  run_step device-copy-renamed-dependency \
    cargo check --locked -p device-copy-renamed-dependency
  run_step device-copy-derive-real-trait \
    cargo check --locked -p fe2o3-core --test device_copy_derive_compile
  run_step device-copy-derive-ui \
    cargo test --locked -p fe2o3-core --test device_copy_derive_ui
}

run_rustc_codegen_tests() {
  # This package emits both an rlib and an unversioned dylib. Keep its tests in
  # one Cargo process so another workspace build cannot replace that dylib.
  run_step rustc-codegen-tests \
    cargo test --locked -p "${RUSTC_CODEGEN_TEST_PACKAGE}" --all-targets
}

run_workspace_tests() {
  run_step workspace-tests \
    cargo test --locked --workspace --all-targets \
      --exclude "${RUSTC_CODEGEN_TEST_PACKAGE}"
  run_rustc_codegen_tests
}

run_backend_build() {
  run_step backend-build cargo build --locked -p rustc-codegen-fe2o3
}

run_verus() {
  run_step verus-fixtures \
    "${REPO_ROOT}/examples/verus_vecadd/run-verus.sh" --require
}

run_parity_matrix_checks() {
  run_step parity-matrix-check bash scripts/parity-matrix.sh check
  run_step parity-matrix-tests bash scripts/tests/parity-matrix.sh
  run_step parity-evidence-tests bash scripts/tests/parity-evidence.sh
  run_step parity-row-evidence-tests \
    bash scripts/tests/parity-row-evidence.sh
  run_step mi300x-evidence-queue-tests \
    bash scripts/tests/mi300x-evidence-queue.sh
  run_step hosted-parity-ci-tests \
    bash scripts/tests/hosted-parity-ci.sh
}

run_generic() {
  run_step example-manifest \
    cargo run --quiet --locked -p cargo-fe2o3 -- examples check
  run_parity_matrix_checks
  run_format
  run_check
  run_backend_build
  run_step ci-local-test-gate bash scripts/tests/ci-local-test-gate.sh
  run_tests
}

run_rocm_compile() {
  export FE2O3_TARGET="${FE2O3_TARGET:-gfx1100}"
  local -a example_packages
  load_example_packages rocm-compile example_packages
  run_step rocm-doctor cargo run --locked -p cargo-fe2o3 -- doctor
  run_step rocm-trusted-device-items \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test trusted_device_items \
      genuine_markers_emit_and_local_external_spoofs_fail_closed -- \
      --ignored --exact
  run_step rocm-trusted-device-item-stale-cleanup \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test trusted_device_items \
      rejected_lookalikes_remove_preseeded_artifacts_atomically -- \
      --ignored --exact
  run_step rocm-cross-crate-typed-binding \
    env FE2O3_TEST_TARGET="${FE2O3_TARGET}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --test cross_crate_typed_binding \
      same_logical_name_in_two_rlibs_resolves_distinct_artifacts -- \
      --ignored --exact
  run_step rocm-g1-code-object \
    cargo test --locked -p dialect-amdgcn --test lowering \
      rocm_compiles_the_golden_to_an_amdgpu_code_object -- \
      --ignored --exact
  run_step rocm-kernel-ir-codegen-rejection \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test kernel_ir_codegen \
      selected_pipeline_rejects_invalid_or_unsupported_inputs_and_cleans_stale_artifacts -- \
      --ignored --exact
  run_step rocm-kernel-ir-vecadd \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test kernel_ir_codegen \
      opt_in_vecadd_publishes_exact_g1_without_gpu -- \
      --ignored --exact

  local package
  for package in "${example_packages[@]}"; do
    run_step "rocm-build-${package}" \
      cargo run --locked -p cargo-fe2o3 -- build -p "${package}"
    run_step "rocm-artifacts-${package}" \
      cargo run --quiet --locked -p cargo-fe2o3 -- \
        examples check-artifacts "${package}"
  done
  run_step rocm-kernel-ir-verification \
    cargo test --locked -p cargo-fe2o3 --test kernel_ir_verification \
      verification_gate_accepts_rejects_and_remains_opt_in -- --ignored --exact
}

require_gpu_access() {
  local kfd_path="${1:-/dev/kfd}"
  local dxg_path="${2:-/dev/dxg}"
  local device_node

  if [[ -e "${kfd_path}" ]]; then
    device_node="${kfd_path}"
  elif [[ -e "${dxg_path}" ]]; then
    if [[ "${HSA_ENABLE_DXG_DETECTION:-}" != "1" ]]; then
      printf '%s\n' \
        'WSL GPU smoke requires HSA_ENABLE_DXG_DETECTION=1' >&2
      return 2
    fi
    device_node="${dxg_path}"
  else
    printf '%s\n' \
      'GPU smoke requires /dev/kfd (native Linux) or /dev/dxg (WSL)' >&2
    return 2
  fi

  if [[ ! -r "${device_node}" || ! -w "${device_node}" ]]; then
    printf 'GPU smoke requires read/write access to %s\n' \
      "${device_node}" >&2
    return 2
  fi
}

wavefront_for_target() {
  local processor="${1%%:*}"
  case "${processor}" in
    gfx9*) printf '%s\n' 64 ;;
    gfx*) printf '%s\n' 32 ;;
    *)
      printf 'cannot derive wavefront size for FE2O3_TARGET=%s\n' "$1" >&2
      return 2
      ;;
  esac
}

run_hardware_smoke() {
  if [[ "${FE2O3_ALLOW_GPU_SMOKE:-}" != "1" ]]; then
    printf '%s\n' \
      'refusing to run GPU smoke without FE2O3_ALLOW_GPU_SMOKE=1' >&2
    return 2
  fi
  if [[ -z "${FE2O3_TARGET:-}" ]]; then
    printf '%s\n' \
      'hardware HSACO inspection requires an explicit FE2O3_TARGET' >&2
    return 2
  fi
  require_gpu_access
  if ! command -v rocminfo >/dev/null 2>&1; then
    printf '%s\n' 'GPU smoke requires rocminfo on PATH' >&2
    return 2
  fi

  run_step hardware-rocminfo rocminfo
  run_step hardware-doctor cargo run --locked -p cargo-fe2o3 -- doctor
  local rocm_path="${ROCM_PATH:-/opt/rocm}"
  local native_test="${REPO_ROOT}/target/fe2o3-hip-device-properties-test"
  run_step hardware-hip-device-properties-build \
    "${CC:-cc}" -std=c11 -Wall -Wextra -Werror -D__HIP_PLATFORM_AMD__ \
      -I "${rocm_path}/include" -I "${REPO_ROOT}/crates/fe2o3-hip-sys/native" \
      "${REPO_ROOT}/crates/fe2o3-hip-sys/native/device_properties_test.c" \
      -L "${rocm_path}/lib" -Wl,-rpath,"${rocm_path}/lib" -lamdhip64 \
      -o "${native_test}"
  run_step hardware-hip-device-properties-test "${native_test}"
  run_step hardware-observed-device-target \
    cargo test --locked -p fe2o3-core --lib \
      device_target::tests::context_observes_a_real_hip_device -- \
      --ignored --exact
  run_step hardware-device-copy-transfer \
    cargo test --locked -p fe2o3-core --test device_copy_derive_hardware -- \
      --ignored --exact derived_struct_bytes_round_trip_through_device_memory
  run_step hardware-kernel-ir-fill \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test kernel_ir_codegen \
      opt_in_fill_publishes_g1_and_executes_on_the_gpu -- \
      --ignored --exact
  run_step hardware-kernel-ir-vecadd \
    cargo test --locked -p rustc-codegen-fe2o3 \
      --test kernel_ir_codegen \
      opt_in_vecadd_publishes_exact_g1_and_executes_on_the_gpu -- \
      --ignored --exact
  run_step hardware-smoke cargo run --locked -p cargo-fe2o3 -- smoke
  local test_wavefront
  test_wavefront="$(wavefront_for_target "${FE2O3_TARGET}")"
  run_step hardware-hsaco-inspection env \
    FE2O3_TEST_HSACO="${REPO_ROOT}/target/fe2o3/vecadd.hsaco" \
    FE2O3_TEST_TARGET="${FE2O3_TARGET}" \
    FE2O3_TEST_WAVEFRONT="${test_wavefront}" \
    cargo test --locked -p fe2o3-hsaco --test inspection \
      inspects_real_generated_vecadd_hsaco -- --ignored --exact
}

main() {
  cd "${REPO_ROOT}"
  mkdir -p "${LOG_DIR}"

  case "${1:-}" in
    generic) run_generic ;;
    format) run_format ;;
    check) run_check ;;
    test) run_tests ;;
    workspace-test) run_workspace_tests ;;
    backend) run_backend_build ;;
    parity-evidence) run_parity_matrix_checks ;;
    verus) run_verus ;;
    rocm-compile) run_rocm_compile ;;
    hardware-smoke) run_hardware_smoke ;;
    -h | --help | help) usage ;;
    *)
      usage >&2
      return 2
      ;;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
