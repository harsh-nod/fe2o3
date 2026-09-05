#!/usr/bin/env bash
set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly here
readonly clang="${ROCM_PATH:-/opt/rocm}/llvm/bin/clang"
readonly source="${here}/inplace_transform.ll"
readonly checked="${here}/inplace_transform.hsaco"
readonly expected_clang='AMD clang version 22.0.0git (https://github.com/RadeonOpenCompute/llvm-project roc-7.2.0 26014 7b800a19466229b8479a78de19143dc33c3ab9b5)'
readonly expected_source_sha256='1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5'
readonly expected_policy_sha256='c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585'
readonly expected_hsaco_sha256='8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9'

[[ -x "${clang}" ]] || {
  printf 'missing pinned ROCm clang: %s\n' "${clang}" >&2
  exit 2
}
[[ "$("${clang}" --version | sed -n '1p')" == "${expected_clang}" ]] || {
  printf 'ROCm clang identity does not match the pinned producer\n' >&2
  exit 2
}
printf '%s  %s\n' "${expected_source_sha256}" "${source}" | sha256sum --check --status
printf '%s  %s\n' "${expected_policy_sha256}" "${here}/policy-v1.txt" | sha256sum --check --status

tmp="$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx942-inplace-transform-v1.XXXXXX")"
trap 'rm -f -- "${tmp}"' EXIT
LC_ALL=C SOURCE_DATE_EPOCH=0 "${clang}" \
  --target=amdgcn-amd-amdhsa \
  -mcpu=gfx942:xnack- \
  -nogpulib \
  -O2 \
  "${source}" \
  -o "${tmp}"
printf '%s  %s\n' "${expected_hsaco_sha256}" "${tmp}" | sha256sum --check --status
cmp --silent -- "${tmp}" "${checked}" || {
  printf 'rebuilt HSACO is not byte-identical to the checked artifact\n' >&2
  exit 1
}
printf 'verified trusted-gfx942-inplace-transform-v1 %s\n' "${expected_hsaco_sha256}"
