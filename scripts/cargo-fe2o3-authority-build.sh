#!/bin/bash

set -Eeuo pipefail
umask 077
IFS=$' \t\n'
unset CDPATH GLOBIGNORE

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

# BASH_ENV and loader variables take effect before this script can inspect
# them. Rejecting them records that trusted caller startup is still required;
# this script does not claim a hermetic build-service boundary.
readonly CALLER_BOUNDARY_VARIABLES=(
  BASH_ENV
  ENV
  GLIBC_TUNABLES
  LD_ASSUME_KERNEL
  LD_AUDIT
  LD_BIND_NOT
  LD_BIND_NOW
  LD_DEBUG
  LD_DEBUG_OUTPUT
  LD_DYNAMIC_WEAK
  LD_HWCAP_MASK
  LD_LIBRARY_PATH
  LD_ORIGIN_PATH
  LD_PRELOAD
  LD_PROFILE
  LD_SHOW_AUXV
  LD_USE_LOAD_BIAS
)
for variable_name in "${CALLER_BOUNDARY_VARIABLES[@]}"; do
  if [[ -v "${variable_name}" ]]; then
    printf 'build caller boundary variable must be absent: %s\n' \
      "${variable_name}" >&2
    exit 2
  fi
done

script_parent="${BASH_SOURCE[0]%/*}"
[[ "${script_parent}" != "${BASH_SOURCE[0]}" ]] || script_parent=.
SCRIPT_DIR="$(cd -- "${script_parent}" && pwd -P)"
readonly SCRIPT_DIR
readonly SOURCE="${SCRIPT_DIR}/cargo-fe2o3-authority-launcher.c"

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

candidate_parent="${CANDIDATE%/*}"
[[ -n "${candidate_parent}" ]] || candidate_parent=/
readonly CANDIDATE_PARENT="${candidate_parent}"
readonly CANDIDATE_NAME="${CANDIDATE##*/}"
if [[ -z "${CANDIDATE_NAME}" || "${CANDIDATE_NAME}" == . ||
  "${CANDIDATE_NAME}" == .. ]]; then
  fail 'candidate path must name a file'
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

if ! canonical_parent="$(
  run_clean /usr/bin/readlink --canonicalize-existing -- \
    "${CANDIDATE_PARENT}" 2>/dev/null
)"; then
  fail 'cannot resolve candidate directory'
fi
readonly CANONICAL_PARENT="${canonical_parent}"
if [[ "${CANONICAL_PARENT}" != "${CANDIDATE_PARENT}" ]]; then
  fail 'candidate directory path must be canonical and contain no symlinks'
fi

effective_uid="$(run_clean /usr/bin/id -u)"
parent_uid="$(run_clean /usr/bin/stat --format='%u' -- "${CANDIDATE_PARENT}")"
parent_mode="$(run_clean /usr/bin/stat --format='%a' -- "${CANDIDATE_PARENT}")"
parent_identity="$(
  run_clean /usr/bin/stat --format='%d:%i' -- "${CANDIDATE_PARENT}"
)"
readonly EFFECTIVE_UID="${effective_uid}"
readonly PARENT_UID="${parent_uid}"
readonly PARENT_MODE="${parent_mode}"
readonly PARENT_IDENTITY="${parent_identity}"
if [[ "${PARENT_UID}" != "${EFFECTIVE_UID}" ]]; then
  fail 'candidate directory must be owned by the effective user'
fi
if (( (8#${PARENT_MODE} & 0022) != 0 )); then
  fail 'candidate directory must not be group/world-writable'
fi

validate_candidate_path() {
  if [[ -L "${CANDIDATE}" ]]; then
    fail 'candidate path must not be a symlink'
  fi
  if [[ -e "${CANDIDATE}" && ! -f "${CANDIDATE}" ]]; then
    fail 'candidate path must be a regular file'
  fi
}

require_header_value() {
  local header="$1"
  local label="$2"
  local expected="$3"
  local observed
  # shellcheck disable=SC2016  # This expression is evaluated by awk.
  observed="$(
    printf '%s\n' "${header}" |
      run_clean /usr/bin/awk -F: -v label="${label}" \
        '$1 ~ "^[[:space:]]*" label "$" { sub(/^[[:space:]]+/, "", $2); print $2 }'
  )"
  if [[ "${observed}" != "${expected}" ]]; then
    fail "authority launcher ELF ${label} is '${observed}', expected '${expected}'"
  fi
}

verify_elf() {
  local executable="$1"
  local header
  local program_headers
  local dynamic_tags
  local dynamic_symbols
  local mode_and_links

  if [[ ! -f "${executable}" || -L "${executable}" ]]; then
    fail 'authority launcher verification requires a regular non-symlink file'
  fi
  header="$(run_clean /usr/bin/readelf --file-header --wide -- "${executable}")"
  program_headers="$(
    run_clean /usr/bin/readelf --program-headers --wide -- "${executable}"
  )"
  dynamic_tags="$(
    run_clean /usr/bin/readelf --dynamic --wide -- "${executable}"
  )"
  dynamic_symbols="$(
    run_clean /usr/bin/readelf --dyn-syms --wide -- "${executable}"
  )"

  require_header_value "${header}" Class ELF64
  require_header_value "${header}" Data "2's complement, little endian"
  require_header_value "${header}" Type \
    'DYN (Position-Independent Executable file)'
  require_header_value "${header}" Machine \
    'Advanced Micro Devices X86-64'
  require_header_value "${header}" Flags 0x0

  if printf '%s\n' "${program_headers}" |
    run_clean /usr/bin/grep -E '^[[:space:]]*INTERP[[:space:]]' >/dev/null; then
    fail 'authority launcher unexpectedly has a PT_INTERP segment'
  fi
  # shellcheck disable=SC2016  # This expression is evaluated by awk.
  if ! printf '%s\n' "${program_headers}" |
    run_clean /usr/bin/awk '
      function flags(first, last, result, position) {
        result = ""
        for (position = first; position <= last; ++position) {
          result = result $position
        }
        return result
      }
      $1 == "LOAD" {
        ++loads
        value = flags(7, NF - 1)
        if (value != "R" && value != "RE" && value != "RW") bad = 1
        if (index(value, "W") && index(value, "E")) bad = 1
        if (value == "RE") ++executable
        if (value == "RW") ++writable
      }
      $1 == "DYNAMIC" { ++dynamic; if (flags(7, NF - 1) != "RW") bad = 1 }
      $1 == "GNU_RELRO" { ++relro; if (flags(7, NF - 1) != "R") bad = 1 }
      $1 == "GNU_STACK" { ++stack; if (flags(7, NF - 1) != "RW") bad = 1 }
      $1 ~ /^(INTERP|PHDR|SHLIB)$/ { bad = 1 }
      END {
        exit !(loads == 4 && executable == 1 && writable == 1 &&
               dynamic == 1 && relro == 1 && stack == 1 && !bad)
      }
    '; then
    fail 'authority launcher program headers violate exact W^X, RELRO, or stack policy'
  fi

  # shellcheck disable=SC2016  # This expression is evaluated by awk.
  if ! printf '%s\n' "${dynamic_tags}" |
    run_clean /usr/bin/awk '
      /^ 0x/ {
        tag = $2
        gsub(/[()]/, "", tag)
        allowed = tag ~ /^(INIT|FINI|INIT_ARRAY|INIT_ARRAYSZ|FINI_ARRAY|FINI_ARRAYSZ|GNU_HASH|STRTAB|SYMTAB|STRSZ|SYMENT|DEBUG|PLTGOT|PLTRELSZ|PLTREL|JMPREL|RELA|RELASZ|RELAENT|FLAGS|FLAGS_1|RELACOUNT|NULL)$/
        if (!allowed) bad = 1
        ++count
      }
      END { exit !(count == 23 && !bad) }
    '; then
    fail 'authority launcher has an unexpected dynamic tag set'
  fi
  if ! printf '%s\n' "${dynamic_tags}" |
    run_clean /usr/bin/grep -E '\(FLAGS\)[[:space:]]+BIND_NOW$' >/dev/null ||
    ! printf '%s\n' "${dynamic_tags}" |
      run_clean /usr/bin/grep -E '\(FLAGS_1\)[[:space:]]+Flags: NOW PIE$' >/dev/null; then
    fail 'authority launcher does not require RELRO-compatible immediate binding'
  fi
  if printf '%s\n' "${dynamic_tags}" |
    run_clean /usr/bin/grep -E '\((NEEDED|RPATH|RUNPATH)\)' >/dev/null; then
    fail 'authority launcher unexpectedly has a dependency or runtime search path'
  fi

  # shellcheck disable=SC2016  # This expression is evaluated by awk.
  if ! printf '%s\n' "${dynamic_symbols}" |
    run_clean /usr/bin/awk '
      /^Symbol table .*dynsym.* contains 1 entry:$/ { header = 1 }
      $7 == "UND" {
        ++undefined
        if ($1 != "0:" || $4 != "NOTYPE" || $5 != "LOCAL" ||
            $6 != "DEFAULT" || NF != 7) bad = 1
      }
      END { exit !(header && undefined == 1 && !bad) }
    '; then
    fail 'authority launcher has unexpected undefined dynamic symbols'
  fi

  if run_clean /usr/bin/strings --all -- "${executable}" |
    run_clean /usr/bin/grep -Fx 'FE2O3_AUTHORITY_TEST_ONLY_BUILD' >/dev/null; then
    fail 'production authority launcher contains the test-only build marker'
  fi
  for required_path in \
    /usr/libexec/fe2o3/cargo-fe2o3-authority-launcher \
    /usr/libexec/fe2o3/cargo-fe2o3 \
    /etc/fe2o3/build-authority/policy-v1 \
    /etc/ld.so.preload; do
    if ! run_clean /usr/bin/strings --all -- "${executable}" |
      run_clean /usr/bin/grep -Fx -- "${required_path}" >/dev/null; then
      fail "authority launcher lacks fixed production path: ${required_path}"
    fi
  done
  mode_and_links="$(
    run_clean /usr/bin/stat --format='%a:%h' -- "${executable}"
  )"
  if [[ "${mode_and_links}" != 555:1 ]]; then
    fail 'authority launcher mode or link count is not 0555:1'
  fi
}

validate_candidate_path
if [[ "${MODE}" == verify ]]; then
  [[ -f "${CANDIDATE}" ]] || fail 'verification candidate does not exist'
  verify_elf "${CANDIDATE}"
  run_clean /usr/bin/sha256sum -- "${CANDIDATE}"
  exit 0
fi

temporary="$(
  run_clean /usr/bin/mktemp \
    --tmpdir="${CANDIDATE_PARENT}" ".${CANDIDATE_NAME}.tmp.XXXXXXXXXX"
)" || fail 'cannot create exclusive temporary output'
readonly TEMPORARY="${temporary}"
cleanup() {
  run_clean /usr/bin/rm -f -- "${TEMPORARY}" || true
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

run_clean /usr/bin/cc \
  -std=c11 -O2 -fPIE -static-pie \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  -Wl,-z,relro,-z,now,-z,noexecstack \
  "${SOURCE}" -o "${TEMPORARY}"
run_clean /usr/bin/strip --strip-all "${TEMPORARY}"
run_clean /usr/bin/chmod 0555 "${TEMPORARY}"
verify_elf "${TEMPORARY}"

if [[ "$(
  run_clean /usr/bin/stat --format='%d:%i' -- "${CANDIDATE_PARENT}"
)" != "${PARENT_IDENTITY}" ]]; then
  fail 'candidate directory changed during build'
fi
validate_candidate_path
run_clean /usr/bin/mv -fT -- "${TEMPORARY}" "${CANDIDATE}"
run_clean /usr/bin/sha256sum -- "${CANDIDATE}"
