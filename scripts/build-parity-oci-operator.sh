#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly SOURCE="${SCRIPT_DIR}/parity-oci-operator-launcher.c"

if (($# != 1)) || [[ "$1" != /* ]]; then
  printf 'usage: %s /absolute/output/path\n' "$0" >&2
  exit 2
fi

readonly OUTPUT="$1"
readonly TEMPORARY="${OUTPUT}.tmp.$$"
trap 'rm -f -- "${TEMPORARY}"' EXIT

/usr/bin/cc \
  -std=c11 -O2 -fPIE -static-pie \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  "${SOURCE}" -o "${TEMPORARY}"
/usr/bin/strip --strip-all "${TEMPORARY}"
chmod 0555 "${TEMPORARY}"
elf_type="$(
  /usr/bin/readelf --file-header --wide -- "${TEMPORARY}" |
    awk '$1 == "Type:" { print $2 }'
)"
if [[ "${elf_type}" != DYN ]]; then
  printf 'operator launcher is not an ELF ET_DYN static PIE\n' >&2
  exit 1
fi
if /usr/bin/readelf --program-headers --wide -- "${TEMPORARY}" |
  grep -Eq '^[[:space:]]*INTERP[[:space:]]'; then
  printf 'operator launcher unexpectedly has a PT_INTERP segment\n' >&2
  exit 1
fi
if /usr/bin/readelf --dynamic --wide -- "${TEMPORARY}" |
  grep -F '(NEEDED)' >/dev/null; then
  printf 'operator launcher unexpectedly has a DT_NEEDED dependency\n' >&2
  exit 1
fi
mv -f -- "${TEMPORARY}" "${OUTPUT}"
/usr/bin/sha256sum -- "${OUTPUT}"
