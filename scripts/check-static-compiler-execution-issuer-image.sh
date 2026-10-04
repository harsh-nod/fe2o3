#!/usr/bin/env bash
# Shared loader/entry checks, not image-family authentication or installation.
set -euo pipefail
export LC_ALL=C
[[ $# == 2 && -f "$1" && ! -L "$1" ]] || { printf 'usage: %s IMAGE REPORT\n' "$0" >&2; exit 2; }
readonly image="$1" report="$2"
size="$(stat -c %s -- "${image}")"
[[ ${size} -gt 0 && ${size} -le 268435456 ]] || exit 1
/usr/bin/readelf -hW -lW -dW -sW -- "${image}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Data:.*little endian' "${report}"
/usr/bin/grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
entry="$(/usr/bin/awk '/Entry point address:/ { print $4 }' "${report}")"
secure="$(/usr/bin/nm -n --defined-only -- "${image}" | /usr/bin/awk '$3 == "fe2o3_secure_start_v1" { print "0x" $1 }')"
[[ "${entry}" =~ ^0x[0-9a-fA-F]+$ && "${secure}" =~ ^0x[0-9a-fA-F]+$ ]] \
  || { printf 'issuer has no unique secure entry\n' >&2; exit 1; }
[[ $((entry)) -eq $((secure)) ]] \
  || { printf 'issuer bypasses its secure pre-runtime entry\n' >&2; exit 1; }
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)|GNU_STACK.*RWE' "${report}"; then
  printf 'issuer has a loader dependency or executable stack\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
[[ -z "$(/usr/bin/nm -u -- "${image}")" ]] \
  || { printf 'issuer has undefined symbols\n' >&2; exit 1; }
