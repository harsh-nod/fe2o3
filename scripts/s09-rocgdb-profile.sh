#!/usr/bin/env bash

set -Eeuo pipefail
export PYTHONDONTWRITEBYTECODE=1

readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3.12
readonly DWARFDUMP=/opt/rocm/llvm/bin/llvm-dwarfdump
readonly READOBJ=/opt/rocm/llvm/bin/llvm-readobj
readonly READELF=/opt/rocm/llvm/bin/llvm-readobj
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly CHECKER="${SCRIPT_DIR}/s09-debug-check.py"
readonly PINNER="${SCRIPT_DIR}/s09_pinned_snapshot.py"
readonly PROFILE=s09-alpha-gfx942-o0-v1

usage() {
  printf 'Usage: %s ABSOLUTE-HSACO ABSOLUTE-HARDWARE-TEST ABSOLUTE-NEW-ARCHIVE\n' "$0" >&2
}

fail() {
  printf 's09-rocgdb-profile: %s\n' "$1" >&2
  exit 2
}

canonical_file() {
  local label="$1"
  local path="$2"
  [[ "${path}" == /* ]] || fail "${label} must be an absolute path"
  [[ -f "${path}" && ! -L "${path}" ]] || fail "${label} must be a regular non-symlink file"
  [[ "$(realpath --canonicalize-existing -- "${path}")" == "${path}" ]] ||
    fail "${label} must already be canonical"
}

pinned_tool() {
  local label="$1"
  local path="$2"
  [[ -f "${path}" && ! -L "${path}" && -x "${path}" ]] ||
    fail "${label} pin is not an executable regular file"
}

hash_or_missing() {
  local path="$1"
  if [[ -f "${path}" ]]; then
    sha256sum -- "${path}" | awk '{print $1}'
  else
    printf 'missing\n'
  fi
}

mode=outer
if [[ "${1:-}" == --pinned-profile ]]; then
  mode=pinned-profile
  shift
fi
readonly mode

if (($# != 3)); then
  usage
  exit 2
fi

readonly HSACO="$1"
readonly HARDWARE_TEST="$2"
readonly ARCHIVE="$3"

[[ "${ARCHIVE}" == /* ]] || fail "archive must be an absolute path"
[[ ! -e "${ARCHIVE}" && ! -L "${ARCHIVE}" ]] || fail "archive must not already exist"
ARCHIVE_PARENT="$(dirname -- "${ARCHIVE}")"
readonly ARCHIVE_PARENT
[[ -d "${ARCHIVE_PARENT}" && ! -L "${ARCHIVE_PARENT}" ]] ||
  fail "archive parent must be a real directory"
[[ "$(realpath --canonicalize-existing -- "${ARCHIVE_PARENT}")" == "${ARCHIVE_PARENT}" ]] ||
  fail "archive parent must already be canonical"
pinned_tool ROCgdb "${ROCGDB}"
pinned_tool llvm-dwarfdump "${DWARFDUMP}"
pinned_tool llvm-readobj "${READOBJ}"
pinned_tool llvm-readelf "${READELF}"
canonical_file checker "${CHECKER}"
pinned_tool snapshot-supervisor "${PINNER}"

if [[ "${mode}" == outer ]]; then
  canonical_file HSACO "${HSACO}"
  canonical_file hardware-test "${HARDWARE_TEST}"
  [[ -x "${HARDWARE_TEST}" ]] || fail "hardware-test must be executable"
  [[ "$(basename -- "${HARDWARE_TEST}")" == s09_gfx942_alpha_hardware-* ]] ||
    fail "hardware-test basename is outside the fixed S09 profile"
  exec "${PINNER}" \
    --input "hsaco=${HSACO}" \
    --input "host=${HARDWARE_TEST}" \
    --executable host \
    -- "${BASH_SOURCE[0]}" --pinned-profile '{hsaco}' '{host}' "${ARCHIVE}"
fi

"${PINNER}" --verify-only \
  --input "hsaco=${HSACO}" \
  --input "host=${HARDWARE_TEST}" \
  --executable host

HSACO_SHA256="$(sha256sum -- "${HSACO}" | awk '{print $1}')"
readonly HSACO_SHA256
HARDWARE_SHA256="$(sha256sum -- "${HARDWARE_TEST}" | awk '{print $1}')"
readonly HARDWARE_SHA256
RUN_NONCE="$(/usr/bin/od -An -N32 -tx1 /dev/urandom | /usr/bin/tr -d ' \n')"
readonly RUN_NONCE
[[ "${RUN_NONCE}" =~ ^[0-9a-f]{64}$ ]] || fail "could not generate a bounded run nonce"

umask 077
mkdir -- "${ARCHIVE}"
mkdir -- "${ARCHIVE}/tmp"

readonly DWARF_VERIFY_RAW="${ARCHIVE}/dwarf-verify.raw.txt"
readonly DWARF_RAW="${ARCHIVE}/dwarf.raw.txt"
readonly DWARF_NORMALIZED="${ARCHIVE}/dwarf.normalized.txt"
readonly ARTIFACT_RAW="${ARCHIVE}/artifact.raw.txt"
readonly ARTIFACT_FACTS="${ARCHIVE}/artifact.facts.txt"
readonly HARDWARE_RAW="${ARCHIVE}/hardware.raw.txt"
readonly HARDWARE_FACTS="${ARCHIVE}/hardware.facts.txt"
readonly ROCGDB_RAW="${ARCHIVE}/rocgdb.raw.txt"
readonly ROCGDB_NORMALIZED="${ARCHIVE}/rocgdb.normalized.txt"
readonly MANIFEST="${ARCHIVE}/manifest.txt"

set +e
"${DWARFDUMP}" --verify "${HSACO}" >"${DWARF_VERIFY_RAW}" 2>&1
dwarf_verify_status=$?
"${DWARFDUMP}" --debug-info --debug-line "${HSACO}" >"${DWARF_RAW}" 2>&1
dwarf_dump_status=$?
"${CHECKER}" normalize-dwarf --input "${DWARF_RAW}" --output "${DWARF_NORMALIZED}"
dwarf_normalize_status=$?
if ((dwarf_normalize_status == 0)); then
  "${CHECKER}" check-dwarf --input "${DWARF_NORMALIZED}"
  dwarf_check_status=$?
else
  dwarf_check_status=1
fi
"${READOBJ}" --file-headers --notes "${HSACO}" >"${ARTIFACT_RAW}" 2>&1
artifact_read_status=$?
if ((artifact_read_status == 0 && dwarf_dump_status == 0)); then
  "${CHECKER}" artifact-facts \
    --metadata "${ARTIFACT_RAW}" \
    --dwarf "${DWARF_RAW}" \
    --output "${ARTIFACT_FACTS}"
  artifact_check_status=$?
else
  artifact_check_status=1
fi
"${READELF}" --elf-output-style=GNU --file-header --notes "${HARDWARE_TEST}" >"${HARDWARE_RAW}" 2>&1
hardware_read_status=$?
if ((hardware_read_status == 0)); then
  "${CHECKER}" hardware-facts \
    --input "${HARDWARE_RAW}" \
    --sha256 "${HARDWARE_SHA256}" \
    --output "${HARDWARE_FACTS}"
  hardware_check_status=$?
else
  hardware_check_status=1
fi
set -e

artifact_target=unavailable
artifact_optimization=unavailable
hardware_build_id=unavailable
if ((artifact_check_status == 0)); then
  artifact_target="$(sed -n 's/^target=//p' "${ARTIFACT_FACTS}")"
  artifact_optimization="$(sed -n 's/^optimization=//p' "${ARTIFACT_FACTS}")"
fi
if ((hardware_check_status == 0)); then
  hardware_build_id="$(sed -n 's/^build_id=//p' "${HARDWARE_FACTS}")"
fi
readonly artifact_target artifact_optimization hardware_build_id

artifact_facts_sha256="$(hash_or_missing "${ARTIFACT_FACTS}")"
readonly artifact_facts_sha256

if ((dwarf_verify_status == 0 && dwarf_dump_status == 0 && dwarf_normalize_status == 0 && dwarf_check_status == 0 && artifact_read_status == 0 && artifact_check_status == 0 && hardware_read_status == 0 && hardware_check_status == 0)); then
  set +e
  # ROCgdb, rather than Bash, evaluates the literal $pc expressions below.
  # shellcheck disable=SC2016
  "${PINNER}" \
    --input "hsaco=${HSACO}" \
    --input "host=${HARDWARE_TEST}" \
    --input "facts=${ARTIFACT_FACTS}" \
    --executable host \
    -- /usr/bin/timeout --signal=TERM --kill-after=10s 180s \
    /usr/bin/env -i \
      HOME="${ARCHIVE}/tmp" \
      PATH=/opt/rocm/bin:/usr/bin:/bin \
      LD_LIBRARY_PATH=/opt/rocm/lib:/opt/rocm/lib64 \
      TMPDIR="${ARCHIVE}/tmp" \
      FE2O3_RUN_S09_GFX942_ALPHA=1 \
      FE2O3_S09_GFX942_ALPHA_HSACO='{hsaco}' \
      FE2O3_S09_GFX942_ALPHA_SHA256="${HSACO_SHA256}" \
      FE2O3_S09_GFX942_ALPHA_FACTS='{facts}' \
      FE2O3_S09_GFX942_ALPHA_FACTS_SHA256="${artifact_facts_sha256}" \
      FE2O3_S09_RUN_NONCE="${RUN_NONCE}" \
      "${ROCGDB}" --batch --nx --nh --return-child-result \
      -ex 'set confirm off' \
      -ex 'set pagination off' \
      -ex 'set width 0' \
      -ex 'set height 0' \
      -ex 'set auto-load off' \
      -ex 'set debuginfod enabled off' \
      -ex 'set startup-with-shell off' \
      -ex 'set breakpoint pending on' \
      -ex 'echo FE2O3_S09_BEGIN\n' \
      -ex 'echo FE2O3_S09_BINDING\n' \
      -ex "echo hsaco_sha256 = ${HSACO_SHA256}\\n" \
      -ex "echo hardware_sha256 = ${HARDWARE_SHA256}\\n" \
      -ex "echo hardware_build_id = ${hardware_build_id}\\n" \
      -ex "echo run_nonce = ${RUN_NONCE}\\n" \
      -ex "echo target = ${artifact_target}\\n" \
      -ex "echo optimization = ${artifact_optimization}\\n" \
      -ex 'echo FE2O3_S09_KERNEL_LOAD\n' \
      -ex 'break alpha' \
      -ex 'echo FE2O3_S09_GPU_CONTEXT\n' \
      -ex 'run' \
      -ex 'info threads' \
      -ex 'info sharedlibrary memory://' \
      -ex 'echo FE2O3_S09_FUNCTION\n' \
      -ex 'frame' \
      -ex 'info line *$pc' \
      -ex 'disable 1' \
      -ex 'break main.rs:69' \
      -ex 'echo FE2O3_S09_BP2_ARMED\n' \
      -ex 'continue' \
      -ex 'echo FE2O3_S09_BP2_STOP\n' \
      -ex 'info threads' \
      -ex 'frame' \
      -ex 'echo FE2O3_S09_ARGUMENTS\n' \
      -ex 'echo scale = ' -ex 'output scale' -ex 'echo \n' \
      -ex 'echo input_data = ' -ex 'output input_data' -ex 'echo \n' \
      -ex 'echo input_len = ' -ex 'output input_len' -ex 'echo \n' \
      -ex 'echo output_data = ' -ex 'output output_data' -ex 'echo \n' \
      -ex 'echo output_len = ' -ex 'output output_len' -ex 'echo \n' \
      -ex 'disable 2' \
      -ex 'break main.rs:70' \
      -ex 'echo FE2O3_S09_BP3_ARMED\n' \
      -ex 'continue' \
      -ex 'echo FE2O3_S09_BP3_STOP\n' \
      -ex 'info threads' \
      -ex 'echo FE2O3_S09_LOCAL\n' \
      -ex 'frame' \
      -ex 'info line *$pc' \
      -ex 'echo i = ' -ex 'output i' -ex 'echo \n' \
      -ex 'disable 3' \
      -ex 'echo FE2O3_S09_RESUME\n' \
      -ex 'continue' \
      --args '{host}' \
        s09_gfx942_cov6_alpha_only_controller \
        --ignored --exact --nocapture >"${ROCGDB_RAW}" 2>&1
  rocgdb_status=$?
  set -e
else
  printf 'FE2O3_S09_BEGIN\nDWARF or identity prerequisite failed; ROCgdb was not run\n' >"${ROCGDB_RAW}"
  rocgdb_status=125
fi
if ((rocgdb_status == 0)); then
  printf 'FE2O3_S09_HARNESS_RESULT_V1 hsaco_sha256=%s run_nonce=%s result=passed\n' \
    "${HSACO_SHA256}" "${RUN_NONCE}" >>"${ROCGDB_RAW}"
  printf 'FE2O3_S09_HARDWARE_PASS\n' >>"${ROCGDB_RAW}"
fi
printf 'FE2O3_S09_ROCGDB_EXIT_STATUS = %d\nFE2O3_S09_END\n' \
  "${rocgdb_status}" >>"${ROCGDB_RAW}"
set +e
"${CHECKER}" normalize-rocgdb --input "${ROCGDB_RAW}" --output "${ROCGDB_NORMALIZED}"
rocgdb_normalize_status=$?
if ((rocgdb_normalize_status == 0)); then
  "${CHECKER}" check-rocgdb \
    --input "${ROCGDB_NORMALIZED}" \
    --hsaco-sha256 "${HSACO_SHA256}" \
    --hardware-sha256 "${HARDWARE_SHA256}" \
    --hardware-build-id "${hardware_build_id}"
  rocgdb_check_status=$?
else
  rocgdb_check_status=1
fi
set -e
ROCGDB_RAW_SHA256="$(hash_or_missing "${ROCGDB_RAW}")"
readonly ROCGDB_RAW_SHA256
rm -f -- "${ROCGDB_RAW}"

result=passed
for status in \
  "${dwarf_verify_status}" \
  "${dwarf_dump_status}" \
  "${dwarf_normalize_status}" \
  "${dwarf_check_status}" \
  "${artifact_read_status}" \
  "${artifact_check_status}" \
  "${hardware_read_status}" \
  "${hardware_check_status}" \
  "${rocgdb_status}" \
  "${rocgdb_normalize_status}" \
  "${rocgdb_check_status}"; do
  if ((status != 0)); then
    result=failed
  fi
done

{
  printf 'format=fe2o3-s09-debug-archive-v2\n'
  printf 'profile=%s\n' "${PROFILE}"
  printf 'result=%s\n' "${result}"
  printf 'target=%s\n' "${artifact_target}"
  printf 'optimization=%s\n' "${artifact_optimization}"
  printf 'rocgdb=/opt/rocm/bin/rocgdb-py_3.12\n'
  printf 'llvm_dwarfdump=/opt/rocm/llvm/bin/llvm-dwarfdump\n'
  printf 'llvm_readobj=/opt/rocm/llvm/bin/llvm-readobj\n'
  printf 'llvm_readelf=/opt/rocm/llvm/bin/llvm-readobj --elf-output-style=GNU\n'
  printf 'hsaco_sha256=%s\n' "${HSACO_SHA256}"
  printf 'hardware_test_sha256=%s\n' "${HARDWARE_SHA256}"
  printf 'hardware_test_build_id=%s\n' "${hardware_build_id}"
  printf 'run_nonce=%s\n' "${RUN_NONCE}"
  printf 'checker_sha256=%s\n' "$(hash_or_missing "${CHECKER}")"
  printf 'artifact_facts_sha256=%s\n' "${artifact_facts_sha256}"
  printf 'hardware_facts_sha256=%s\n' "$(hash_or_missing "${HARDWARE_FACTS}")"
  printf 'dwarf_normalized_sha256=%s\n' "$(hash_or_missing "${DWARF_NORMALIZED}")"
  printf 'rocgdb_normalized_sha256=%s\n' "$(hash_or_missing "${ROCGDB_NORMALIZED}")"
  printf 'rocgdb_raw_sha256=%s\n' "${ROCGDB_RAW_SHA256}"
  printf 'rocgdb_raw_retained=false\n'
  printf 'dwarf_verify_status=%d\n' "${dwarf_verify_status}"
  printf 'dwarf_dump_status=%d\n' "${dwarf_dump_status}"
  printf 'dwarf_normalize_status=%d\n' "${dwarf_normalize_status}"
  printf 'dwarf_check_status=%d\n' "${dwarf_check_status}"
  printf 'artifact_read_status=%d\n' "${artifact_read_status}"
  printf 'artifact_check_status=%d\n' "${artifact_check_status}"
  printf 'hardware_read_status=%d\n' "${hardware_read_status}"
  printf 'hardware_check_status=%d\n' "${hardware_check_status}"
  printf 'rocgdb_status=%d\n' "${rocgdb_status}"
  printf 'rocgdb_normalize_status=%d\n' "${rocgdb_normalize_status}"
  printf 'rocgdb_check_status=%d\n' "${rocgdb_check_status}"
} >"${MANIFEST}"

if [[ "${result}" != passed ]]; then
  printf 'S09 debug inspection failed closed; inspect %s\n' "${ARCHIVE}" >&2
  exit 1
fi

printf 'S09 debug inspection archive: %s\n' "${ARCHIVE}"
