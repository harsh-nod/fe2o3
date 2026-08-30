#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_DEPLOYMENT_VERIFIER_TARGET_DIR:-${repo_root}/target/static-deployment-verifier}"
readonly target="x86_64-unknown-linux-musl"
readonly manifest="${target_dir}/${target}/release/fe2o3-compiler-execution-manifest"
readonly verifier="${target_dir}/${target}/release/fe2o3-compiler-execution-deployment-verify"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}/profile-test" \
  cargo test --locked -p fe2o3-compiler-execution-deployment --all-targets

for binary in \
  fe2o3-compiler-execution-manifest \
  fe2o3-compiler-execution-deployment-verify; do
  CARGO_TARGET_DIR="${target_dir}" cargo rustc \
    --locked \
    --release \
    --target "${target}" \
    -p fe2o3-compiler-execution-deployment \
    --bin "${binary}" \
    -- \
    -C target-feature=+crt-static \
    -C relocation-model=static \
    -C link-arg=-static \
    -C link-arg=-no-pie

  executable="${target_dir}/${target}/release/${binary}"
  report="${target_dir}/${binary}.readelf.txt"
  /usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
  /usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
  if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
    printf '%s contains a dynamic-loader dependency\n' "${binary}" >&2
    exit 1
  fi
  /usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
  undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
  if [[ -n "${undefined_symbols}" ]]; then
    printf '%s contains undefined symbols\n' "${binary}" >&2
    exit 1
  fi
done

set +e
manifest_usage="$(/usr/bin/env -i "${manifest}" 2>&1)"
manifest_status=$?
verifier_usage="$(/usr/bin/env -i "${verifier}" 2>&1)"
verifier_status=$?
set -e
if [[ ${manifest_status} -ne 2 \
  || "${manifest_usage}" != 'usage: fe2o3-compiler-execution-manifest BUNDLE_ROOT GIT_COMMIT TARGET' ]]; then
  printf 'static deployment manifest generator argument gate changed\n' >&2
  exit 1
fi
if [[ ${verifier_status} -ne 2 \
  || "${verifier_usage}" != 'usage: fe2o3-compiler-execution-deployment-verify BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT' ]]; then
  printf 'static deployment verifier argument gate changed\n' >&2
  exit 1
fi

printf 'manifest_generator=%s\n' "${manifest}"
printf 'deployment_verifier=%s\n' "${verifier}"
