#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_PROVISIONER_TARGET_DIR:-${repo_root}/target/static-provisioner}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/fe2o3-compiler-execution-provision"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-coordinator \
  --bin fe2o3-compiler-execution-provision \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie

readonly report="${target_dir}/fe2o3-compiler-execution-provision.readelf.txt"
/usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'compiler-execution provisioner contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'compiler-execution provisioner contains undefined symbols\n' >&2
  exit 1
fi

for argument in '' 0 01 +1; do
  set +e
  if [[ -z "${argument}" ]]; then
    smoke_output="$({ /usr/bin/env -i "${executable}"; } 2>&1)"
  else
    smoke_output="$({ /usr/bin/env -i "${executable}" "${argument}"; } 2>&1)"
  fi
  smoke_status=$?
  set -e
  if [[ ${smoke_status} -ne 1 \
    || "${smoke_output}" != 'expected exactly one canonical nonzero decimal policy generation' ]]; then
    printf 'compiler-execution provisioner accepted a noncanonical generation\n' >&2
    exit 1
  fi
done

printf '%s\n' "${executable}"
