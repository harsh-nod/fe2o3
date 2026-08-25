#!/usr/bin/env bash

set -Eeuo pipefail
umask 077
export PYTHONDONTWRITEBYTECODE=1

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
DEFAULT_CARGO_TARGET_ROOT="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
if [[ "${DEFAULT_CARGO_TARGET_ROOT}" != /* ]]; then
  DEFAULT_CARGO_TARGET_ROOT="${REPO_ROOT}/${DEFAULT_CARGO_TARGET_ROOT}"
fi
DEFAULT_CARGO_TARGET_ROOT="$(realpath --canonicalize-missing -- "${DEFAULT_CARGO_TARGET_ROOT}")"
readonly DEFAULT_CARGO_TARGET_ROOT
LOG_DIR="${CI_LOG_DIR:-${DEFAULT_CARGO_TARGET_ROOT}/ci-logs}"
if [[ "${LOG_DIR}" != /* ]]; then
  LOG_DIR="${REPO_ROOT}/${LOG_DIR}"
fi
LOG_DIR="$(realpath --canonicalize-missing -- "${LOG_DIR}")"
readonly LOG_DIR
readonly RUSTC_CODEGEN_TEST_PACKAGE="rustc-codegen-fe2o3"
readonly RUSTC_CODEGEN_TEST_DRIVER_PACKAGE="cargo-fe2o3"
readonly RUSTC_CODEGEN_QUALIFICATION_FEATURE="qualification-oracles-test-only"
readonly CARGO_FE2O3_QUALIFICATION_FEATURE="qualification-oracles-test-only"
readonly RUSTC_CODEGEN_SHARD_POLICY="${REPO_ROOT}/scripts/rustc-codegen-shards.py"
readonly WORKSPACE_DEPENDENCY_POLICY_CHECKER="${REPO_ROOT}/scripts/workspace_dependency_policy.py"
readonly WORKSPACE_DEPENDENCY_POLICY="${REPO_ROOT}/scripts/workspace-dependency-policy.json"
readonly WORKSPACE_DEPENDENCY_POLICY_TESTS="${REPO_ROOT}/scripts/tests/workspace_dependency_policy.py"
readonly PLIRON_DEPENDENCY_POLICY_CHECKER="${REPO_ROOT}/scripts/pliron_dependency_policy.py"
readonly PLIRON_DEPENDENCY_POLICY_TESTS="${REPO_ROOT}/scripts/tests/pliron_dependency_policy.py"
readonly STANDALONE_LOCKFILE_CHECKER="${REPO_ROOT}/scripts/check-standalone-lockfiles.sh"
readonly RUNTIME_PURE_RUST_AUDITOR="${REPO_ROOT}/scripts/runtime_pure_rust_audit.py"
readonly RUNTIME_PURE_RUST_POLICY="${REPO_ROOT}/scripts/runtime-pure-rust-policy.json"
readonly RUNTIME_PURE_RUST_AUDIT_TESTS="${REPO_ROOT}/scripts/tests/runtime_pure_rust_audit.py"
readonly RUNTIME_IDENTITY_ORACLE_TESTS="${REPO_ROOT}/scripts/tests/runtime_identity_oracle.py"
readonly RUNTIME_IDENTITY_ORACLE="${REPO_ROOT}/scripts/runtime-identity-oracle.sh"
readonly RUNTIME_PURE_RUST_TARGET_DIR="${DEFAULT_CARGO_TARGET_ROOT}/runtime-pure-rust-policy"
readonly CI_STEP_TIMEOUT_SECONDS="${FE2O3_CI_STEP_TIMEOUT_SECONDS:-3000}"
readonly GENERAL_GEMM_SEMANTIC_FRONTEND_TIMEOUT_SECONDS=4200
readonly COLLECTED_CONTROL_FLOW_TIMEOUT_SECONDS=4200
readonly COLLECTED_CONTROL_FLOW_TEST_THREADS=1
readonly CI_STEP_KILL_AFTER_SECONDS="${FE2O3_CI_STEP_KILL_AFTER_SECONDS:-15}"
readonly TEST_DRIVER_BINARY_ENV="FE2O3_TEST_CARGO_FE2O3_BIN"
readonly TEST_DRIVER_SHA256_ENV="FE2O3_TEST_CARGO_FE2O3_SHA256"
CARGO_FE2O3_BINARY=
CARGO_FE2O3_SHA256=
CARGO_FE2O3_DRIVER_ROOT=
CARGO_FE2O3_DRIVER_PROFILE=
CARGO_TARGET_DIRECTORY=
CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY=
CI_PRIVATE_TMP_ROOT=
readonly -a ROCM_TRUSTED_DEVICE_ITEM_PACKAGES=(
  fe2o3-vecadd
  fe2o3-trusted-item-renamed-genuine
  fe2o3-trusted-item-lookalike-type
  fe2o3-trusted-item-lookalike-helper
  fe2o3-trusted-item-lookalike-thread
  fe2o3-trusted-item-external-spoof
  fe2o3-trusted-item-local-marker
  fe2o3-typed-alias-spoof
)
# This proof fixture is also compiled outside the managed wrapper and carries a
# source-identity-pinned fallback namespace. Every other gate source must use
# the compiler-derived binding.
readonly -a ROCM_EXPLICIT_NAMESPACE_FALLBACK_PACKAGES=(
  fe2o3-typed-alias-spoof
)
readonly -a ROCM_QUALIFICATION_TEST_PACKAGES=(
  fe2o3-host
  fe2o3-pliron-scalar-add-v1
)
readonly CPU_TEST_PACKAGES=(
  dialect-amdgcn
  dialect-autotune
  dialect-dispatch
  dialect-gpu
  dialect-kernel
  dialect-mir
  dialect-proof
  dialect-schedule
  dialect-tile
  fe2o3-amd-target
  fe2o3-amdgcn-model
  fe2o3-amdhsa-loader
  fe2o3-aql
  fe2o3-artifact-transaction
  fe2o3-completion
  fe2o3-compiler-api
  fe2o3-compiler-driver
  fe2o3-artifacts
  fe2o3-contracts
  fe2o3-device
  fe2o3-differential
  fe2o3-drm-uapi
  fe2o3-hsaco
  fe2o3-hsaco-finalize
  fe2o3-host
  fe2o3-host-api
  fe2o3-kfd
  fe2o3-kfd-uapi
  fe2o3-kernel-analysis
  fe2o3-kernel-descriptor
  fe2o3-kernel-ir
  fe2o3-kir-sim
  fe2o3-kir-sim-cli
  fe2o3-kir-sim-trace
  fe2o3-kir-pliron-bridge
  fe2o3-lower-kernel-gpu
  fe2o3-lower-mir-kernel
  fe2o3-macros
  fe2o3-mir-model
  fe2o3-pliron
  fe2o3-pliron-conformance
  fe2o3-pliron-scalar-add-v1
  fe2o3-proof-contracts
  fe2o3-rustc-front
  fe2o3-rustc-invocation
  fe2o3-service-host
  fe2o3-service-model
  fe2o3-semantic-import
  fe2o3-semantic-query
  fe2o3-semantic-trace
  fe2o3-runtime-model
  fe2o3-verifier
  fe2o3-worker-v2-bundle
  reserved-fe2o3-symbols
)

usage() {
  cat <<'EOF'
Usage: scripts/ci-local.sh <command>

Commands:
  generic         Run all validation suitable for a machine without ROCm/GPU
  generic-core    Run generic validation except codegen integration shards
  workspace-policy  Validate workspace ownership and dependency directions
  standalone-locks  Validate every tracked standalone Cargo lockfile
  runtime-policy  Validate the pure-Rust runtime dependency and ELF auditor
  runtime-identity-oracle  Measure MI300X identity against isolated rocminfo; explicit opt-in
  shard-policy    Validate the codegen integration shard assignment
  rustc-codegen-shard <id>  Run one codegen integration shard
  format          Check Rust formatting
  check           Check every workspace target, including example binaries
  test            Run unit tests that do not link or load the HIP runtime
  workspace-test  Run every workspace test target; may require ROCm libraries
  rustc-codegen-test  Run backend library and integration tests without dylib replacement
  backend         Build the rustc codegen backend dylib
  authority-launcher  Run bounded protected build-authority launcher tests
  rustc-trampoline    Run non-integrated static rustc trampoline tests
  parity-evidence Run parity, signed-attestation, and queue shell tests
  parity-production-immutable  Run opt-in root ext4/XFS ingestion test
  verus           Run positive and negative Verus proof fixtures; requires Verus
  rocm-compile    Compile every example to host code and HSACO; requires ROCm
  hardware-smoke  Build and run every example; requires an AMD GPU and opt-in
  s09-debug-hardware  Run exact gfx942 direct-link and ROCgdb evidence; explicit opt-in
EOF
}

run_step_with_timeout() {
  local timeout_seconds="$1"
  local name="$2"
  shift 2
  local log_file="${LOG_DIR}/${name}.log"

  if [[ ! "${timeout_seconds}" =~ ^[1-9][0-9]*$ ]] ||
    ((timeout_seconds >= 7200)); then
    printf 'step timeout must be an integer from 1 through 7199: %s\n' \
      "${timeout_seconds}" >&2
    return 2
  fi
  if [[ ! "${CI_STEP_KILL_AFTER_SECONDS}" =~ ^[1-9][0-9]*$ ]] ||
    ((CI_STEP_KILL_AFTER_SECONDS > 300)); then
    printf '%s\n' \
      'FE2O3_CI_STEP_KILL_AFTER_SECONDS must be an integer from 1 through 300' >&2
    return 2
  fi
  if ! command -v timeout >/dev/null 2>&1; then
    printf '%s\n' 'ci-local requires GNU timeout to supervise each step' >&2
    return 2
  fi

  printf '\n==> %s\n' "${name}"
  printf '   command:'
  printf ' %q' "$@"
  printf '\n   timeout: %ss' "${timeout_seconds}"
  printf '\n   log: %s\n' "${log_file}"

  set +e
  timeout --signal=TERM --kill-after="${CI_STEP_KILL_AFTER_SECONDS}s" \
    "${timeout_seconds}s" "$@" 2>&1 | tee "${log_file}"
  local -a pipeline_status=("${PIPESTATUS[@]}")
  local command_status="${pipeline_status[0]}"
  local tee_status="${pipeline_status[1]}"
  local status
  set -e

  if ((command_status != 0)); then
    status="${command_status}"
  else
    status="${tee_status}"
  fi
  if ((tee_status != 0)); then
    printf 'step %s log write failed with status %d\n' \
      "${name}" "${tee_status}" >&2
  fi
  if ((status != 0)); then
    if ((command_status == 124)); then
      printf 'step %s timed out after %s seconds\n' \
        "${name}" "${timeout_seconds}" >&2
    fi
    printf 'step %s failed with status %d\n' "${name}" "${status}" >&2
    return "${status}"
  fi
}

run_step() {
  if [[ ! "${CI_STEP_TIMEOUT_SECONDS}" =~ ^[1-9][0-9]*$ ]] ||
    ((CI_STEP_TIMEOUT_SECONDS >= 3600)); then
    printf '%s\n' \
      'FE2O3_CI_STEP_TIMEOUT_SECONDS must be an integer from 1 through 3599' >&2
    return 2
  fi
  run_step_with_timeout "${CI_STEP_TIMEOUT_SECONDS}" "$@"
}

validate_private_directory() {
  local label="$1"
  local directory="$2"
  local canonical mode owner

  [[ "${directory}" == /* && -d "${directory}" && ! -L "${directory}" ]] || {
    printf '%s must be an absolute non-symlink directory: %s\n' \
      "${label}" "${directory}" >&2
    return 2
  }
  canonical="$(realpath --canonicalize-existing -- "${directory}")"
  mode="$(stat -c '%a' -- "${directory}")"
  owner="$(stat -c '%u' -- "${directory}")"
  if [[ "${canonical}" != "${directory}" ]] ||
    ((8#${mode} & 8#077)) || [[ "${owner}" != "$(id -u)" ]]; then
    printf '%s must be canonical, owner-held, and private: %s\n' \
      "${label}" "${directory}" >&2
    return 2
  fi
}

resolve_cargo_target_directory() {
  local target_directory canonical
  target_directory="$(
    cargo metadata --locked --no-deps --format-version 1 |
      python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
  )"
  if [[ "${target_directory}" != /* ]] ||
    [[ "${target_directory}" == *$'\n'* ]]; then
    printf 'Cargo reported an invalid target directory: %q\n' \
      "${target_directory}" >&2
    return 2
  fi
  [[ -d "${target_directory}" && ! -L "${target_directory}" ]] || {
    printf 'Cargo target directory is not a real directory: %s\n' \
      "${target_directory}" >&2
    return 2
  }
  canonical="$(realpath --canonicalize-existing -- "${target_directory}")"
  if [[ "${canonical}" != "${target_directory}" ]]; then
    printf 'Cargo target directory must already be canonical: %s\n' \
      "${target_directory}" >&2
    return 2
  fi
  validate_private_directory 'Cargo target directory' "${canonical}"
  printf '%s\n' "${canonical}"
}

validate_cargo_fe2o3_driver() {
  local canonical mode owner digest parent parent_mode parent_owner

  [[ -n "${CARGO_FE2O3_BINARY}" && -n "${CARGO_FE2O3_SHA256}" ]] || {
    printf '%s\n' 'cargo-fe2o3 qualification driver is not prepared' >&2
    return 2
  }
  [[ "${CARGO_FE2O3_BINARY}" == /* && -f "${CARGO_FE2O3_BINARY}" &&
    ! -L "${CARGO_FE2O3_BINARY}" && -x "${CARGO_FE2O3_BINARY}" ]] || {
    printf 'cargo-fe2o3 qualification driver is not an absolute executable file: %s\n' \
      "${CARGO_FE2O3_BINARY}" >&2
    return 2
  }
  canonical="$(realpath --canonicalize-existing -- "${CARGO_FE2O3_BINARY}")"
  mode="$(stat -c '%a' -- "${CARGO_FE2O3_BINARY}")"
  owner="$(stat -c '%u' -- "${CARGO_FE2O3_BINARY}")"
  parent="$(dirname -- "${CARGO_FE2O3_BINARY}")"
  parent_mode="$(stat -c '%a' -- "${parent}")"
  parent_owner="$(stat -c '%u' -- "${parent}")"
  digest="$(sha256sum -- "${CARGO_FE2O3_BINARY}")"
  digest="${digest%% *}"
  if [[ "${canonical}" != "${CARGO_FE2O3_BINARY}" ]] ||
    [[ "${mode}" != 500 ]] || [[ "${owner}" != "$(id -u)" ]] ||
    [[ "${parent_mode}" != 500 ]] || [[ "${parent_owner}" != "$(id -u)" ]] ||
    [[ "${digest}" != "${CARGO_FE2O3_SHA256}" ]]; then
    printf '%s\n' \
      'cargo-fe2o3 qualification driver identity or private custody changed' >&2
    return 2
  fi
}

prepare_cargo_fe2o3_qualification_target() {
  validate_private_directory 'Cargo target directory' "${CARGO_TARGET_DIRECTORY}"
  CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY="${CARGO_TARGET_DIRECTORY}/fe2o3-qualification-tests-${BASHPID}"
  [[ ! -e "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" &&
    ! -L "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" ]] || {
    printf 'qualification test target already exists: %s\n' \
      "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" >&2
    return 2
  }
  mkdir -m 700 -- "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}"
  validate_private_directory 'Qualification test target' \
    "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}"
}

resolve_cargo_fe2o3_artifact() {
  python3 - "$@" <<'PY'
import json
import os
import sys

receipt, expected_package, expected_source, expected_target = sys.argv[1:]
paths = []
with open(receipt, "rb") as records:
    for line in records:
        record = json.loads(line)
        if record.get("reason") != "compiler-artifact" or record.get("target", {}).get("name") != "cargo-fe2o3":
            continue
        target = record.get("target", {})
        profile = record.get("profile", {})
        if (
            record.get("package_id") != expected_package
            or target.get("kind") != ["bin"]
            or target.get("crate_types") != ["bin"]
            or target.get("src_path") != expected_source
            or profile.get("test") is not False
            or profile.get("opt_level") != "0"
        ):
            raise SystemExit("Cargo reported a cargo-fe2o3 artifact with mismatched package, source, target, or profile identity")
        executable = record.get("executable")
        if not executable or not os.path.isabs(executable):
            raise SystemExit("Cargo reported cargo-fe2o3 without an absolute executable")
        canonical = os.path.realpath(executable)
        if canonical != executable or os.path.commonpath([expected_target, canonical]) != expected_target:
            raise SystemExit("Cargo reported cargo-fe2o3 outside the admitted target")
        paths.append(canonical)
if len(paths) != 1:
    raise SystemExit(
        f"expected exactly one cargo-fe2o3 executable artifact; found {len(paths)}"
    )
print(paths[0])
PY
}

prepare_private_tmp_root() {
  if [[ -n "${TMPDIR:-}" ]]; then
    validate_private_directory TMPDIR "${TMPDIR}"
    return
  fi
  validate_private_directory 'Cargo target directory' "${CARGO_TARGET_DIRECTORY}"
  CI_PRIVATE_TMP_ROOT="${CARGO_TARGET_DIRECTORY}/fe2o3-ci-tmp-${BASHPID}"
  [[ ! -e "${CI_PRIVATE_TMP_ROOT}" && ! -L "${CI_PRIVATE_TMP_ROOT}" ]] || {
    printf 'private CI temporary root already exists: %s\n' \
      "${CI_PRIVATE_TMP_ROOT}" >&2
    return 2
  }
  mkdir -m 700 -- "${CI_PRIVATE_TMP_ROOT}"
  [[ "$(realpath --canonicalize-existing -- "${CI_PRIVATE_TMP_ROOT}")" == \
    "${CI_PRIVATE_TMP_ROOT}" ]] || {
    printf 'private CI temporary root is not canonical: %s\n' \
      "${CI_PRIVATE_TMP_ROOT}" >&2
    return 2
  }
  TMPDIR="${CI_PRIVATE_TMP_ROOT}"
  export TMPDIR
  validate_private_directory TMPDIR "${TMPDIR}"
}

prepare_cargo_fe2o3_driver() {
  local step_prefix="$1"
  local driver_profile="$2"
  local metadata_receipt receipt built_binary built_sha256
  local -a driver_identity feature_args=()

  case "${driver_profile}" in
    production) ;;
    qualification)
      feature_args=(--features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}")
      ;;
    *)
      printf 'unknown cargo-fe2o3 driver profile: %s\n' \
        "${driver_profile}" >&2
      return 2
      ;;
  esac
  CARGO_FE2O3_DRIVER_PROFILE=

  CARGO_TARGET_DIRECTORY="$(resolve_cargo_target_directory)"
  prepare_private_tmp_root
  metadata_receipt="$(mktemp -- "${TMPDIR}/cargo-fe2o3-metadata.XXXXXX.json")"
  cargo metadata --locked --no-deps --format-version 1 >"${metadata_receipt}"
  mapfile -t driver_identity < <(
    python3 - "${metadata_receipt}" "${REPO_ROOT}" "${CARGO_TARGET_DIRECTORY}" <<'PY'
import json
import os
import sys

workspace = os.path.realpath(sys.argv[2])
expected_target = sys.argv[3]
with open(sys.argv[1], "rb") as source:
    metadata = json.load(source)
if metadata.get("target_directory") != expected_target:
    raise SystemExit("Cargo metadata target_directory changed during driver bootstrap")
packages = [package for package in metadata.get("packages", []) if package.get("name") == "cargo-fe2o3"]
if len(packages) != 1:
    raise SystemExit(f"expected exactly one cargo-fe2o3 package; found {len(packages)}")
package = packages[0]
expected_manifest = os.path.join(workspace, "crates/cargo-fe2o3/Cargo.toml")
if os.path.realpath(package.get("manifest_path", "")) != expected_manifest:
    raise SystemExit("cargo-fe2o3 package resolved outside the admitted workspace")
targets = [
    target
    for target in package.get("targets", [])
    if target.get("name") == "cargo-fe2o3" and target.get("kind") == ["bin"]
]
if len(targets) != 1:
    raise SystemExit(f"expected exactly one cargo-fe2o3 binary target; found {len(targets)}")
source = os.path.realpath(targets[0].get("src_path", ""))
if source != os.path.join(workspace, "crates/cargo-fe2o3/src/main.rs"):
    raise SystemExit("cargo-fe2o3 binary resolved to an unexpected source")
print(package["id"])
print(source)
PY
  )
  rm -f -- "${metadata_receipt}"
  ((${#driver_identity[@]} == 2)) || {
    printf '%s\n' 'failed to bind cargo-fe2o3 package identity' >&2
    return 2
  }
  receipt="$(mktemp -- "${TMPDIR}/cargo-fe2o3-artifacts.XXXXXX.json")"
  run_step "${step_prefix}-cargo-fe2o3-bootstrap" \
    bash -c 'set -Eeuo pipefail
      receipt="$1"
      shift
      exec env CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=1 \
        cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 \
        "$@" --message-format=json-render-diagnostics >"${receipt}"' \
    cargo-fe2o3-bootstrap "${receipt}" "${feature_args[@]}"
  built_binary="$(resolve_cargo_fe2o3_artifact "${receipt}" \
    "${driver_identity[0]}" "${driver_identity[1]}" \
    "${CARGO_TARGET_DIRECTORY}")"
  rm -f -- "${receipt}"
  [[ "${built_binary}" == /* && -f "${built_binary}" &&
    ! -L "${built_binary}" && -x "${built_binary}" ]] || {
    printf 'Cargo reported an invalid cargo-fe2o3 executable: %s\n' \
      "${built_binary}" >&2
    return 2
  }
  [[ "$(realpath --canonicalize-existing -- "${built_binary}")" == "${built_binary}" ]] || {
    printf 'Cargo reported a noncanonical cargo-fe2o3 executable: %s\n' \
      "${built_binary}" >&2
    return 2
  }
  built_sha256="$(sha256sum -- "${built_binary}")"
  built_sha256="${built_sha256%% *}"
  CARGO_FE2O3_DRIVER_ROOT="${TMPDIR}/fe2o3-ci-driver-${built_sha256}"
  [[ ! -e "${CARGO_FE2O3_DRIVER_ROOT}" && ! -L "${CARGO_FE2O3_DRIVER_ROOT}" ]] || {
    printf 'private qualification driver root already exists: %s\n' \
      "${CARGO_FE2O3_DRIVER_ROOT}" >&2
    return 2
  }
  mkdir -m 700 -- "${CARGO_FE2O3_DRIVER_ROOT}"
  install -m 0500 -- "${built_binary}" \
    "${CARGO_FE2O3_DRIVER_ROOT}/cargo-fe2o3"
  CARGO_FE2O3_BINARY="${CARGO_FE2O3_DRIVER_ROOT}/cargo-fe2o3"
  CARGO_FE2O3_SHA256="${built_sha256}"
  chmod 500 -- "${CARGO_FE2O3_DRIVER_ROOT}"
  validate_cargo_fe2o3_driver
  CARGO_FE2O3_DRIVER_PROFILE="${driver_profile}"
}

ensure_production_cargo_fe2o3_driver() {
  local step_prefix="$1"
  case "${CARGO_FE2O3_DRIVER_PROFILE}" in
    production)
      validate_cargo_fe2o3_driver
      ;;
    "")
      prepare_cargo_fe2o3_driver "${step_prefix}" production
      ;;
    *)
      printf 'CPU tests cannot reuse %s cargo-fe2o3 driver as production\n' \
        "${CARGO_FE2O3_DRIVER_PROFILE}" >&2
      return 2
      ;;
  esac
}

cleanup_cargo_fe2o3_driver() {
  if [[ -n "${CARGO_FE2O3_DRIVER_ROOT}" && -d "${CARGO_FE2O3_DRIVER_ROOT}" &&
    ! -L "${CARGO_FE2O3_DRIVER_ROOT}" ]]; then
    chmod 700 -- "${CARGO_FE2O3_DRIVER_ROOT}" || true
    rm -rf -- "${CARGO_FE2O3_DRIVER_ROOT}"
  fi
  if [[ -n "${CI_PRIVATE_TMP_ROOT}" && -n "${CARGO_TARGET_DIRECTORY}" &&
    -d "${CI_PRIVATE_TMP_ROOT}" && ! -L "${CI_PRIVATE_TMP_ROOT}" &&
    "$(dirname -- "${CI_PRIVATE_TMP_ROOT}")" == "${CARGO_TARGET_DIRECTORY}" &&
    "$(basename -- "${CI_PRIVATE_TMP_ROOT}")" == "fe2o3-ci-tmp-${BASHPID}" &&
    "$(stat -c '%u' -- "${CI_PRIVATE_TMP_ROOT}")" == "$(id -u)" ]]; then
    rm -rf -- "${CI_PRIVATE_TMP_ROOT}"
  fi
  if [[ -n "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" &&
    -n "${CARGO_TARGET_DIRECTORY}" &&
    -d "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" &&
    ! -L "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" &&
    "$(dirname -- "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}")" == \
      "${CARGO_TARGET_DIRECTORY}" &&
    "$(basename -- "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}")" == \
      "fe2o3-qualification-tests-${BASHPID}" &&
    "$(stat -c '%u' -- "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}")" == \
      "$(id -u)" ]]; then
    rm -rf -- "${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}"
  fi
  CARGO_FE2O3_BINARY=
  CARGO_FE2O3_SHA256=
  CARGO_FE2O3_DRIVER_ROOT=
  CARGO_FE2O3_DRIVER_PROFILE=
  CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY=
}

trap cleanup_cargo_fe2o3_driver EXIT

load_dynamic_loader_environment_removals() {
  local destination_name="$1"
  local name
  local -n destination="${destination_name}"

  destination=()
  while IFS= read -r name; do
    case "${name}" in
      LD_* | DYLD_* | GLIBC_TUNABLES)
        destination+=(-u "${name}")
        ;;
    esac
  done < <(compgen -e)
}

validate_managed_wrapper_source_namespaces() {
  local package fallback
  local -a managed_packages=() loader_environment_removals
  local -A seen=() fallback_packages=()

  for fallback in "${ROCM_EXPLICIT_NAMESPACE_FALLBACK_PACKAGES[@]}"; do
    fallback_packages["${fallback}"]=1
  done
  for package in "$@"; do
    if [[ -n "${seen[${package}]:-}" ]]; then
      continue
    fi
    seen["${package}"]=1
    if [[ -n "${fallback_packages[${package}]:-}" ]]; then
      continue
    fi
    managed_packages+=("${package}")
  done
  ((${#managed_packages[@]} > 0)) || return 0
  validate_cargo_fe2o3_driver
  load_dynamic_loader_environment_removals loader_environment_removals
  env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}" examples check-wrapper-namespaces \
    "${managed_packages[@]}"
}

load_example_packages() {
  local lane="$1"
  local destination_name="$2"
  local cargo_fe2o3_binary="${3:-}"
  local output
  local -n destination="${destination_name}"

  if [[ -n "${cargo_fe2o3_binary}" ]]; then
    local -a loader_environment_removals
    load_dynamic_loader_environment_removals loader_environment_removals
    output="$(
      env "${loader_environment_removals[@]}" \
        "${cargo_fe2o3_binary}" examples list "${lane}"
    )"
  else
    output="$(
      cargo run --quiet --locked -p cargo-fe2o3 -- examples list "${lane}"
    )"
  fi
  destination=()
  if [[ -n "${output}" ]]; then
    # shellcheck disable=SC2034  # The destination is written through a nameref.
    mapfile -t destination <<<"${output}"
  fi
}

run_format() {
  run_step format cargo fmt --all -- --check
}

run_check() {
  local -a all_examples rustc_examples wrapper_managed_packages cargo_args
  local -a loader_environment_removals
  local -A rustc_example_set=()
  local package
  prepare_cargo_fe2o3_driver generic-check production
  validate_cargo_fe2o3_driver
  load_example_packages all all_examples "${CARGO_FE2O3_BINARY}"
  load_example_packages rustc-check rustc_examples "${CARGO_FE2O3_BINARY}"
  load_example_packages wrapper-managed wrapper_managed_packages \
    "${CARGO_FE2O3_BINARY}"
  for package in "${rustc_examples[@]}"; do
    rustc_example_set["${package}"]=1
  done

  cargo_args=(check --workspace --all-targets --locked)
  # These fixtures intentionally require the authenticated cargo-fe2o3 wrapper
  # to supply their compiler-owned crate binding.
  cargo_args+=(
    --exclude fe2o3-production-extraction-fixture
    --exclude fe2o3-production-ranked-bounds-fixture
  )
  for package in "${all_examples[@]}"; do
    if [[ -z "${rustc_example_set[${package}]+selected}" ]]; then
      cargo_args+=(--exclude "${package}")
    fi
  done

  ((${#wrapper_managed_packages[@]} > 0)) || {
    printf '%s\n' 'workspace contains no structurally detected wrapper-managed package' >&2
    return 2
  }
  # The package-aware wrapper sees the complete supported workspace graph. Its sealed
  # target-source projection injects bindings only for directly managed package targets,
  # including transitive managed libraries, and leaves ordinary dependencies unbound.
  load_dynamic_loader_environment_removals loader_environment_removals
  run_step workspace-binding-check \
    env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}" "${cargo_args[@]}"
  run_step workspace-binding-check-boundary \
    bash scripts/tests/binding-check-boundary.sh \
    "${CARGO_FE2O3_BINARY}" "${wrapper_managed_packages[0]}"
  # This is an authority-free policy rescan, not a retained source snapshot.
  # Recompute the exact set after the nested Cargo checks so projection drift
  # fails this lane before it reports success.
  run_step workspace-binding-projection-revalidation \
    env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}" examples check-wrapper-managed \
    "${wrapper_managed_packages[@]}"
}

run_cpu_tests() {
  local cargo_args=(test --locked)
  local wrapper_cargo_args=(test --locked --all-targets)
  local -a raw_cpu_examples wrapper_cpu_examples wrapper_managed_packages
  local -a loader_environment_removals
  local -A selected_cpu_examples=()
  local package
  ensure_production_cargo_fe2o3_driver cpu-tests
  for package in "${CPU_TEST_PACKAGES[@]}"; do
    cargo_args+=(-p "${package}")
  done
  load_example_packages cpu-test-raw raw_cpu_examples "${CARGO_FE2O3_BINARY}"
  load_example_packages cpu-test-wrapper-managed wrapper_cpu_examples \
    "${CARGO_FE2O3_BINARY}"
  load_example_packages wrapper-managed wrapper_managed_packages \
    "${CARGO_FE2O3_BINARY}"
  for package in "${raw_cpu_examples[@]}"; do
    [[ -z "${selected_cpu_examples[${package}]+selected}" ]] || {
      printf 'duplicate raw CPU example package: %s\n' "${package}" >&2
      return 2
    }
    selected_cpu_examples["${package}"]=raw
    cargo_args+=(-p "${package}")
  done
  for package in "${wrapper_cpu_examples[@]}"; do
    [[ -z "${selected_cpu_examples[${package}]+selected}" ]] || {
      printf 'CPU example partition selected package twice: %s\n' "${package}" >&2
      return 2
    }
    selected_cpu_examples["${package}"]=wrapper-managed
    wrapper_cargo_args+=(-p "${package}")
  done
  # Keep the generic test lane independent of whether the host happens to have
  # ROCm installed. The raw HIP crate supplies a fail-closed no-runtime ABI.
  run_step cargo-fe2o3-tests env FE2O3_HIP_SYS_DISABLE=1 \
    cargo test --locked -p cargo-fe2o3 \
      --features "${CARGO_FE2O3_QUALIFICATION_FEATURE}"
  run_step cpu-tests env FE2O3_HIP_SYS_DISABLE=1 cargo "${cargo_args[@]}"
  load_dynamic_loader_environment_removals loader_environment_removals
  if ((${#wrapper_cpu_examples[@]} > 0)); then
    validate_cargo_fe2o3_driver
    run_step wrapper-managed-cpu-tests \
      env "${loader_environment_removals[@]}" FE2O3_HIP_SYS_DISABLE=1 \
      "${CARGO_FE2O3_BINARY}" "${wrapper_cargo_args[@]}"
  fi
  # The queries above and the test command are authority-free policy scans. Recheck
  # both selected lists and the complete structural set so source drift cannot
  # silently reroute or omit a package.
  run_step cpu-test-partition-revalidation \
    env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}" examples check-cpu-test-partition \
    "${raw_cpu_examples[@]}" -- "${wrapper_cpu_examples[@]}"
  run_step cpu-test-binding-projection-revalidation \
    env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}" examples check-wrapper-managed \
    "${wrapper_managed_packages[@]}"
  run_step dialect-mir-pliron-tests \
    cargo test --locked -p dialect-mir --features pliron --test pliron_shell
}

run_auxiliary_tests() {
  # fe2o3-core unit tests link HIP, but its compile-fail doctests do not.
  run_step core-doc-tests cargo test --locked --doc -p fe2o3-core
  run_step device-copy-renamed-dependency \
    cargo check --locked -p device-copy-renamed-dependency
  run_step device-copy-derive-real-trait \
    cargo check --locked -p fe2o3-core --test device_copy_derive_compile
  run_step device-copy-derive-ui \
    cargo test --locked -p fe2o3-core --test device_copy_derive_ui
  run_step s09-debug-checker bash scripts/tests/s09-debug.sh
  run_step s09-debug-ci-guard bash scripts/tests/s09-debug-ci.sh
}

run_shard_policy() {
  run_step rustc-codegen-shard-policy \
    python3 "${RUSTC_CODEGEN_SHARD_POLICY}" check
}

run_workspace_dependency_policy() {
  run_step workspace-dependency-policy-tests \
    python3 "${WORKSPACE_DEPENDENCY_POLICY_TESTS}"
  run_step workspace-dependency-policy \
    python3 "${WORKSPACE_DEPENDENCY_POLICY_CHECKER}" \
      --policy "${WORKSPACE_DEPENDENCY_POLICY}"
  run_step pliron-dependency-policy-tests \
    python3 "${PLIRON_DEPENDENCY_POLICY_TESTS}"
  run_step pliron-dependency-policy \
    python3 "${PLIRON_DEPENDENCY_POLICY_CHECKER}"
}

run_standalone_lockfiles() {
  run_step standalone-lockfiles bash "${STANDALONE_LOCKFILE_CHECKER}"
}

run_runtime_pure_rust_policy() {
  run_step runtime-pure-rust-audit-tests \
    env PYTHONDONTWRITEBYTECODE=1 python3 "${RUNTIME_PURE_RUST_AUDIT_TESTS}"
  run_step runtime-identity-oracle-parser-tests \
    env PYTHONDONTWRITEBYTECODE=1 python3 "${RUNTIME_IDENTITY_ORACLE_TESTS}"
  run_step runtime-pure-rust-metadata \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" metadata --cargo \
      --root fe2o3-kfd \
      --root fe2o3-drm-uapi \
      --root fe2o3-kfd-uapi \
      --root fe2o3-amdhsa-loader \
      --root fe2o3-aql \
      --root fe2o3-runtime-model
  run_step runtime-pure-rust-kfd-examples-build \
    env CARGO_TARGET_DIR="${RUNTIME_PURE_RUST_TARGET_DIR}" \
      cargo build --locked -p fe2o3-kfd \
        --example kfd-version \
        --example kfd-topology \
        --example kfd-device-identity \
        --example kfd-host-visible-memory-policy \
        --example kfd-shared-gtt-memory-policy \
        --example kfd-queue-resources \
        --example kfd-compute-aql-queue-policy
  run_step runtime-pure-rust-kfd-version-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-version"
  run_step runtime-pure-rust-kfd-topology-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-topology"
  run_step runtime-pure-rust-kfd-device-identity-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-device-identity"
  run_step runtime-pure-rust-kfd-memory-policy-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-host-visible-memory-policy"
  run_step runtime-pure-rust-kfd-shared-memory-policy-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-shared-gtt-memory-policy"
  run_step runtime-pure-rust-kfd-queue-resources-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-queue-resources"
  run_step runtime-pure-rust-kfd-compute-aql-queue-elf \
    python3 "${RUNTIME_PURE_RUST_AUDITOR}" \
      --policy "${RUNTIME_PURE_RUST_POLICY}" elf \
      --input "${RUNTIME_PURE_RUST_TARGET_DIR}/debug/examples/kfd-compute-aql-queue-policy"
}

run_runtime_identity_oracle() {
  run_step runtime-identity-oracle bash "${RUNTIME_IDENTITY_ORACLE}"
}

load_rustc_codegen_shards() {
  local destination_name="$1"
  local output
  # shellcheck disable=SC2178  # The destination is a caller-owned array nameref.
  local -n destination="${destination_name}"
  if ! output="$(python3 "${RUSTC_CODEGEN_SHARD_POLICY}" list)"; then
    return 2
  fi
  destination=()
  # shellcheck disable=SC2034  # The destination is written through a nameref.
  mapfile -t destination <<<"${output}"
}

load_rustc_codegen_shard_targets() {
  local shard_id="$1"
  local destination_name="$2"
  local output
  # shellcheck disable=SC2178  # The destination is a caller-owned array nameref.
  local -n destination="${destination_name}"
  if ! output="$(python3 "${RUSTC_CODEGEN_SHARD_POLICY}" tests "${shard_id}")"; then
    return 2
  fi
  destination=()
  # shellcheck disable=SC2034  # The destination is written through a nameref.
  mapfile -t destination <<<"${output}"
}

run_rustc_codegen_lib_tests() {
  # Do not combine this with integration targets: Cargo can emit a test rlib
  # and an unversioned backend dylib with different Rust symbol hashes.
  # Keep the aggregate rustc-private harness bounded like the isolated targets;
  # full debuginfo can exceed the executable identity measurement limit.
  run_step rustc-codegen-lib-tests \
    env CARGO_PROFILE_DEV_DEBUG=1 \
      cargo test --locked -p "${RUSTC_CODEGEN_TEST_PACKAGE}" --lib
  run_step rustc-codegen-qualification-route-tests \
    cargo test --locked -p "${RUSTC_CODEGEN_TEST_PACKAGE}" \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --lib qualification_selection::tests
}

run_rustc_codegen_test_driver() {
  # Integration targets invoke cargo-fe2o3 from inside the test process. Build
  # that shared driver once, with bounded parallelism, before any nested Cargo
  # invocation can compete with the test harness on a small hosted runner.
  run_step rustc-codegen-driver-bootstrap \
    env CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=1 \
      cargo build --locked -p "${RUSTC_CODEGEN_TEST_DRIVER_PACKAGE}" \
        --features "${CARGO_FE2O3_QUALIFICATION_FEATURE}" \
        --bin "${RUSTC_CODEGEN_TEST_DRIVER_PACKAGE}"
}

run_rustc_codegen_target() {
  local test_target="$1"
  local -a command=(
    env CARGO_PROFILE_DEV_DEBUG=1
    cargo test --locked -p "${RUSTC_CODEGEN_TEST_PACKAGE}"
    --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}"
    --test "${test_target}"
  )

  # This target launches several independent clean Cargo graphs. Bound libtest
  # concurrency explicitly so runner core count cannot multiply their storage
  # and compiler pressure without limit.
  if [[ "${test_target}" == collected_executable_scalar_control_flow_v2 ]]; then
    command+=(-- --test-threads="${COLLECTED_CONTROL_FLOW_TEST_THREADS}")
  fi

  # Cargo can emit a test rlib and an unversioned backend dylib with different
  # Rust symbol hashes during one --all-targets build. Each target-isolated
  # invocation produces the exact backend dylib before running its linked test.
  # Match the limited-debug profile used by the production automatic backend
  # builder so clean and cache-restored shards exercise the same bounded image.
  if [[ "${test_target}" == collected_executable_scalar_control_flow_v2 ]]; then
    run_step_with_timeout "${COLLECTED_CONTROL_FLOW_TIMEOUT_SECONDS}" \
      "rustc-codegen-test-${test_target}" \
      "${command[@]}"
    return
  fi
  if [[ "${test_target}" == general_gemm_semantic_frontend ]]; then
    run_step_with_timeout "${GENERAL_GEMM_SEMANTIC_FRONTEND_TIMEOUT_SECONDS}" \
      "rustc-codegen-test-${test_target}" \
      "${command[@]}"
    return
  fi
  run_step "rustc-codegen-test-${test_target}" \
    "${command[@]}"
}

run_rustc_codegen_shard_targets() {
  local shard_id="$1"
  local -a test_targets
  local test_target
  load_rustc_codegen_shard_targets "${shard_id}" test_targets
  for test_target in "${test_targets[@]}"; do
    run_rustc_codegen_target "${test_target}"
  done
}

run_all_rustc_codegen_shards() {
  local -a shard_ids
  local shard_id
  load_rustc_codegen_shards shard_ids
  for shard_id in "${shard_ids[@]}"; do
    run_rustc_codegen_shard_targets "${shard_id}"
  done
}

run_rustc_codegen_shard() {
  local shard_id="$1"
  run_shard_policy
  run_rustc_codegen_test_driver
  run_rustc_codegen_shard_targets "${shard_id}"
}

run_rustc_codegen_tests() {
  run_shard_policy
  run_rustc_codegen_test_driver
  run_rustc_codegen_lib_tests
  run_all_rustc_codegen_shards
}

run_tests() {
  run_cpu_tests
  run_rustc_codegen_tests
  run_auxiliary_tests
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
  run_step runtime-model-verus \
    "${REPO_ROOT}/crates/fe2o3-runtime-model/verus/verify-verus.sh"
  run_step verus-fixtures \
    "${REPO_ROOT}/examples/verus_vecadd/run-verus.sh" --require
  run_step scalar-gemm-verus \
    "${REPO_ROOT}/examples/scalar_gemm_v1/run-verus.sh" --require
  run_step mir-pliron-semantic-refinement-verus \
    "${REPO_ROOT}/scripts/test-mir-pliron-semantic-refinement-verus.sh"
}

run_authority_launcher_tests() {
  run_step authority-launcher-tests \
    bash scripts/tests/cargo-fe2o3-authority-launcher.sh
}

run_rustc_trampoline_tests() {
  run_step rustc-trampoline-tests \
    bash scripts/tests/fe2o3-rustc-trampoline.sh
  run_step cargo-binding-trampoline-tests \
    bash scripts/tests/fe2o3-cargo-binding-trampoline.sh
}

run_parity_matrix_checks() {
  run_step parity-matrix-check bash scripts/parity-matrix.sh check
  run_step parity-matrix-tests bash scripts/tests/parity-matrix.sh
  run_step parity-evidence-tests bash scripts/tests/parity-evidence.sh
  run_step parity-oci-executor-tests \
    bash scripts/tests/parity-oci-executor.sh
  run_step parity-oci-operator-tests \
    bash scripts/tests/parity-oci-operator.sh
  run_authority_launcher_tests
  run_rustc_trampoline_tests
  run_step parity-row-evidence-tests \
    bash scripts/tests/parity-row-evidence.sh
  run_step parity-publisher-client-tests \
    python3 scripts/tests/parity-publisher-client.py
  run_step parity-signed-evidence-fd-tests \
    python3 scripts/tests/parity-signed-evidence-fd.py
  run_step parity-repository-rules-tests \
    bash scripts/tests/parity-repository-rules.sh
  run_step mi300x-evidence-queue-tests \
    bash scripts/tests/mi300x-evidence-queue.sh
  run_step hosted-parity-ci-tests \
    bash scripts/tests/hosted-parity-ci.sh
}

run_generic_core() {
  run_workspace_dependency_policy
  run_standalone_lockfiles
  run_runtime_pure_rust_policy
  run_step example-manifest \
    cargo run --quiet --locked -p cargo-fe2o3 -- examples check
  run_step bounded-moe-docs \
    python3 scripts/test-bounded-moe-docs.py
  run_shard_policy
  run_parity_matrix_checks
  run_format
  run_check
  run_backend_build
  run_step ci-local-test-gate bash scripts/tests/ci-local-test-gate.sh
  run_cpu_tests
  run_rustc_codegen_lib_tests
  run_auxiliary_tests
}

run_generic() {
  run_generic_core
  run_rustc_codegen_test_driver
  run_all_rustc_codegen_shards
}

run_rocm_compile() {
  export FE2O3_TARGET="${FE2O3_TARGET:-gfx1100}"
  local -a example_packages loader_environment_removals
  local package

  prepare_cargo_fe2o3_driver rocm qualification
  prepare_cargo_fe2o3_qualification_target
  load_dynamic_loader_environment_removals loader_environment_removals
  validate_cargo_fe2o3_driver
  load_example_packages \
    rocm-compile example_packages "${CARGO_FE2O3_BINARY}"
  validate_managed_wrapper_source_namespaces \
    "${example_packages[@]}" \
    "${ROCM_TRUSTED_DEVICE_ITEM_PACKAGES[@]}"
  validate_cargo_fe2o3_driver
  run_step rocm-doctor \
    env "${loader_environment_removals[@]}" \
      "${CARGO_FE2O3_BINARY}" doctor
  for package in "${ROCM_QUALIFICATION_TEST_PACKAGES[@]}"; do
    run_step "rocm-qualification-tests-${package}" \
      env -u FE2O3_TARGET "${loader_environment_removals[@]}" \
        CARGO_TARGET_DIR="${CARGO_FE2O3_QUALIFICATION_TARGET_DIRECTORY}" \
        "${CARGO_FE2O3_BINARY}" test --locked --all-targets \
        -p "${package}" \
        --features "${CARGO_FE2O3_QUALIFICATION_FEATURE}"
  done
  run_step rocm-trusted-device-items \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test trusted_device_items \
      genuine_markers_emit_and_local_external_spoofs_fail_closed -- \
      --ignored --exact
  run_step rocm-trusted-device-item-stale-cleanup \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test trusted_device_items \
      rejected_lookalikes_remove_preseeded_artifacts_atomically -- \
      --ignored --exact
  run_step rocm-cross-crate-typed-binding \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      FE2O3_TEST_TARGET="${FE2O3_TARGET}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test cross_crate_typed_binding \
      same_logical_name_in_two_rlibs_resolves_distinct_artifacts -- \
      --ignored --exact
  run_step rocm-g1-code-object \
    cargo test --locked -p dialect-amdgcn --test lowering \
      rocm_compiles_the_golden_to_an_amdgpu_code_object -- \
      --ignored --exact
  run_step rocm-kernel-ir-codegen-rejection \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test kernel_ir_codegen \
      selected_pipeline_rejects_invalid_or_unsupported_inputs_and_cleans_stale_artifacts -- \
      --ignored --exact
  run_step rocm-kernel-ir-vecadd \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test kernel_ir_codegen \
      opt_in_vecadd_publishes_exact_g1_without_gpu -- \
      --ignored --exact

  local package
  for package in "${example_packages[@]}"; do
    # cargo-fe2o3 rotates its device-artifact generation when codegen
    # semantics change. Rebuild each example so Cargo cannot reuse a host
    # package fingerprint that predates the new generation.
    run_step "rocm-clean-${package}" \
      cargo clean -p "${package}"
    validate_cargo_fe2o3_driver
    run_step "rocm-build-${package}" \
      env "${loader_environment_removals[@]}" \
        "${CARGO_FE2O3_BINARY}" build -p "${package}"
    validate_cargo_fe2o3_driver
    run_step "rocm-artifacts-${package}" \
      env "${loader_environment_removals[@]}" \
        "${CARGO_FE2O3_BINARY}" \
        examples check-artifacts "${package}" \
        "${CARGO_TARGET_DIRECTORY}/fe2o3"
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
  require_gpu_access /dev/kfd /dev/dxg
  if ! command -v rocminfo >/dev/null 2>&1; then
    printf '%s\n' 'GPU smoke requires rocminfo on PATH' >&2
    return 2
  fi

  local -a loader_environment_removals
  prepare_cargo_fe2o3_driver hardware qualification
  load_dynamic_loader_environment_removals loader_environment_removals

  validate_cargo_fe2o3_driver
  run_step hardware-rocminfo rocminfo
  validate_cargo_fe2o3_driver
  run_step hardware-doctor \
    env "${loader_environment_removals[@]}" \
      "${CARGO_FE2O3_BINARY}" doctor
  local rocm_path="${ROCM_PATH:-/opt/rocm}"
  local native_test_directory="${CARGO_TARGET_DIRECTORY}/ci-native"
  local native_test="${native_test_directory}/fe2o3-hip-device-properties-test"
  mkdir -p -- "${native_test_directory}"
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
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test kernel_ir_codegen \
      opt_in_fill_publishes_g1_and_executes_on_the_gpu -- \
      --ignored --exact
  run_step hardware-kernel-ir-vecadd \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      cargo test --locked -p rustc-codegen-fe2o3 \
      --features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}" \
      --test kernel_ir_codegen \
      opt_in_vecadd_publishes_exact_g1_and_executes_on_the_gpu -- \
      --ignored --exact
  validate_cargo_fe2o3_driver
  run_step hardware-smoke \
    env "${loader_environment_removals[@]}" \
      "${CARGO_FE2O3_BINARY}" smoke
  local test_wavefront
  test_wavefront="$(wavefront_for_target "${FE2O3_TARGET}")"
  run_step hardware-hsaco-inspection env \
    FE2O3_TEST_HSACO="${CARGO_TARGET_DIRECTORY}/fe2o3/vecadd.hsaco" \
    FE2O3_TEST_TARGET="${FE2O3_TARGET}" \
    FE2O3_TEST_WAVEFRONT="${test_wavefront}" \
    cargo test --locked -p fe2o3-hsaco --test inspection \
      inspects_real_generated_vecadd_hsaco -- --ignored --exact
}

run_s09_debug_hardware() {
  if [[ -z "${FE2O3_S09_EVIDENCE_DIR:-}" ]]; then
    printf '%s\n' 'S09 debug hardware requires FE2O3_S09_EVIDENCE_DIR' >&2
    return 2
  fi
  local -a loader_environment_removals
  prepare_cargo_fe2o3_driver s09 qualification
  load_dynamic_loader_environment_removals loader_environment_removals
  validate_cargo_fe2o3_driver
  run_step s09-debug-hardware \
    env "${loader_environment_removals[@]}" \
      FE2O3_TEST_CARGO_FE2O3_BIN="${CARGO_FE2O3_BINARY}" \
      FE2O3_TEST_CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256}" \
      bash scripts/s09-debug-ci.sh "${FE2O3_S09_EVIDENCE_DIR}"
}

run_parity_production_immutable() {
  run_step parity-production-immutable \
    bash scripts/tests/parity-production-immutable-ingest.sh
}

main() {
  cd "${REPO_ROOT}"
  mkdir -p "${LOG_DIR}"
  validate_private_directory 'CI log directory' "${LOG_DIR}"

  case "${1:-}" in
    generic) run_generic ;;
    generic-core) run_generic_core ;;
    workspace-policy) run_workspace_dependency_policy ;;
    standalone-locks) run_standalone_lockfiles ;;
    runtime-policy) run_runtime_pure_rust_policy ;;
    runtime-identity-oracle) run_runtime_identity_oracle ;;
    shard-policy) run_shard_policy ;;
    rustc-codegen-shard)
      if (($# != 2)); then
        printf '%s\n' 'rustc-codegen-shard requires exactly one shard id' >&2
        return 2
      fi
      run_rustc_codegen_shard "$2"
      ;;
    format) run_format ;;
    check) run_check ;;
    test) run_tests ;;
    workspace-test) run_workspace_tests ;;
    rustc-codegen-test) run_rustc_codegen_tests ;;
    backend) run_backend_build ;;
    authority-launcher) run_authority_launcher_tests ;;
    rustc-trampoline) run_rustc_trampoline_tests ;;
    parity-evidence) run_parity_matrix_checks ;;
    parity-production-immutable) run_parity_production_immutable ;;
    verus) run_verus ;;
    rocm-compile) run_rocm_compile ;;
    hardware-smoke) run_hardware_smoke ;;
    s09-debug-hardware) run_s09_debug_hardware ;;
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
