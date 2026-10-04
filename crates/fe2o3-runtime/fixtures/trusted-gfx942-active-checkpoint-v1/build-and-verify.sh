#!/usr/bin/env bash
set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly here
readonly clang="${ROCM_PATH:-/opt/rocm}/llvm/bin/clang"
readonly source="${here}/active-checkpoint.ll"
readonly checked="${here}/active-checkpoint.hsaco"
readonly expected_clang='AMD clang version 22.0.0git (https://github.com/RadeonOpenCompute/llvm-project roc-7.2.4 26084 f58b06dce1f9c15707c5f808fd002e18c2accf7e)'
readonly expected_source_sha256='b50fedd4597ec586d0d80e2b51f11611e622d58026858c01407f27e38e1f2b74'
readonly expected_policy_sha256='efe4983756731cd000a27e2d9d1d8bc678c72a06ec1b4e61e4feb0e0b2282bb6'
readonly expected_hsaco_sha256='3f65c33a886dd3f43604104764210344dfdeb8d4f8dac0b0d71098b9ccf91950'

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

tmp="$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx942-active-checkpoint-v1.XXXXXX")"
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
printf 'verified trusted-gfx942-active-checkpoint-v1 %s\n' "${expected_hsaco_sha256}"
