#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly repo_root
readonly builder="${repo_root}/scripts/build-static-compiler-execution-deployment.sh"

fail() {
  printf 'compiler-execution deployment-bundle contract failed: %s\n' "$*" >&2
  exit 1
}

bash -n "${builder}"
set +e
usage="$(${builder} 2>&1)"
status=$?
set -e
[[ ${status} -eq 2 && "${usage}" == usage:* ]] || fail 'builder argument gate changed'

for helper in \
  build-static-compiler-execution-coordinator.sh \
  build-static-compiler-execution-supervisor.sh \
  build-static-compiler-execution-issuer.sh \
  build-static-external-anchor-provisioning-helper.sh \
  build-static-external-anchor-service.sh \
  build-static-compiler-execution-provisioner.sh; do
  grep -Fq -- "scripts/${helper}" "${builder}" || fail "missing ${helper}"
done

for image in \
  fe2o3-compiler-execution-coordinator \
  fe2o3-compiler-execution-supervisor \
  fe2o3-static-preexec-launcher \
  fe2o3-compiler-execution-issuer \
  fe2o3-external-anchor-provisioning-helper \
  fe2o3-external-anchor-service \
  fe2o3-compiler-execution-provision; do
  grep -Fq -- "${image}\"" "${builder}" || fail "missing image ${image}"
done

grep -Fq -- 'ctest --test-dir' "${builder}" || fail 'launcher CTest qualification is missing'
grep -Fq -- 'sha256sum --check --strict SHA256SUMS' "${builder}" ||
  fail 'strict bundle hash verification is missing'

printf 'compiler-execution deployment-bundle inputs are complete\n'
