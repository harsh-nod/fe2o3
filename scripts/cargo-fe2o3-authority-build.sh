#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly SOURCE="${SCRIPT_DIR}/cargo-fe2o3-authority-launcher.c"

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

if (($# != 1)) || [[ "$1" != /* ]]; then
  printf 'usage: %s /absolute/output/path\n' "$0" >&2
  exit 2
fi

readonly OUTPUT="$1"
destination_directory="${OUTPUT%/*}"
[[ -n "${destination_directory}" ]] || destination_directory=/
readonly DESTINATION_DIRECTORY="${destination_directory}"
readonly OUTPUT_NAME="${OUTPUT##*/}"

if [[ -z "${OUTPUT_NAME}" || "${OUTPUT_NAME}" == . || \
  "${OUTPUT_NAME}" == .. ]]; then
  fail 'output path must name a file'
fi
if [[ ! -d "${DESTINATION_DIRECTORY}" || -L "${DESTINATION_DIRECTORY}" ]]; then
  fail 'destination directory must be an existing non-symlink directory'
fi
if ! canonical_destination="$(/usr/bin/readlink \
  --canonicalize-existing -- "${DESTINATION_DIRECTORY}" 2>/dev/null)"; then
  fail 'cannot resolve destination directory'
fi
readonly CANONICAL_DESTINATION="${canonical_destination}"
if [[ "${CANONICAL_DESTINATION}" != "${DESTINATION_DIRECTORY}" ]]; then
  fail 'destination directory path must be canonical and contain no symlinks'
fi

effective_uid="$(/usr/bin/id -u)"
destination_uid="$(/usr/bin/stat --format='%u' -- "${DESTINATION_DIRECTORY}")"
destination_mode="$(/usr/bin/stat --format='%a' -- "${DESTINATION_DIRECTORY}")"
destination_identity="$(
  /usr/bin/stat --format='%d:%i' -- "${DESTINATION_DIRECTORY}"
)"
readonly EFFECTIVE_UID="${effective_uid}"
readonly DESTINATION_UID="${destination_uid}"
readonly DESTINATION_MODE="${destination_mode}"
readonly DESTINATION_IDENTITY="${destination_identity}"
if [[ "${DESTINATION_UID}" != "${EFFECTIVE_UID}" ]]; then
  fail 'destination directory must be owned by the effective user'
fi
if (( (8#${DESTINATION_MODE} & 0022) != 0 )); then
  fail 'destination directory must not be group/world-writable'
fi

validate_output() {
  if [[ -L "${OUTPUT}" ]]; then
    fail 'output path must not be a symlink'
  fi
  if [[ -e "${OUTPUT}" && ! -f "${OUTPUT}" ]]; then
    fail 'existing output path must be a regular file'
  fi
}

validate_output
temporary="$(
  /usr/bin/mktemp \
    --tmpdir="${DESTINATION_DIRECTORY}" ".${OUTPUT_NAME}.tmp.XXXXXXXXXX"
)" || fail 'cannot create exclusive temporary output'
readonly TEMPORARY="${temporary}"
cleanup() {
  /usr/bin/rm -f -- "${TEMPORARY}"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

/usr/bin/cc \
  -std=c11 -O2 -fPIE -static-pie \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  "${SOURCE}" -o "${TEMPORARY}"
/usr/bin/strip --strip-all "${TEMPORARY}"
/usr/bin/chmod 0555 "${TEMPORARY}"

elf_type="$(
  /usr/bin/readelf --file-header --wide -- "${TEMPORARY}" |
    /usr/bin/awk '$1 == "Type:" { print $2 }'
)"
if [[ "${elf_type}" != DYN ]]; then
  fail 'authority launcher is not an ELF ET_DYN static PIE'
fi
if /usr/bin/readelf --program-headers --wide -- "${TEMPORARY}" |
  /usr/bin/grep -E '^[[:space:]]*INTERP[[:space:]]' >/dev/null; then
  fail 'authority launcher unexpectedly has a PT_INTERP segment'
fi
if /usr/bin/readelf --dynamic --wide -- "${TEMPORARY}" |
  /usr/bin/grep -E '\((NEEDED|RPATH|RUNPATH)\)' >/dev/null; then
  fail 'authority launcher unexpectedly has a dynamic dependency or search path'
fi
if /usr/bin/readelf --program-headers --wide -- "${TEMPORARY}" |
  /usr/bin/awk '$1 == "GNU_STACK" && $0 ~ /E/ { found = 1 } END { exit !found }'; then
  fail 'authority launcher unexpectedly requests an executable stack'
fi
if /usr/bin/strings --all -- "${TEMPORARY}" |
  /usr/bin/grep -Fx 'FE2O3_AUTHORITY_TEST_ONLY_BUILD' >/dev/null; then
  fail 'production authority launcher contains the test-only build marker'
fi
for required_path in \
  /usr/libexec/fe2o3/cargo-fe2o3-authority-launcher \
  /usr/libexec/fe2o3/cargo-fe2o3 \
  /etc/fe2o3/build-authority/policy-v1 \
  /etc/ld.so.preload; do
  if ! /usr/bin/strings --all -- "${TEMPORARY}" |
    /usr/bin/grep -Fx -- "${required_path}" >/dev/null; then
    fail "authority launcher does not retain fixed production path: ${required_path}"
  fi
done
if [[ "$(/usr/bin/stat --format='%a:%h' -- "${TEMPORARY}")" != 555:1 ]]; then
  fail 'authority launcher output mode or link count is not 0555:1'
fi
if [[ "$(/usr/bin/stat --format='%d:%i' -- "${DESTINATION_DIRECTORY}")" != \
  "${DESTINATION_IDENTITY}" ]]; then
  fail 'destination directory changed during build'
fi
validate_output
/usr/bin/mv -fT -- "${TEMPORARY}" "${OUTPUT}"
/usr/bin/sha256sum -- "${OUTPUT}"
