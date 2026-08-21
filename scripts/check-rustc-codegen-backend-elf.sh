#!/usr/bin/env bash

set -Eeuo pipefail

readonly MAX_CODEGEN_BACKEND_BYTES=536870912

if (($# != 2)); then
  printf '%s\n' \
    'usage: check-rustc-codegen-backend-elf.sh <cargo-target-dir> <rustc>' >&2
  exit 2
fi

readonly TARGET_DIR="$1"
readonly RUSTC="$2"
readonly BACKEND="${TARGET_DIR}/debug/librustc_codegen_fe2o3.so"

if [[ -L "${BACKEND}" || ! -f "${BACKEND}" ]]; then
  printf 'rustc-codegen backend is not a regular non-symlink file: %s\n' \
    "${BACKEND}" >&2
  exit 1
fi

backend_bytes="$(stat --format='%s' -- "${BACKEND}")"
if [[ ! "${backend_bytes}" =~ ^[1-9][0-9]*$ ]]; then
  printf 'rustc-codegen backend has an invalid byte length: %s\n' \
    "${backend_bytes}" >&2
  exit 1
fi
if ((backend_bytes > MAX_CODEGEN_BACKEND_BYTES)); then
  printf 'rustc-codegen backend is %d bytes, exceeding the unchanged %d-byte limit\n' \
    "${backend_bytes}" "${MAX_CODEGEN_BACKEND_BYTES}" >&2
  exit 1
fi
readonly backend_bytes
readonly backend_headroom=$((MAX_CODEGEN_BACKEND_BYTES - backend_bytes))

backend_sha256="$(sha256sum -- "${BACKEND}" | cut -d' ' -f1)"
if [[ ! "${backend_sha256}" =~ ^[0-9a-f]{64}$ ]]; then
  printf 'rustc-codegen backend has an invalid SHA-256 measurement: %s\n' \
    "${backend_sha256}" >&2
  exit 1
fi
readonly backend_sha256

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-codegen-elf.XXXXXXXX")"
readonly SCRATCH
cleanup() {
  find "${SCRATCH}" -depth -delete
}
trap cleanup EXIT
readonly ELF_REPORT="${SCRATCH}/elf.txt"

readelf --file-header --wide -- "${BACKEND}" >"${ELF_REPORT}"
grep -Eq 'Class:[[:space:]]+ELF64' "${ELF_REPORT}" || {
  printf '%s\n' 'rustc-codegen backend is not ELF64' >&2
  exit 1
}
grep -Eq 'Data:[[:space:]]+2.s complement, little endian' "${ELF_REPORT}" || {
  printf '%s\n' 'rustc-codegen backend is not little-endian ELF' >&2
  exit 1
}
grep -Eq 'Type:[[:space:]]+DYN \(Shared object file\)' "${ELF_REPORT}" || {
  printf '%s\n' 'rustc-codegen backend is not ET_DYN' >&2
  exit 1
}

cat >"${SCRATCH}/load_probe.rs" <<'EOF'
#![no_std]

pub fn fe2o3_codegen_backend_load_probe() {}
EOF

env \
  -u CARGO_ENCODED_RUSTFLAGS \
  -u RUSTC_WRAPPER \
  -u RUSTC_WORKSPACE_WRAPPER \
  -u RUSTFLAGS \
  timeout --signal=TERM --kill-after=5s 60s \
  "${RUSTC}" \
  --crate-name fe2o3_codegen_backend_load_probe \
  --crate-type lib \
  --emit metadata \
  "-Zcodegen-backend=${BACKEND}" \
  --out-dir "${SCRATCH}" \
  "${SCRATCH}/load_probe.rs"

printf '%s\n' \
  'rustc_codegen_backend_elf_regular=true' \
  'rustc_codegen_backend_elf_type=ET_DYN' \
  'rustc_codegen_backend_rustc_loadable=true'
printf 'rustc_codegen_backend_bytes=%d\n' "${backend_bytes}"
printf 'rustc_codegen_backend_limit_bytes=%d\n' "${MAX_CODEGEN_BACKEND_BYTES}"
printf 'rustc_codegen_backend_headroom_bytes=%d\n' "${backend_headroom}"
printf 'rustc_codegen_backend_sha256=%s\n' "${backend_sha256}"
