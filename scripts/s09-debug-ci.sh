#!/usr/bin/env bash

set -Eeuo pipefail

# This lane demonstrates local capability only. Its output cannot satisfy the
# protected S09 evidence policy without an externally pinned provenance manifest.
readonly S09_CLAIM="capability-only local pilot"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly ROOT
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly DWARFDUMP=/opt/rocm/llvm/bin/llvm-dwarfdump

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

readonly WORKER="${FE2O3_LLVM_LINK_WORKER:-}"
readonly WORKER_BUILD_ID="${FE2O3_LLVM_LINK_WORKER_BUILD_ID:-}"
readonly LLVM_BUILD_ID="${FE2O3_LLVM_BUILD_ID:-}"
canonical_executable Worker-V2 "${WORKER}"
executable_file llvm-dwarfdump "${DWARFDUMP}"
executable_file S09-runner "${RUNNER}"
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

mkdir -- "${EVIDENCE}"
readonly HSACO="${EVIDENCE}/alpha-debug.hsaco"
readonly ARCHIVE="${EVIDENCE}/rocgdb-archive"

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
  --test gfx942_two_kernel_hardware \
  --target-dir "${BUILD_TARGET}" \
  --no-run

mapfile -d '' -t hardware_tests < <(
  find "${BUILD_TARGET}/debug/deps" -maxdepth 1 -type f -perm -0100 \
    -name 'gfx942_two_kernel_hardware-*' -print0
)
if ((${#hardware_tests[@]} != 1)); then
  fail "isolated build produced ${#hardware_tests[@]} candidate hardware executables"
fi
HARDWARE_TEST="$(realpath --canonicalize-existing -- "${hardware_tests[0]}")"
readonly HARDWARE_TEST

"${RUNNER}" "${HSACO}" "${HARDWARE_TEST}" "${ARCHIVE}"
rg -q $'^supplemental\tS09\tMissing$' docs/cuda-oxide-parity-status.tsv ||
  fail "S09 parity status must remain Missing"
printf 'S09 %s output: %s\n' "${S09_CLAIM}" "${EVIDENCE}"
