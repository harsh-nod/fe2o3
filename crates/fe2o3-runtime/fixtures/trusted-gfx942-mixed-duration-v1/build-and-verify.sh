#!/usr/bin/env bash
set -euo pipefail
here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
llvm="${ROCM_PATH:-/opt/rocm}/llvm/bin"
readonly here llvm
printf '%s  %s\n' \
  5c1d565a505c004aedf425b6c3b7c5199e09fd5207ef3f7d31d0ba00774a8f28 "$llvm/clang" \
  d088c46031665db7985be41360fcf3f8e80e2536fa608b7a3d64223fd13cfebb "$llvm/ld.lld" \
  c894b7482effe9c285b5d0a355ec6484d7675dcfaffcb1ca30a6d08b393441cb "$llvm/llvm-objdump" \
  95b388cceb9a7174f4d5804b6abab7e3ab22345b19d027a990980c0b29ab5d36 "$here/short.ll" \
  8f5961bcdf462636863d9169f7903e128305432ad511335a73379ccae670c829 "$here/long.ll" \
  ff196cd47ea25cd558f9e6fa8da4e895380a23f5ff55056a78ca0e456c4e20d1 "$here/policy-v1.txt" \
  b3cf15845879f968b3e3b09f4a5f4bdd72d1f77466a0c048d9727da066719ee5 "$here/short.hsaco" \
  642f08b1ce18f6c3428d3dfac4c9e567b3d23c11cec0849ab47a6de8a06274a9 "$here/long.hsaco" \
  | sha256sum --check --status
tmp="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-gfx942-mixed-duration-v1.XXXXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
for fixture in short long; do
  LC_ALL=C SOURCE_DATE_EPOCH=0 "$llvm/clang" --no-default-config \
    --target=amdgcn-amd-amdhsa -mcpu=gfx942:xnack- -nogpulib -O2 \
    --ld-path="$llvm/ld.lld" "$here/$fixture.ll" -o "$tmp/$fixture.hsaco"
  cmp --silent -- "$tmp/$fixture.hsaco" "$here/$fixture.hsaco"
done
python3 -I -B "$here/oracle.py" check
printf 'PASS: byte-identical fixed-work gfx942 fixture pair\n'
