#!/usr/bin/env bash

set -Eeuo pipefail
export PYTHONDONTWRITEBYTECODE=1

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly ROOT
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly FINALIZER="${ROOT}/scripts/s09-debug-finalize.sh"
readonly PINNER="${ROOT}/scripts/s09_pinned_snapshot.py"
readonly SOURCE_STATE_CHECKER="${ROOT}/scripts/s09-source-state.py"
readonly DWARFDUMP=/opt/rocm/llvm/bin/llvm-dwarfdump
readonly READOBJ=/opt/rocm/llvm/bin/llvm-readobj
readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3.12

fail() {
  printf 's09-debug-ci: %s\n' "$1" >&2
  exit 2
}

canonical_executable() {
  local label="$1"
  local path="$2"
  [[ "${path}" == /* && -f "${path}" && ! -L "${path}" && -x "${path}" ]] ||
    fail "${label} must be an absolute executable regular file"
  [[ "$(realpath --canonicalize-existing -- "${path}")" == "${path}" ]] ||
    fail "${label} path must already be canonical"
}

private_driver() {
  local path="$1"
  local expected_sha256="$2"
  local parent file_mode parent_mode owner parent_owner observed

  canonical_executable cargo-fe2o3-test-driver "${path}"
  [[ "${expected_sha256}" =~ ^[0-9a-f]{64}$ ]] ||
    fail 'cargo-fe2o3-test-driver SHA-256 is malformed'
  parent="$(dirname -- "${path}")"
  file_mode="$(stat -c '%a' -- "${path}")"
  parent_mode="$(stat -c '%a' -- "${parent}")"
  owner="$(stat -c '%u' -- "${path}")"
  parent_owner="$(stat -c '%u' -- "${parent}")"
  observed="$(sha256sum -- "${path}")"
  observed="${observed%% *}"
  [[ "${file_mode}" == 500 && "${parent_mode}" == 500 &&
    "${owner}" == "$(id -u)" && "${parent_owner}" == "$(id -u)" &&
    "${observed}" == "${expected_sha256}" ]] ||
    fail 'cargo-fe2o3-test-driver identity or private custody changed'
}

executable_file() {
  local label="$1"
  local path="$2"
  [[ -f "${path}" && ! -L "${path}" && -x "${path}" ]] ||
    fail "${label} must be an executable regular file"
}

mode=outer
if [[ "${1:-}" == --source-supervised ]]; then
  mode=source-supervised
  shift
fi
readonly mode
if [[ "${mode}" == outer && $# != 1 ]] ||
  [[ "${mode}" == source-supervised && $# != 3 ]]; then
  fail "usage: scripts/s09-debug-ci.sh ABSOLUTE-NEW-EVIDENCE-DIRECTORY"
fi
[[ "${FE2O3_ALLOW_S09_DEBUG:-}" == 1 ]] ||
  fail "set FE2O3_ALLOW_S09_DEBUG=1 to run the real S09 hardware lane"

readonly EVIDENCE="$1"
[[ "${EVIDENCE}" == /* ]] || fail "evidence directory must be absolute"
[[ ! -e "${EVIDENCE}" && ! -L "${EVIDENCE}" ]] ||
  fail "evidence directory must not already exist"
EVIDENCE_PARENT="$(dirname -- "${EVIDENCE}")"
readonly EVIDENCE_PARENT
[[ -d "${EVIDENCE_PARENT}" && ! -L "${EVIDENCE_PARENT}" ]] ||
  fail "evidence parent must be a real directory"
[[ "$(realpath --canonicalize-existing -- "${EVIDENCE_PARENT}")" == "${EVIDENCE_PARENT}" ]] ||
  fail "evidence parent must already be canonical"
[[ "${EVIDENCE}/" != "${ROOT}/"* ]] ||
  fail "evidence directory must be outside the source worktree"

if [[ "${mode}" == outer ]]; then
  exec "${SOURCE_STATE_CHECKER}" \
    --root "${ROOT}" \
    -- "${BASH_SOURCE[0]}" --source-supervised "${EVIDENCE}" \
    '{source_commit}' '{source_tree}'
fi

SOURCE_COMMIT="$2"
SOURCE_TREE="$3"
[[ "${SOURCE_COMMIT}" =~ ^[0-9a-f]{40}$|^[0-9a-f]{64}$ ]] ||
  fail "source-state supervisor supplied a malformed commit"
[[ "${SOURCE_TREE}" =~ ^[0-9a-f]{40}$|^[0-9a-f]{64}$ ]] ||
  fail "source-state supervisor supplied a malformed tree"
readonly SOURCE_COMMIT SOURCE_TREE

readonly WORKER="${FE2O3_LLVM_LINK_WORKER:-}"
readonly WORKER_BUILD_ID="${FE2O3_LLVM_LINK_WORKER_BUILD_ID:-}"
readonly LLVM_BUILD_ID="${FE2O3_LLVM_BUILD_ID:-}"
readonly CARGO_FE2O3="${FE2O3_TEST_CARGO_FE2O3_BIN:-}"
readonly CARGO_FE2O3_SHA256="${FE2O3_TEST_CARGO_FE2O3_SHA256:-}"
canonical_executable Worker-V2 "${WORKER}"
private_driver "${CARGO_FE2O3}" "${CARGO_FE2O3_SHA256}"
executable_file llvm-dwarfdump "${DWARFDUMP}"
executable_file llvm-readobj "${READOBJ}"
executable_file ROCgdb "${ROCGDB}"
executable_file S09-runner "${RUNNER}"
executable_file S09-finalizer "${FINALIZER}"
executable_file snapshot-supervisor "${PINNER}"
[[ "${WORKER_BUILD_ID}" =~ ^fe2o3-worker-v1-sha256-[0-9a-f]{64}$ ]] ||
  fail "Worker V2 build identity is malformed"
[[ "${LLVM_BUILD_ID}" =~ ^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$ ]] ||
  fail "LLVM build identity is malformed"
CARGO_TARGET_ROOT="${CARGO_TARGET_DIR:-${ROOT}/target}"
if [[ "${CARGO_TARGET_ROOT}" != /* ]]; then
  CARGO_TARGET_ROOT="${ROOT}/${CARGO_TARGET_ROOT}"
fi
[[ -d "${CARGO_TARGET_ROOT}" && ! -L "${CARGO_TARGET_ROOT}" ]] ||
  fail 'Cargo target root must be an existing non-symlink directory'
[[ "$(realpath --canonicalize-existing -- "${CARGO_TARGET_ROOT}")" == \
  "${CARGO_TARGET_ROOT}" ]] ||
  fail 'Cargo target root must already be canonical and contain no traversal'
TARGET_MODE="$(stat -c '%a' -- "${CARGO_TARGET_ROOT}")"
TARGET_OWNER="$(stat -c '%u' -- "${CARGO_TARGET_ROOT}")"
readonly TARGET_MODE TARGET_OWNER
(((8#${TARGET_MODE} & 8#077) == 0)) ||
  fail 'Cargo target root must be private'
[[ "${TARGET_OWNER}" == "$(id -u)" ]] ||
  fail 'Cargo target root must be owned by the current user'
readonly CARGO_TARGET_ROOT
readonly BUILD_TARGET="${CARGO_TARGET_ROOT}/s09-debug-hardware-${BASHPID}"
[[ ! -e "${BUILD_TARGET}" && ! -L "${BUILD_TARGET}" ]] ||
  fail "isolated hardware target already exists"
cleanup() {
  rm -rf -- "${BUILD_TARGET}"
}
trap cleanup EXIT

umask 077
mkdir -m 700 -- "${EVIDENCE}"
readonly HSACO="${EVIDENCE}/alpha-debug.hsaco"

cd "${ROOT}"
FE2O3_LLVM_LINK_WORKER="${WORKER}" \
FE2O3_LLVM_LINK_WORKER_BUILD_ID="${WORKER_BUILD_ID}" \
FE2O3_LLVM_BUILD_ID="${LLVM_BUILD_ID}" \
FE2O3_LLVM_DWARFDUMP="${DWARFDUMP}" \
FE2O3_S09_DEBUG_HSACO_OUTPUT="${HSACO}" \
  cargo test --locked -p rustc-codegen-fe2o3 \
    --features qualification-oracles-test-only \
    --test kernel_ir_codegen \
    worker_v2_s09_alpha_o0_preserves_source_dwarf_in_hsaco -- \
    --ignored --exact --nocapture

cargo test --locked -p fe2o3-hsa-runtime \
  --features hardware-test-hooks \
  --test s09_gfx942_alpha_hardware \
  --target-dir "${BUILD_TARGET}" \
  --no-run

mapfile -d '' -t hardware_tests < <(
  find "${BUILD_TARGET}/debug/deps" -maxdepth 1 -type f -perm -0100 \
    -name 's09_gfx942_alpha_hardware-*' -print0
)
if ((${#hardware_tests[@]} != 1)); then
  fail "isolated build produced ${#hardware_tests[@]} candidate hardware executables"
fi
HARDWARE_TEST="$(realpath --canonicalize-existing -- "${hardware_tests[0]}")"
readonly HARDWARE_TEST

"${PINNER}" \
  --input "hsaco=${HSACO}" \
  --input "host=${HARDWARE_TEST}" \
  --executable host \
  -- "${FINALIZER}" \
  "${EVIDENCE}" '{hsaco}' '{host}' "${SOURCE_COMMIT}" "${SOURCE_TREE}"
