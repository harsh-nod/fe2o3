#!/usr/bin/env bash
set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly here
readonly clang="${ROCM_PATH:-/opt/rocm}/llvm/bin/clang"
readonly source="${here}/vecadd.ll"
readonly checked="${here}/vecadd.hsaco"
readonly expected_clang='AMD clang version 22.0.0git (https://github.com/RadeonOpenCompute/llvm-project roc-7.2.4 26084 f58b06dce1f9c15707c5f808fd002e18c2accf7e)'
readonly expected_source_sha256='b3412c050ce2182feb669d267e3e7208400c4d16f0865efb7aeafd118c8f7e51'
readonly expected_policy_sha256='558897b2c24edacb9a0d83a630d6f8480a74095771211fafc0fdb823d476c9a7'
readonly expected_hsaco_sha256='3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab'

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

tmp="$(mktemp "${TMPDIR:-/tmp}/fe2o3-gfx942-vecadd-v1.XXXXXX")"
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
printf 'verified trusted-gfx942-vecadd-v1 %s\n' "${expected_hsaco_sha256}"
