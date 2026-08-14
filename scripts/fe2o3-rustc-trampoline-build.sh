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
  local header program_headers dynamic_tags mode_and_links
  if [[ ! -f "${executable}" || -L "${executable}" ]]; then
    fail 'rustc trampoline verification requires a regular non-symlink file'
  fi
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
      function flags(first, last, value, position) {
        value = ""
        for (position = first; position <= last; ++position) value = value $position
        return value
      }
      $1 == "LOAD" {
        ++loads
        value = flags(7, NF - 1)
        if (value != "R" && value != "RE" && value != "RW") bad = 1
        if (index(value, "W") && index(value, "E")) bad = 1
        if (value == "RE") ++executable
        if (value == "RW") ++writable
      }
      $1 == "GNU_RELRO" { ++relro; if (flags(7, NF - 1) != "R") bad = 1 }
      $1 == "GNU_STACK" { ++stack; if (flags(7, NF - 1) != "RW") bad = 1 }
      $1 ~ /^(INTERP|SHLIB)$/ { bad = 1 }
      END {
        exit !(loads == 4 && executable == 1 && writable == 1 &&
               relro == 1 && stack == 1 && !bad)
      }
    '; then
    fail 'rustc trampoline program headers violate W^X, RELRO, or stack policy'
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
  if run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_TEST_ONLY_BUILD' >/dev/null; then
    fail 'production rustc trampoline contains the test-only ELF marker'
  fi
  mode_and_links="$(run_clean /usr/bin/stat --format='%a:%h' -- "${executable}")"
  [[ "${mode_and_links}" == 555:1 ]] ||
    fail "rustc trampoline mode/link count is ${mode_and_links}, expected 555:1"
}

if [[ "${MODE}" == verify ]]; then
  verify_elf "${CANDIDATE}"
  exit 0
fi

[[ -f "${SOURCE}" && ! -L "${SOURCE}" ]] || fail 'trampoline source is unavailable'
temporary="${CANDIDATE_PARENT}/.${CANDIDATE_NAME}.tmp.$$"
readonly TEMPORARY="${temporary}"
cleanup() {
  rm -f -- "${TEMPORARY}"
}
trap cleanup EXIT

run_clean /usr/bin/cc \
  -std=c11 -O2 -fPIE -static-pie -march=x86-64 -mtune=generic \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  -fno-ident -ffile-prefix-map="${SCRIPT_DIR}"=. \
  -fdebug-prefix-map="${SCRIPT_DIR}"=. \
  -Wl,-z,relro,-z,now,-z,noexecstack,--fatal-warnings,--build-id=none \
  "${SOURCE}" -o "${TEMPORARY}"
run_clean /usr/bin/chmod 0555 "${TEMPORARY}"
verify_elf "${TEMPORARY}"
run_clean /usr/bin/mv -- "${TEMPORARY}" "${CANDIDATE}"
verify_elf "${CANDIDATE}"
run_clean /usr/bin/sha256sum -- "${CANDIDATE}"
printf '%s\n' \
  'non-authoritative foundation: Rust broker, seccomp, and policy integration remain required' >&2
