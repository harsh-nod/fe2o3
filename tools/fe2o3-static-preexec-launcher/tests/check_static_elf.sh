#!/usr/bin/env bash
set -euo pipefail

readonly launcher="$1"
readonly report="$2"

/usr/bin/readelf -hW -lW -dW -sW -- "$launcher" >"$report"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "$report"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "$report"
if [[ "$(/usr/bin/grep -Ec '^[[:space:]]+PHDR[[:space:]]' "$report")" -ne 1 ]]; then
  printf 'static launcher does not contain exactly one PT_PHDR\n' >&2
  exit 1
fi
if /usr/bin/grep -Eq 'LOAD.*RWE' "$report"; then
  printf 'static launcher contains a writable/executable load segment\n' >&2
  exit 1
fi
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "$report"; then
  printf 'static launcher contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "$report"
if /usr/bin/nm -u -- "$launcher" | /usr/bin/grep -q .; then
  printf 'freestanding launcher contains undefined symbols\n' >&2
  exit 1
fi
/usr/bin/nm -n -- "$launcher" | /usr/bin/grep -Eq '[[:space:]]T[[:space:]]_start$'
if /usr/bin/grep -Eq 'GLIBC_|__libc|^[[:space:]]+[1-9][0-9]*:.*[[:space:]]UND[[:space:]]' "$report"; then
  printf 'freestanding launcher contains a libc or undefined-symbol boundary\n' >&2
  exit 1
fi
