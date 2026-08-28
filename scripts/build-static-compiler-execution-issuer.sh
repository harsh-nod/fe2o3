#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_ISSUER_TARGET_DIR:-${repo_root}/target/static-issuer}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/fe2o3-compiler-execution-issuer"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-issuer \
  --bin fe2o3-compiler-execution-issuer \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie \
  -C link-arg=-Wl,-e,fe2o3_secure_start_v1

readonly report="${target_dir}/fe2o3-compiler-execution-issuer.readelf.txt"
/usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
entry_address="$(/usr/bin/awk '/Entry point address:/ { print $4 }' "${report}")"
secure_start_address="$(
  /usr/bin/nm -n --defined-only -- "${executable}" \
    | /usr/bin/awk '$3 == "fe2o3_secure_start_v1" { print "0x" $1 }'
)"
if [[ -z "${entry_address}" || -z "${secure_start_address}" \
  || $((entry_address)) -ne $((secure_start_address)) ]]; then
  printf 'compiler-execution issuer does not enter through its secure pre-runtime shim\n' >&2
  exit 1
fi
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'compiler-execution issuer contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'compiler-execution issuer contains undefined symbols\n' >&2
  exit 1
fi

set +e
smoke_output="$(/usr/bin/env -i "${executable}" 3<&- 4<&- 5<&- 6<&- 7<&- 8<&- 9<&- 2>&1)"
smoke_status=$?
set -e
if [[ ${smoke_status} -ne 1 || -n "${smoke_output}" ]]; then
  printf 'compiler-execution issuer did not fail closed silently without its descriptor contract\n' >&2
  exit 1
fi
printf '%s\n' "${executable}"
