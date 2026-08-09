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
  -std=c11 -O2 -fPIE -pie -static \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  "${SOURCE}" -o "${TEMPORARY}"
/usr/bin/strip --strip-all "${TEMPORARY}"
chmod 0555 "${TEMPORARY}"
if ! /usr/bin/file --brief -- "${TEMPORARY}" | grep -F 'statically linked' >/dev/null; then
  printf 'operator launcher is not statically linked\n' >&2
  exit 1
fi
mv -f -- "${TEMPORARY}" "${OUTPUT}"
/usr/bin/sha256sum -- "${OUTPUT}"
