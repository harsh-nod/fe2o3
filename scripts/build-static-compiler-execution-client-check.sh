#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_CLIENT_CHECK_TARGET_DIR:-${repo_root}/target/static-client-check}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/fe2o3-compiler-execution-client-check"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}/test" cargo test \
  --locked \
  -p fe2o3-compiler-execution-client \
  --all-targets

CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-client \
  --bin fe2o3-compiler-execution-client-check \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie

readonly report="${target_dir}/fe2o3-compiler-execution-client-check.readelf.txt"
/usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'compiler-execution client check contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'compiler-execution client check contains undefined symbols\n' >&2
  exit 1
fi

set +e
usage_output="$(/usr/bin/env -i "${executable}" forbidden 2>&1)"
usage_status=$?
set -e
if [[ ${usage_status} -ne 2 \
  || "${usage_output}" != 'usage: fe2o3-compiler-execution-client-check' ]]; then
  printf 'compiler-execution client check argument gate changed\n' >&2
  exit 1
fi

if [[ $(/usr/bin/id -u) -eq 0 ]]; then
  set +e
  root_output="$(/usr/bin/env -i "${executable}" 2>&1)"
  root_status=$?
  set -e
  if [[ ${root_status} -ne 1 \
    || "${root_output}" != \
      'compiler-execution client check failed: root credentials are forbidden' ]]; then
    printf 'compiler-execution client check root rejection changed\n' >&2
    exit 1
  fi
fi

printf '%s\n' "${executable}"
