#!/usr/bin/env bash

set -Eeuo pipefail

readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3.12
readonly DWARFDUMP=/opt/rocm/llvm/bin/llvm-dwarfdump
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly CHECKER="${SCRIPT_DIR}/s09-debug-check.py"
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

if (($# != 3)); then
  usage
  exit 2
fi

readonly HSACO="$1"
readonly HARDWARE_TEST="$2"
readonly ARCHIVE="$3"

canonical_file HSACO "${HSACO}"
canonical_file hardware-test "${HARDWARE_TEST}"
[[ -x "${HARDWARE_TEST}" ]] || fail "hardware-test must be executable"
[[ "$(basename -- "${HARDWARE_TEST}")" == gfx942_two_kernel_hardware-* ]] ||
  fail "hardware-test basename is outside the fixed S09 profile"
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
canonical_file checker "${CHECKER}"

umask 077
mkdir -- "${ARCHIVE}"
mkdir -- "${ARCHIVE}/tmp"

readonly DWARF_VERIFY_RAW="${ARCHIVE}/dwarf-verify.raw.txt"
readonly DWARF_RAW="${ARCHIVE}/dwarf.raw.txt"
readonly DWARF_NORMALIZED="${ARCHIVE}/dwarf.normalized.txt"
readonly ROCGDB_RAW="${ARCHIVE}/rocgdb.raw.txt"
readonly ROCGDB_NORMALIZED="${ARCHIVE}/rocgdb.normalized.txt"
readonly MANIFEST="${ARCHIVE}/manifest.txt"
HSACO_SHA256="$(sha256sum -- "${HSACO}" | awk '{print $1}')"
readonly HSACO_SHA256

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
set -e

if ((dwarf_verify_status == 0 && dwarf_dump_status == 0 && dwarf_normalize_status == 0 && dwarf_check_status == 0)); then
  set +e
  # ROCgdb, rather than Bash, evaluates the literal $pc expressions below.
  # shellcheck disable=SC2016
  /usr/bin/timeout --signal=TERM --kill-after=10s 180s \
    /usr/bin/env -i \
      HOME="${ARCHIVE}/tmp" \
      PATH=/opt/rocm/bin:/usr/bin:/bin \
      LD_LIBRARY_PATH=/opt/rocm/lib:/opt/rocm/lib64 \
      TMPDIR="${ARCHIVE}/tmp" \
      FE2O3_RUN_GFX942_TWO_KERNEL=1 \
      FE2O3_GFX942_ALPHA_ZETA_HSACO="${HSACO}" \
      FE2O3_GFX942_ALPHA_ZETA_SHA256="${HSACO_SHA256}" \
      "${ROCGDB}" --batch --nx --nh \
      -ex 'set confirm off' \
      -ex 'set pagination off' \
      -ex 'set width 0' \
      -ex 'set height 0' \
      -ex 'set auto-load off' \
      -ex 'set debuginfod enabled off' \
      -ex 'set startup-with-shell off' \
      -ex 'set breakpoint pending on' \
      -ex 'break alpha' \
      -ex 'echo FE2O3_S09_BEGIN\n' \
      -ex 'run' \
      -ex 'echo FE2O3_S09_FUNCTION\n' \
      -ex 'frame' \
      -ex 'info line *$pc' \
      -ex 'disable 1' \
      -ex 'break main.rs:69' \
      -ex 'continue' \
      -ex 'echo FE2O3_S09_ARGUMENTS\n' \
      -ex 'echo scale = ' -ex 'output scale' -ex 'echo \n' \
      -ex 'echo input_data = ' -ex 'output input_data' -ex 'echo \n' \
      -ex 'echo input_len = ' -ex 'output input_len' -ex 'echo \n' \
      -ex 'echo output_data = ' -ex 'output output_data' -ex 'echo \n' \
      -ex 'echo output_len = ' -ex 'output output_len' -ex 'echo \n' \
      -ex 'disable 2' \
      -ex 'break main.rs:70' \
      -ex 'continue' \
      -ex 'echo FE2O3_S09_LOCAL\n' \
      -ex 'frame' \
      -ex 'info line *$pc' \
      -ex 'echo i = ' -ex 'output i' -ex 'echo \n' \
      -ex 'echo FE2O3_S09_END\n' \
      --args "${HARDWARE_TEST}" \
        gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator \
        --ignored --exact --nocapture >"${ROCGDB_RAW}" 2>&1
  rocgdb_status=$?
  set -e
else
  printf 'FE2O3_S09_BEGIN\nDWARF prerequisite failed; ROCgdb was not run\nFE2O3_S09_END\n' >"${ROCGDB_RAW}"
  rocgdb_status=125
fi
set +e
"${CHECKER}" normalize-rocgdb --input "${ROCGDB_RAW}" --output "${ROCGDB_NORMALIZED}"
rocgdb_normalize_status=$?
if ((rocgdb_normalize_status == 0)); then
  "${CHECKER}" check-rocgdb --input "${ROCGDB_NORMALIZED}"
  rocgdb_check_status=$?
else
  rocgdb_check_status=1
fi
set -e

result=passed
for status in \
  "${dwarf_verify_status}" \
  "${dwarf_dump_status}" \
  "${dwarf_normalize_status}" \
  "${dwarf_check_status}" \
  "${rocgdb_status}" \
  "${rocgdb_normalize_status}" \
  "${rocgdb_check_status}"; do
  if ((status != 0)); then
    result=failed
  fi
done

{
  printf 'format=fe2o3-s09-debug-archive-v1\n'
  printf 'profile=%s\n' "${PROFILE}"
  printf 'result=%s\n' "${result}"
  printf 'target=gfx942:xnack-\n'
  printf 'optimization=O0\n'
  printf 'rocgdb=/opt/rocm/bin/rocgdb-py_3.12\n'
  printf 'llvm_dwarfdump=/opt/rocm/llvm/bin/llvm-dwarfdump\n'
  printf 'hsaco_sha256=%s\n' "${HSACO_SHA256}"
  printf 'hardware_test_sha256=%s\n' "$(hash_or_missing "${HARDWARE_TEST}")"
  printf 'checker_sha256=%s\n' "$(hash_or_missing "${CHECKER}")"
  printf 'dwarf_normalized_sha256=%s\n' "$(hash_or_missing "${DWARF_NORMALIZED}")"
  printf 'rocgdb_normalized_sha256=%s\n' "$(hash_or_missing "${ROCGDB_NORMALIZED}")"
  printf 'dwarf_verify_status=%d\n' "${dwarf_verify_status}"
  printf 'dwarf_dump_status=%d\n' "${dwarf_dump_status}"
  printf 'dwarf_normalize_status=%d\n' "${dwarf_normalize_status}"
  printf 'dwarf_check_status=%d\n' "${dwarf_check_status}"
  printf 'rocgdb_status=%d\n' "${rocgdb_status}"
  printf 'rocgdb_normalize_status=%d\n' "${rocgdb_normalize_status}"
  printf 'rocgdb_check_status=%d\n' "${rocgdb_check_status}"
} >"${MANIFEST}"

if [[ "${result}" != passed ]]; then
  printf 'S09 debug inspection failed closed; inspect %s\n' "${ARCHIVE}" >&2
  exit 1
fi

printf 'S09 debug inspection archive: %s\n' "${ARCHIVE}"
