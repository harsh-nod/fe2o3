#!/usr/bin/env bash

set -Eeuo pipefail

# This lane demonstrates local capability only. Its output cannot satisfy the
# protected S09 evidence policy without an externally pinned provenance manifest.
readonly S09_CLAIM="Manifest V2 capability-only local pilot"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly ROOT
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly CHECKER="${ROOT}/scripts/s09-debug-check.py"
readonly DWARFDUMP=/opt/rocm/llvm/bin/llvm-dwarfdump
readonly READOBJ=/opt/rocm/llvm/bin/llvm-readobj
readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3.12
readonly SOURCE_RELATIVE=crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs
readonly SOURCE="${ROOT}/${SOURCE_RELATIVE}"
readonly HARNESS_SOURCE="${ROOT}/crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs"

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

require_sha256() {
  local label="$1"
  local value="$2"
  [[ "${value}" =~ ^[0-9a-f]{64}$ && "${value}" != "$(printf '%064d' 0)" ]] ||
    fail "${label} must be a nonzero lowercase SHA-256 digest"
}

hash_file() {
  sha256sum -- "$1" | cut -d ' ' -f 1
}

archive_value() {
  local key="$1"
  local manifest="$2"
  local -a matches=()
  mapfile -t matches < <(awk -F= -v key="${key}" '$1 == key {sub(/^[^=]*=/, ""); print}' "${manifest}")
  ((${#matches[@]} == 1)) || fail "archive manifest field ${key} is absent or duplicated"
  [[ -n "${matches[0]}" ]] || fail "archive manifest field ${key} is empty"
  printf '%s' "${matches[0]}"
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
readonly PORTABLE_MIR_SHA256="${FE2O3_S09_PORTABLE_MIR_SHA256:-}"
readonly PORTABLE_ABI_SHA256="${FE2O3_S09_PORTABLE_ABI_SHA256:-}"
readonly CARGO_METADATA_SHA256="${FE2O3_S09_ORDERED_CARGO_METADATA_SHA256:-}"
readonly CRATE_BINDING_ID="${FE2O3_S09_CRATE_BINDING_ID:-}"
readonly KERNEL_BINDING_ID="${FE2O3_S09_KERNEL_BINDING_ID:-}"
readonly OBSERVED_DEF_PATH="${FE2O3_S09_OBSERVED_DEF_PATH:-}"
readonly OBSERVED_SYMBOL="${FE2O3_S09_OBSERVED_SYMBOL:-}"
readonly RUSTC_MIR_CAPTURE_SHA256="${FE2O3_S09_RUSTC_MIR_CAPTURE_SHA256:-}"
readonly CARGO_SHA256="${FE2O3_S09_CARGO_SHA256:-}"
readonly RUSTC_SHA256="${FE2O3_S09_RUSTC_SHA256:-}"
readonly BACKEND_SHA256="${FE2O3_S09_BACKEND_SHA256:-}"
readonly LLVM_SHA256="${FE2O3_S09_LLVM_SHA256:-}"
readonly LLD_SHA256="${FE2O3_S09_LLD_SHA256:-}"
canonical_executable Worker-V2 "${WORKER}"
executable_file llvm-dwarfdump "${DWARFDUMP}"
executable_file llvm-readobj "${READOBJ}"
executable_file ROCgdb "${ROCGDB}"
executable_file S09-runner "${RUNNER}"
[[ "${WORKER_BUILD_ID}" =~ ^fe2o3-worker-v1-sha256-[0-9a-f]{64}$ ]] ||
  fail "Worker V2 build identity is malformed"
[[ "${LLVM_BUILD_ID}" =~ ^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$ ]] ||
  fail "LLVM build identity is malformed"
for digest_input in \
  "portable MIR:${PORTABLE_MIR_SHA256}" \
  "portable ABI:${PORTABLE_ABI_SHA256}" \
  "ordered Cargo metadata:${CARGO_METADATA_SHA256}" \
  "crate binding:${CRATE_BINDING_ID}" \
  "kernel binding:${KERNEL_BINDING_ID}" \
  "exact rustc MIR capture:${RUSTC_MIR_CAPTURE_SHA256}" \
  "Cargo executable:${CARGO_SHA256}" \
  "rustc executable:${RUSTC_SHA256}" \
  "backend library:${BACKEND_SHA256}" \
  "LLVM toolchain:${LLVM_SHA256}" \
  "LLD executable:${LLD_SHA256}"; do
  require_sha256 "${digest_input%%:*}" "${digest_input#*:}"
done
[[ -n "${OBSERVED_DEF_PATH}" && -n "${OBSERVED_SYMBOL}" ]] ||
  fail "observed DefPath and symbol are required BuildObservationV2 fields"

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

readonly ARCHIVE_MANIFEST="${ARCHIVE}/manifest.txt"
readonly ARTIFACT_FACTS="${ARCHIVE}/artifact.facts.txt"
readonly HARDWARE_FACTS="${ARCHIVE}/hardware.facts.txt"
readonly DWARF_NORMALIZED="${ARCHIVE}/dwarf.normalized.txt"
readonly ROCGDB_NORMALIZED="${ARCHIVE}/rocgdb.normalized.txt"
readonly MANIFEST_V2="${EVIDENCE}/s09-evidence-manifest-v2.tsv"
for evidence_file in \
  "${ARCHIVE_MANIFEST}" \
  "${ARTIFACT_FACTS}" \
  "${HARDWARE_FACTS}" \
  "${DWARF_NORMALIZED}" \
  "${ROCGDB_NORMALIZED}"; do
  [[ -f "${evidence_file}" && ! -L "${evidence_file}" ]] ||
    fail "runner did not produce required evidence file ${evidence_file}"
done

SOURCE_COMMIT="$(git rev-parse --verify HEAD)"
SOURCE_TREE="$(git rev-parse --verify 'HEAD^{tree}')"
SOURCE_SHA256="$(hash_file "${SOURCE}")"
SOURCE_LENGTH="$(wc -c <"${SOURCE}")"
HSACO_SHA256="$(hash_file "${HSACO}")"
HOST_SHA256="$(archive_value hardware_test_sha256 "${ARCHIVE_MANIFEST}")"
HOST_BUILD_ID="$(archive_value hardware_test_build_id "${ARCHIVE_MANIFEST}")"
readonly SOURCE_COMMIT SOURCE_TREE SOURCE_SHA256 SOURCE_LENGTH HSACO_SHA256
readonly HOST_SHA256 HOST_BUILD_ID

{
  printf 'manifest_schema\tfe2o3-s09-protected-manifest-v2\n'
  printf 'trust_domain\tlocal-capability-v2\n'
  printf 'claim\tsource-debug-evidence-v2\n'
  printf 'semantic_admission_schema\tfe2o3-s09-semantic-admission-v2\n'
  printf 'source_path\t%s\n' "${SOURCE_RELATIVE}"
  printf 'source_sha256\t%s\n' "${SOURCE_SHA256}"
  printf 'source_length\t%s\n' "${SOURCE_LENGTH}"
  printf 'logical_crate\tfe2o3_typed_alias_spoof\n'
  printf 'logical_module\tgeneral_genuine\n'
  printf 'logical_kernel\talpha\n'
  printf 'logical_export\talpha\n'
  printf 'logical_owner\tfe2o3_typed_alias_spoof::general_genuine::alpha\n'
  printf 'owner_authentication\tcollector-authenticated-kernel-owner-v1\n'
  printf 'profile\tgeneral-scalar-slice-v3\n'
  printf 'portable_mir_sha256\t%s\n' "${PORTABLE_MIR_SHA256}"
  printf 'portable_abi_sha256\t%s\n' "${PORTABLE_ABI_SHA256}"
  printf 'target\tgfx942:xnack-\n'
  printf 'optimization\tO0\n'
  printf 'code_object_version\t6\n'
  printf 'target_policy\tgfx942:xnack-/cov6/o0/source-debug-v1\n'
  printf 'debug_policy\ts09-alpha-source-dwarf-v1\n'
  printf 'build_observation_schema\tfe2o3-s09-build-observation-v2\n'
  printf 'source_commit\t%s\n' "${SOURCE_COMMIT}"
  printf 'source_tree\t%s\n' "${SOURCE_TREE}"
  printf 'ordered_cargo_metadata_sha256\t%s\n' "${CARGO_METADATA_SHA256}"
  printf 'crate_binding_id\t%s\n' "${CRATE_BINDING_ID}"
  printf 'kernel_binding_id\t%s\n' "${KERNEL_BINDING_ID}"
  printf 'observed_def_path\t%s\n' "${OBSERVED_DEF_PATH}"
  printf 'observed_symbol\t%s\n' "${OBSERVED_SYMBOL}"
  printf 'rustc_mir_capture_sha256\t%s\n' "${RUSTC_MIR_CAPTURE_SHA256}"
  printf 'cargo_sha256\t%s\n' "${CARGO_SHA256}"
  printf 'rustc_sha256\t%s\n' "${RUSTC_SHA256}"
  printf 'backend_sha256\t%s\n' "${BACKEND_SHA256}"
  printf 'llvm_sha256\t%s\n' "${LLVM_SHA256}"
  printf 'llvm_link_worker_sha256\t%s\n' "$(hash_file "${WORKER}")"
  printf 'lld_sha256\t%s\n' "${LLD_SHA256}"
  printf 'llvm_dwarfdump_sha256\t%s\n' "$(hash_file "${DWARFDUMP}")"
  printf 'llvm_readobj_sha256\t%s\n' "$(hash_file "${READOBJ}")"
  printf 'rocgdb_sha256\t%s\n' "$(hash_file "${ROCGDB}")"
  printf 'checker_sha256\t%s\n' "$(hash_file "${CHECKER}")"
  printf 'harness_source_sha256\t%s\n' "$(hash_file "${HARNESS_SOURCE}")"
  printf 'hsaco_sha256\t%s\n' "${HSACO_SHA256}"
  printf 'host_executable_sha256\t%s\n' "${HOST_SHA256}"
  printf 'host_executable_build_id\t%s\n' "${HOST_BUILD_ID}"
  printf 'debug_archive_manifest_sha256\t%s\n' "$(hash_file "${ARCHIVE_MANIFEST}")"
  printf 'artifact_facts_sha256\t%s\n' "$(hash_file "${ARTIFACT_FACTS}")"
  printf 'hardware_facts_sha256\t%s\n' "$(hash_file "${HARDWARE_FACTS}")"
  printf 'dwarf_normalized_sha256\t%s\n' "$(hash_file "${DWARF_NORMALIZED}")"
  printf 'rocgdb_normalized_sha256\t%s\n' "$(hash_file "${ROCGDB_NORMALIZED}")"
  printf 'hardware_test\tgfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator\n'
  printf 'execution_closure\tlocal-capability-v2\n'
} >"${MANIFEST_V2}"

MANIFEST_V2_SHA256="$(hash_file "${MANIFEST_V2}")"
readonly MANIFEST_V2_SHA256
"${CHECKER}" check-capability \
  --manifest "${MANIFEST_V2}" \
  --expected-manifest-sha256 "${MANIFEST_V2_SHA256}" \
  --artifact-facts "${ARTIFACT_FACTS}" \
  --hardware-facts "${HARDWARE_FACTS}" \
  --dwarf "${DWARF_NORMALIZED}" \
  --rocgdb "${ROCGDB_NORMALIZED}"

rg -q $'^supplemental\tS09\tMissing$' docs/cuda-oxide-parity-status.tsv ||
  fail "S09 parity status must remain Missing"
printf 'S09 %s output: %s (manifest %s)\n' \
  "${S09_CLAIM}" "${EVIDENCE}" "${MANIFEST_V2_SHA256}"
