#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
case "$#:${1-}" in
  0:) binary=fe2o3-compiler-execution-issuer
      target_dir="${FE2O3_STATIC_ISSUER_TARGET_DIR:-${repo_root}/target/static-issuer}"
      image_env=FE2O3_STATIC_COMPILER_EXECUTION_ISSUER
      image_test=release_image_is_loader_independent_static_elf ;;
  1:--native) binary=fe2o3-compiler-execution-issuer-native
      target_dir="${FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR:-${repo_root}/target/static-native-issuer}"
      image_env=FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE
      image_test=native_release_image_is_loader_independent_static_elf ;;
  *) printf 'usage: %s [--native]\n' "$0" >&2; exit 2 ;;
esac
readonly binary target_dir image_env image_test
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/${binary}"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-issuer \
  --bin "${binary}" \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie \
  -C link-arg=-Wl,-e,fe2o3_secure_start_v1

readonly report="${target_dir}/${binary}.readelf.txt"
bash "${repo_root}/scripts/check-static-compiler-execution-issuer-image.sh" "${executable}" "${report}"

env "${image_env}=${executable}" CARGO_TARGET_DIR="${target_dir}/profile-test" \
  cargo test --locked -p fe2o3-compiler-execution-issuer \
    --test static_image \
    "${image_test}" \
    -- --exact --ignored

set +e
smoke_output="$(/usr/bin/env -i "${executable}" 3<&- 4<&- 5<&- 6<&- 7<&- 8<&- 9<&- 10<&- 11<&- 2>&1)"
smoke_status=$?
set -e
if [[ ${smoke_status} -ne 1 || -n "${smoke_output}" ]]; then
  printf 'compiler-execution issuer did not fail closed silently without its descriptor contract\n' >&2
  exit 1
fi
printf '%s\n' "${executable}"
