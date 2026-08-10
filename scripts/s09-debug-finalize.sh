#!/usr/bin/env bash

set -Eeuo pipefail
export PYTHONDONTWRITEBYTECODE=1

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly ROOT
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly CHECKER="${ROOT}/scripts/s09-debug-check.py"
readonly PINNER="${ROOT}/scripts/s09_pinned_snapshot.py"
readonly SOURCE_STATE_CHECKER="${ROOT}/scripts/s09-source-state.py"
readonly S09_CLAIM="Manifest V2 capability-only local pilot"

fail() {
  printf 's09-debug-finalize: %s\n' "$1" >&2
  exit 2
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

if (($# != 5)); then
  fail "usage: scripts/s09-debug-finalize.sh EVIDENCE PINNED-HSACO PINNED-HOST COMMIT TREE"
fi
readonly EVIDENCE="$1"
readonly HSACO="$2"
readonly HARDWARE_TEST="$3"
readonly SOURCE_COMMIT="$4"
readonly SOURCE_TREE="$5"
[[ "${EVIDENCE}" == /* && -d "${EVIDENCE}" && ! -L "${EVIDENCE}" ]] ||
  fail "evidence directory must be an absolute real directory"
[[ "$(stat -c '%u:%a' -- "${EVIDENCE}")" == "${EUID}:700" ]] ||
  fail "evidence directory must be owned by the caller with mode 0700"
[[ "${SOURCE_COMMIT}" =~ ^[0-9a-f]{40}$|^[0-9a-f]{64}$ ]] || fail "source commit is malformed"
[[ "${SOURCE_TREE}" =~ ^[0-9a-f]{40}$|^[0-9a-f]{64}$ ]] || fail "source tree is malformed"
"${PINNER}" --verify-only \
  --input "hsaco=${HSACO}" \
  --input "host=${HARDWARE_TEST}" \
  --executable host
"${SOURCE_STATE_CHECKER}" \
  --root "${ROOT}" \
  --expected-commit "${SOURCE_COMMIT}" \
  --expected-tree "${SOURCE_TREE}" >/dev/null

readonly ARCHIVE="${EVIDENCE}/rocgdb-archive"
readonly RETAINED_HOST="${EVIDENCE}/s09-host-executable.bin"
"${PINNER}" \
  --input "host=${HARDWARE_TEST}" \
  --executable host \
  --export "host=${RETAINED_HOST}"
[[ "$(stat -c '%u:%a' -- "${RETAINED_HOST}")" == "${EUID}:400" ]] ||
  fail "retained host artifact must be caller-owned with mode 0400"
"${RUNNER}" --pinned-profile "${HSACO}" "${HARDWARE_TEST}" "${ARCHIVE}"

readonly ARCHIVE_MANIFEST="${ARCHIVE}/manifest.txt"
readonly ARTIFACT_FACTS="${ARCHIVE}/artifact.facts.txt"
readonly HARDWARE_FACTS="${ARCHIVE}/hardware.facts.txt"
readonly DWARF_NORMALIZED="${ARCHIVE}/dwarf.normalized.txt"
readonly ROCGDB_NORMALIZED="${ARCHIVE}/rocgdb.normalized.txt"
readonly MANIFEST_V2="${EVIDENCE}/s09-evidence-manifest-v2.tsv"
readonly IDENTITY_FIELDS="${EVIDENCE}/s09-identity-fields-v2.tsv"
for evidence_file in \
  "${ARCHIVE_MANIFEST}" \
  "${ARTIFACT_FACTS}" \
  "${HARDWARE_FACTS}" \
  "${DWARF_NORMALIZED}" \
  "${ROCGDB_NORMALIZED}"; do
  [[ -f "${evidence_file}" && ! -L "${evidence_file}" ]] ||
    fail "runner did not produce required evidence file ${evidence_file}"
done
[[ -f "${RETAINED_HOST}" && ! -L "${RETAINED_HOST}" ]] ||
  fail "retained host artifact is missing or not a regular file"

HSACO_SHA256="$(hash_file "${HSACO}")"
HOST_SHA256="$(hash_file "${RETAINED_HOST}")"
HOST_BUILD_ID="$(archive_value hardware_test_build_id "${ARCHIVE_MANIFEST}")"
readonly HSACO_SHA256 HOST_SHA256 HOST_BUILD_ID
[[ "$(archive_value hardware_test_sha256 "${ARCHIVE_MANIFEST}")" == "${HOST_SHA256}" ]] ||
  fail "retained host artifact does not match the executed image"

"${CHECKER}" identity-fields --hsaco "${HSACO}" --output "${IDENTITY_FIELDS}"

{
  printf 'manifest_schema\tfe2o3-s09-protected-manifest-v2\n'
  printf 'trust_domain\tlocal-capability-v2\n'
  printf 'claim\tsource-debug-evidence-v2\n'
  cat -- "${IDENTITY_FIELDS}"
  printf 'source_commit\t%s\n' "${SOURCE_COMMIT}"
  printf 'source_tree\t%s\n' "${SOURCE_TREE}"
  printf 'hsaco_sha256\t%s\n' "${HSACO_SHA256}"
  printf 'host_executable_sha256\t%s\n' "${HOST_SHA256}"
  printf 'host_executable_build_id\t%s\n' "${HOST_BUILD_ID}"
  printf 'debug_archive_manifest_sha256\t%s\n' "$(hash_file "${ARCHIVE_MANIFEST}")"
  printf 'artifact_facts_sha256\t%s\n' "$(hash_file "${ARTIFACT_FACTS}")"
  printf 'hardware_facts_sha256\t%s\n' "$(hash_file "${HARDWARE_FACTS}")"
  printf 'dwarf_normalized_sha256\t%s\n' "$(hash_file "${DWARF_NORMALIZED}")"
  printf 'rocgdb_normalized_sha256\t%s\n' "$(hash_file "${ROCGDB_NORMALIZED}")"
} >"${MANIFEST_V2}"

MANIFEST_V2_SHA256="$(hash_file "${MANIFEST_V2}")"
readonly MANIFEST_V2_SHA256
"${CHECKER}" check-capability \
  --manifest "${MANIFEST_V2}" \
  --expected-manifest-sha256 "${MANIFEST_V2_SHA256}" \
  --hsaco "${HSACO}" \
  --host-executable "${RETAINED_HOST}" \
  --debug-archive-manifest "${ARCHIVE_MANIFEST}" \
  --artifact-facts "${ARTIFACT_FACTS}" \
  --hardware-facts "${HARDWARE_FACTS}" \
  --dwarf "${DWARF_NORMALIZED}" \
  --rocgdb "${ROCGDB_NORMALIZED}"

"${SOURCE_STATE_CHECKER}" \
  --root "${ROOT}" \
  --expected-commit "${SOURCE_COMMIT}" \
  --expected-tree "${SOURCE_TREE}" >/dev/null
rg -q $'^supplemental\tS09\tPartial$' "${ROOT}/docs/cuda-oxide-parity-status.tsv" ||
  fail "S09 parity status must remain Partial until production-v2 evidence qualifies"
printf 'S09 %s output: %s (manifest %s)\n' \
  "${S09_CLAIM}" "${EVIDENCE}" "${MANIFEST_V2_SHA256}"
