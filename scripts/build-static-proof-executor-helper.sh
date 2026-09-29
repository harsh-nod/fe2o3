#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
if [[ $# -ne 0 ]]; then
  printf 'usage: %s\n' "$0" >&2
  exit 2
fi
readonly binary="fe2o3-proof-executor-helper"
readonly target_dir="${FE2O3_STATIC_PROOF_EXECUTOR_HELPER_TARGET_DIR:-${repo_root}/target/static-proof-executor-helper}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/${binary}"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-verifier \
  --bin "${binary}" \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie \
  -C link-arg=-Wl,-e,fe2o3_secure_start_v1

readonly report="${target_dir}/${binary}.readelf.txt"
/usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
entry_address="$(/usr/bin/awk '/Entry point address:/ { print $4 }' "${report}")"
secure_start_address="$(
  /usr/bin/nm -n --defined-only -- "${executable}" \
    | /usr/bin/awk '$3 == "fe2o3_secure_start_v1" { print "0x" $1 }'
)"
if [[ ! "${entry_address}" =~ ^0x[[:xdigit:]]{1,16}$ \
  || ! "${secure_start_address}" =~ ^0x[[:xdigit:]]{1,16}$ ]] \
  || (( entry_address != secure_start_address )); then
  printf 'proof-executor helper does not enter through its secure pre-runtime shim\n' >&2
  exit 1
fi
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'proof-executor helper contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'proof-executor helper contains undefined symbols\n' >&2
  exit 1
fi
printf '%s\n' "${executable}"
