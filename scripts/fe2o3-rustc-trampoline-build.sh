#!/bin/bash

set -Eeuo pipefail
umask 077
IFS=$' \t\n'
unset CDPATH GLOBIGNORE

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

script_parent="${BASH_SOURCE[0]%/*}"
[[ "${script_parent}" != "${BASH_SOURCE[0]}" ]] || script_parent=.
SCRIPT_DIR="$(cd -- "${script_parent}" && pwd -P)"
readonly SCRIPT_DIR
readonly SOURCE="${SCRIPT_DIR}/fe2o3-rustc-trampoline.c"

mode=build
candidate=
if (($# == 1)) && [[ "$1" == /* ]]; then
  candidate="$1"
elif (($# == 2)) && [[ "$1" == --verify && "$2" == /* ]]; then
  mode=verify
  candidate="$2"
else
  printf 'usage: %s [--verify] /absolute/path\n' "$0" >&2
  exit 2
fi
readonly MODE="${mode}"
readonly CANDIDATE="${candidate}"
readonly CANDIDATE_PARENT="${CANDIDATE%/*}"
readonly CANDIDATE_NAME="${CANDIDATE##*/}"
if [[ -z "${CANDIDATE_PARENT}" || -z "${CANDIDATE_NAME}" ||
  "${CANDIDATE_NAME}" == . || "${CANDIDATE_NAME}" == .. ]]; then
  fail 'candidate path must name a file in an absolute directory'
fi
if [[ ! -d "${CANDIDATE_PARENT}" || -L "${CANDIDATE_PARENT}" ]]; then
  fail 'candidate directory must be an existing non-symlink directory'
fi

run_clean() {
  /usr/bin/env -i \
    HOME=/nonexistent \
    LANG=C \
    LC_ALL=C \
    PATH=/usr/bin:/bin \
    SOURCE_DATE_EPOCH=0 \
    TMPDIR="${CANDIDATE_PARENT}" \
    TZ=UTC \
    ZERO_AR_DATE=1 \
    "$@"
}

canonical_parent="$({
  run_clean /usr/bin/readlink --canonicalize-existing -- "${CANDIDATE_PARENT}"
} 2>/dev/null)" || fail 'cannot resolve candidate directory'
readonly CANONICAL_PARENT="${canonical_parent}"
[[ "${CANONICAL_PARENT}" == "${CANDIDATE_PARENT}" ]] ||
  fail 'candidate directory path must be canonical and contain no symlinks'

output_directory_fd=
exec {output_directory_fd}<"${CANONICAL_PARENT}" ||
  fail 'cannot retain candidate directory'
readonly OUTPUT_DIRECTORY_FD="${output_directory_fd}"
readonly OUTPUT_DIRECTORY_REF="/proc/self/fd/${OUTPUT_DIRECTORY_FD}"
output_directory_identity="$({
  run_clean /usr/bin/stat --dereference --format='%d:%i' \
    -- "${OUTPUT_DIRECTORY_REF}"
} 2>/dev/null)" || fail 'cannot identify retained candidate directory'
readonly OUTPUT_DIRECTORY_IDENTITY="${output_directory_identity}"
output_directory_policy="$({
  run_clean /usr/bin/stat --dereference --format='%u:%a:%F' \
    -- "${OUTPUT_DIRECTORY_REF}"
} 2>/dev/null)" || fail 'cannot inspect retained candidate directory'
readonly OUTPUT_DIRECTORY_POLICY="${output_directory_policy}"
[[ "${OUTPUT_DIRECTORY_POLICY}" == "${EUID}:700:directory" ]] ||
  fail 'candidate directory must be caller-owned mode 0700'

require_output_directory_identity() {
  local retained current
  retained="$({
    run_clean /usr/bin/stat --dereference --format='%d:%i' \
      -- "${OUTPUT_DIRECTORY_REF}"
  } 2>/dev/null)" || fail 'retained candidate directory became unavailable'
  current="$({
    run_clean /usr/bin/stat --dereference --format='%d:%i' \
      -- "${CANONICAL_PARENT}"
  } 2>/dev/null)" || fail 'candidate directory path became unavailable'
  [[ "${retained}" == "${OUTPUT_DIRECTORY_IDENTITY}" ]] ||
    fail 'retained candidate directory identity changed'
  [[ "${current}" == "${OUTPUT_DIRECTORY_IDENTITY}" ]] ||
    fail 'candidate directory path was replaced during the operation'
}

require_header_value() {
  local header="$1"
  local label="$2"
  local expected="$3"
  local observed
  # shellcheck disable=SC2016
  observed="$(
    printf '%s\n' "${header}" |
      run_clean /usr/bin/awk -F: -v label="${label}" \
        '$1 ~ "^[[:space:]]*" label "$" { sub(/^[[:space:]]+/, "", $2); print $2 }'
  )"
  [[ "${observed}" == "${expected}" ]] ||
    fail "rustc trampoline ELF ${label} is '${observed}', expected '${expected}'"
}

verify_elf() {
  local executable="$1"
  local header program_headers dynamic_tags mode_and_links initial_identity final_identity
  initial_identity="$({
    run_clean /usr/bin/stat --dereference \
      --format='%d:%i:%u:%a:%h:%s:%F' -- "${executable}"
  } 2>/dev/null)" || fail 'cannot identify retained rustc trampoline object'
  [[ "${initial_identity}" == *":${EUID}:555:1:"*':regular file' ]] ||
    fail 'rustc trampoline must be one caller-owned mode 0555 regular object'
  header="$(run_clean /usr/bin/readelf --file-header --wide -- "${executable}")"
  program_headers="$(
    run_clean /usr/bin/readelf --program-headers --wide -- "${executable}"
  )"
  dynamic_tags="$(run_clean /usr/bin/readelf --dynamic --wide -- "${executable}")"

  require_header_value "${header}" Class ELF64
  require_header_value "${header}" Data "2's complement, little endian"
  require_header_value "${header}" Type 'DYN (Position-Independent Executable file)'
  require_header_value "${header}" Machine 'Advanced Micro Devices X86-64'
  require_header_value "${header}" Flags 0x0

  if printf '%s\n' "${program_headers}" |
    run_clean /usr/bin/grep -E '^[[:space:]]*INTERP[[:space:]]' >/dev/null; then
    fail 'rustc trampoline unexpectedly has a PT_INTERP segment'
  fi
  # Require the static-PIE layout to have four non-overlapping load classes,
  # one read-only RELRO segment, and a non-executable stack.
  # shellcheck disable=SC2016
  if ! printf '%s\n' "${program_headers}" |
    run_clean /usr/bin/awk '
      function hexadecimal(value, result, index_value, digit) {
        value = tolower(value)
        sub(/^0x/, "", value)
        if (value == "") return -1
        result = 0
        for (index_value = 1; index_value <= length(value); ++index_value) {
          digit = index("0123456789abcdef", substr(value, index_value, 1))
          if (digit == 0) return -1
          result = (result * 16) + digit - 1
        }
        return result
      }
      function power_of_two(value) {
        if (value < 1) return 0
        while (value > 1) {
          if (value % 2 != 0) return 0
          value /= 2
        }
        return 1
      }
      function overlaps(left_start, left_end, right_start, right_end) {
        return left_start < right_end && right_start < left_end
      }
      function flags(first, last, value, position) {
        value = ""
        for (position = first; position <= last; ++position) value = value $position
        return value
      }
      BEGIN { loads = 0 }
      $1 == "LOAD" {
        offset = hexadecimal($2)
        virtual_address = hexadecimal($3)
        file_size = hexadecimal($5)
        memory_size = hexadecimal($6)
        alignment = hexadecimal($NF)
        value = flags(7, NF - 1)
        if (offset < 0 || virtual_address < 0 || file_size < 0 ||
            memory_size < 0 || alignment < 0 || file_size > memory_size ||
            !power_of_two(alignment) ||
            offset % alignment != virtual_address % alignment) {
          print "rustc trampoline PT_LOAD size or alignment is invalid" > "/dev/stderr"
          bad = 1
        }
        if (value != "R" && value != "RE" && value != "RW") {
          print "rustc trampoline PT_LOAD permissions are outside policy" > "/dev/stderr"
          bad = 1
        }
        if (index(value, "W") && index(value, "E")) {
          print "rustc trampoline PT_LOAD is writable and executable" > "/dev/stderr"
          bad = 1
        }
        file_start[loads] = offset
        file_end[loads] = offset + file_size
        virtual_start[loads] = virtual_address
        virtual_end[loads] = virtual_address + memory_size
        for (previous = 0; previous < loads; ++previous) {
          if (file_size > 0 && file_end[previous] > file_start[previous] &&
              overlaps(file_start[loads], file_end[loads],
                       file_start[previous], file_end[previous])) {
            print "rustc trampoline PT_LOAD file ranges overlap" > "/dev/stderr"
            bad = 1
          }
          if (memory_size > 0 && virtual_end[previous] > virtual_start[previous] &&
              overlaps(virtual_start[loads], virtual_end[loads],
                       virtual_start[previous], virtual_end[previous])) {
            print "rustc trampoline PT_LOAD virtual ranges overlap" > "/dev/stderr"
            bad = 1
          }
        }
        if (value == "RE") ++executable
        if (value == "RW") ++writable
        ++loads
      }
      $1 == "GNU_RELRO" { ++relro; if (flags(7, NF - 1) != "R") bad = 1 }
      $1 == "GNU_STACK" { ++stack; if (flags(7, NF - 1) != "RW") bad = 1 }
      $1 ~ /^(INTERP|SHLIB)$/ { bad = 1 }
      END {
        exit !(loads == 4 && executable == 1 && writable == 1 &&
               relro == 1 && stack == 1 && !bad)
      }
    '; then
    fail 'rustc trampoline program headers violate range, W^X, RELRO, or stack policy'
  fi
  if printf '%s\n' "${dynamic_tags}" |
    run_clean /usr/bin/grep -E '\((NEEDED|RPATH|RUNPATH)\)' >/dev/null; then
    fail 'rustc trampoline unexpectedly has a dependency or runtime search path'
  fi
  if ! printf '%s\n' "${dynamic_tags}" |
    run_clean /usr/bin/grep -E '\(FLAGS\)[[:space:]]+BIND_NOW$' >/dev/null ||
    ! printf '%s\n' "${dynamic_tags}" |
      run_clean /usr/bin/grep -E '\(FLAGS_1\)[[:space:]]+Flags: NOW PIE$' >/dev/null; then
    fail 'rustc trampoline does not require immediate binding for static PIE'
  fi
  if ! run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_FOUNDATION_NON_AUTHORITATIVE' >/dev/null; then
    fail 'rustc trampoline lacks the non-authoritative foundation marker'
  fi
  if ! run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_REPLAY_GATE_POST_EXEC_REQUIRED' >/dev/null; then
    fail 'rustc trampoline lacks the post-exec replay-gate marker'
  fi
  if ! run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_DUMPABLE_NOT_PRESERVED_ACROSS_EXEC' >/dev/null; then
    fail 'rustc trampoline lacks the dumpability reset marker'
  fi
  if ! run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_PRODUCTION_BLOCKED_UNTIL_KERNEL_UNTRACEABLE_EXEC_BOUNDARY_OR_STATIC_BINDING_WRAPPER' >/dev/null; then
    fail 'rustc trampoline lacks the untraceable-exec production blocker'
  fi
  if run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_TEST_ONLY_BUILD' >/dev/null; then
    fail 'production rustc trampoline contains the test-only ELF marker'
  fi
  mode_and_links="$(
    run_clean /usr/bin/stat --dereference --format='%a:%h' -- "${executable}"
  )"
  [[ "${mode_and_links}" == 555:1 ]] ||
    fail "rustc trampoline mode/link count is ${mode_and_links}, expected 555:1"
  final_identity="$({
    run_clean /usr/bin/stat --dereference \
      --format='%d:%i:%u:%a:%h:%s:%F' -- "${executable}"
  } 2>/dev/null)" || fail 'cannot reidentify retained rustc trampoline object'
  [[ "${final_identity}" == "${initial_identity}" ]] ||
    fail 'retained rustc trampoline object identity changed during verification'
}

if [[ "${MODE}" == verify ]]; then
  require_output_directory_identity
  [[ ! -L "${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}" ]] ||
    fail 'rustc trampoline verification rejects symlinks'
  candidate_fd=
  exec {candidate_fd}<"${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}" ||
    fail 'cannot retain rustc trampoline candidate'
  readonly CANDIDATE_FD="${candidate_fd}"
  readonly CANDIDATE_REF="/proc/self/fd/${CANDIDATE_FD}"
  verify_elf "${CANDIDATE_REF}"
  require_output_directory_identity
  installed_candidate_identity="$({
    run_clean /usr/bin/stat --dereference --format='%d:%i' \
      -- "${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}"
  } 2>/dev/null)" || fail 'verified rustc trampoline path became unavailable'
  readonly INSTALLED_CANDIDATE_IDENTITY="${installed_candidate_identity}"
  retained_candidate_identity="$({
    run_clean /usr/bin/stat --dereference --format='%d:%i' \
      -- "${CANDIDATE_REF}"
  } 2>/dev/null)" || fail 'verified rustc trampoline object became unavailable'
  readonly RETAINED_CANDIDATE_IDENTITY="${retained_candidate_identity}"
  [[ "${INSTALLED_CANDIDATE_IDENTITY}" == "${RETAINED_CANDIDATE_IDENTITY}" ]] ||
    fail 'verified rustc trampoline path no longer names the retained object'
  exit 0
fi

[[ -f "${SOURCE}" && ! -L "${SOURCE}" ]] || fail 'trampoline source is unavailable'
require_output_directory_identity
staging_directory="$({
  run_clean /usr/bin/mktemp --directory \
    "${OUTPUT_DIRECTORY_REF}/.fe2o3-rustc-trampoline.XXXXXXXXXX"
} 2>/dev/null)" || fail 'cannot create private rustc trampoline staging directory'
readonly STAGING_DIRECTORY="${staging_directory}"
readonly TEMPORARY="${STAGING_DIRECTORY}/trampoline"
cleanup() {
  run_clean /usr/bin/rm -rf -- "${STAGING_DIRECTORY}" 2>/dev/null || true
}
trap cleanup EXIT
staging_policy="$({
  run_clean /usr/bin/stat --format='%u:%a:%F' -- "${STAGING_DIRECTORY}"
} 2>/dev/null)" || fail 'cannot inspect private rustc trampoline staging directory'
readonly STAGING_POLICY="${staging_policy}"
[[ "${STAGING_POLICY}" == "${EUID}:700:directory" ]] ||
  fail 'rustc trampoline staging directory is not caller-owned mode 0700'

run_clean /usr/bin/cc \
  -std=c11 -O2 -fPIE -static-pie -march=x86-64 -mtune=generic \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  -fno-ident -ffile-prefix-map="${SCRIPT_DIR}"=. \
  -fdebug-prefix-map="${SCRIPT_DIR}"=. \
  -Wl,-z,relro,-z,now,-z,noexecstack,--fatal-warnings,--build-id=none \
  "${SOURCE}" -o "${TEMPORARY}"
run_clean /usr/bin/chmod 0555 "${TEMPORARY}"
[[ ! -L "${TEMPORARY}" ]] || fail 'compiler produced a symlink instead of an ELF object'
artifact_fd=
exec {artifact_fd}<"${TEMPORARY}" || fail 'cannot retain compiled rustc trampoline'
readonly ARTIFACT_FD="${artifact_fd}"
readonly ARTIFACT_REF="/proc/self/fd/${ARTIFACT_FD}"
artifact_identity="$({
  run_clean /usr/bin/stat --dereference --format='%d:%i' -- "${ARTIFACT_REF}"
} 2>/dev/null)" || fail 'cannot identify compiled rustc trampoline'
readonly ARTIFACT_IDENTITY="${artifact_identity}"
verify_elf "${ARTIFACT_REF}"
require_output_directory_identity
run_clean /usr/bin/mv -T -- "${TEMPORARY}" \
  "${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}"
require_output_directory_identity
[[ ! -L "${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}" ]] ||
  fail 'installed rustc trampoline unexpectedly became a symlink'
installed_identity="$({
  run_clean /usr/bin/stat --dereference --format='%d:%i' \
    -- "${OUTPUT_DIRECTORY_REF}/${CANDIDATE_NAME}"
} 2>/dev/null)" || fail 'installed rustc trampoline became unavailable'
readonly INSTALLED_IDENTITY="${installed_identity}"
[[ "${INSTALLED_IDENTITY}" == "${ARTIFACT_IDENTITY}" ]] ||
  fail 'installed rustc trampoline is not the retained verified object'
run_clean /usr/bin/sha256sum -- "${ARTIFACT_REF}"
printf '%s\n' \
  'non-authoritative foundation: Rust broker, seccomp, and policy integration remain required' >&2
printf '%s\n' \
  'production blocked: kernel-untraceable exec boundary or static binding-wrapper bootstrap required' >&2
