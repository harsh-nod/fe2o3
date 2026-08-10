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

executable_file() {
  local label="$1"
  local path="$2"
  [[ -f "${path}" && ! -L "${path}" && -x "${path}" ]] ||
    fail "${label} must be an executable regular file"
}

if (($# != 1)); then
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

mapfile -t source_state < <("${SOURCE_STATE_CHECKER}" --root "${ROOT}")
((${#source_state[@]} == 2)) || fail "source-state checker returned malformed output"
[[ "${source_state[0]}" =~ ^source_commit$'\t'([0-9a-f]{40}|[0-9a-f]{64})$ ]] ||
  fail "source-state checker returned a malformed commit"
SOURCE_COMMIT="${BASH_REMATCH[1]}"
[[ "${source_state[1]}" =~ ^source_tree$'\t'([0-9a-f]{40}|[0-9a-f]{64})$ ]] ||
  fail "source-state checker returned a malformed tree"
SOURCE_TREE="${BASH_REMATCH[1]}"
readonly SOURCE_COMMIT SOURCE_TREE

readonly WORKER="${FE2O3_LLVM_LINK_WORKER:-}"
readonly WORKER_BUILD_ID="${FE2O3_LLVM_LINK_WORKER_BUILD_ID:-}"
readonly LLVM_BUILD_ID="${FE2O3_LLVM_BUILD_ID:-}"
canonical_executable Worker-V2 "${WORKER}"
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
readonly BUILD_TARGET="${ROOT}/target/s09-debug-hardware-${BASHPID}"
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
