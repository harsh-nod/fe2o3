#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly LOG_DIR="${CI_LOG_DIR:-${REPO_ROOT}/target/ci-logs}"

readonly CPU_TEST_PACKAGES=(
  cargo-fe2o3
  dialect-amdgcn
  dialect-mir
  fe2o3-amd-target
  fe2o3-completion
  fe2o3-artifacts
  fe2o3-contracts
  fe2o3-device
  fe2o3-kernel-ir
  fe2o3-macros
  reserved-fe2o3-symbols
  rustc-codegen-fe2o3
  verus-vecadd
)

readonly EXAMPLE_PACKAGES=(
  fe2o3-vecadd
  fe2o3-add-inplace
  fe2o3-copy
  fe2o3-downsample
  fe2o3-fill
  fe2o3-gather-odd
  fe2o3-scale
  fe2o3-shift
  fe2o3-previous
  fe2o3-stencil
  fe2o3-raw-add-index
  fe2o3-raw-const-minus
  fe2o3-raw-parenthesized-sub
  fe2o3-raw-disjoint-inplace-shift
  fe2o3-raw-disjoint-shift
  fe2o3-raw-gather
  fe2o3-raw-neighbors
  fe2o3-raw-output-shift
  fe2o3-saxpy
  fe2o3-axpy-inplace
  fe2o3-negate
  fe2o3-normalize
  fe2o3-pipeline
  fe2o3-vecadd-f64
)

usage() {
  cat <<'EOF'
Usage: scripts/ci-local.sh <command>

Commands:
  generic         Run all validation suitable for a machine without ROCm/GPU
  format          Check Rust formatting
  check           Check every workspace target, including example binaries
  test            Run unit tests that do not link or load the HIP runtime
  backend         Build the rustc codegen backend dylib
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

run_format() {
  run_step format cargo fmt --all -- --check
}

run_check() {
  # `cargo check` does not link libamdhip64, so all host-facing examples are safe
  # to validate on a generic runner.
  run_step workspace-check cargo check --workspace --all-targets --locked
}

run_tests() {
  local cargo_args=(test --locked)
  local package
  for package in "${CPU_TEST_PACKAGES[@]}"; do
    cargo_args+=(-p "${package}")
  done
  run_step cpu-tests cargo "${cargo_args[@]}"
  # fe2o3-core unit tests link HIP, but its compile-fail doctests do not.
  run_step core-doc-tests cargo test --locked --doc -p fe2o3-core
  run_step device-copy-renamed-dependency \
    cargo check --locked -p device-copy-renamed-dependency
  run_step device-copy-derive-real-trait \
    cargo check --locked -p fe2o3-core --test device_copy_derive_compile
  run_step device-copy-derive-ui \
    cargo test --locked -p fe2o3-core --test device_copy_derive_ui
}

run_backend_build() {
  run_step backend-build cargo build --locked -p rustc-codegen-fe2o3
}

run_generic() {
  run_format
  run_check
  run_backend_build
  run_tests
}

run_rocm_compile() {
  export FE2O3_TARGET="${FE2O3_TARGET:-gfx1100}"
  run_step rocm-doctor cargo run --locked -p cargo-fe2o3 -- doctor

  local package
  for package in "${EXAMPLE_PACKAGES[@]}"; do
    run_step "rocm-build-${package}" \
      cargo run --locked -p cargo-fe2o3 -- build -p "${package}"
  done
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

run_hardware_smoke() {
  if [[ "${FE2O3_ALLOW_GPU_SMOKE:-}" != "1" ]]; then
    printf '%s\n' \
      'refusing to run GPU smoke without FE2O3_ALLOW_GPU_SMOKE=1' >&2
    return 2
  fi
  require_gpu_access
  if ! command -v rocminfo >/dev/null 2>&1; then
    printf '%s\n' 'GPU smoke requires rocminfo on PATH' >&2
    return 2
  fi

  run_step hardware-rocminfo rocminfo
  run_step hardware-doctor cargo run --locked -p cargo-fe2o3 -- doctor
  run_step hardware-device-copy-transfer \
    cargo test --locked -p fe2o3-core --test device_copy_derive_hardware -- \
      --ignored --exact derived_struct_bytes_round_trip_through_device_memory
  run_step hardware-smoke cargo run --locked -p cargo-fe2o3 -- smoke
}

main() {
  cd "${REPO_ROOT}"
  mkdir -p "${LOG_DIR}"

  case "${1:-}" in
    generic) run_generic ;;
    format) run_format ;;
    check) run_check ;;
    test) run_tests ;;
    backend) run_backend_build ;;
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
