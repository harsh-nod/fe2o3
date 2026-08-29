#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_ANCHOR_TARGET_DIR:-${repo_root}/target/static-anchor}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/fe2o3-external-anchor-service"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-external-anchor-service \
  --bin fe2o3-external-anchor-service \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie \
  -C link-arg=-Wl,-e,fe2o3_secure_start_v1

readonly report="${target_dir}/fe2o3-external-anchor-service.readelf.txt"
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
  printf 'external-anchor service does not enter through its secure pre-runtime shim\n' >&2
  exit 1
fi
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'external-anchor service contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'external-anchor service contains undefined symbols\n' >&2
  exit 1
fi

FE2O3_STATIC_EXTERNAL_ANCHOR="${executable}" \
  CARGO_TARGET_DIR="${target_dir}/profile-test" \
  cargo test --locked -p fe2o3-external-anchor-service \
    --test static_image \
    release_image_is_loader_independent_static_elf \
    -- --exact --ignored

set +e
smoke_output="$({ /usr/bin/env -i "${executable}" 3<&- 4<&- 221<&- 222<&-; } 2>&1)"
smoke_status=$?
set -e
if [[ ${smoke_status} -ne 1 || -n "${smoke_output}" ]]; then
  printf 'external-anchor service did not fail closed silently without its descriptor contract\n' >&2
  exit 1
fi
printf '%s\n' "${executable}"
