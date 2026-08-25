#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

if (($# != 2)); then
  printf 'usage: %s <absolute-cargo-fe2o3> <managed-package>\n' "$0" >&2
  exit 2
fi

readonly DRIVER="$1"
readonly PACKAGE="$2"
[[ "${DRIVER}" == /* && -f "${DRIVER}" && ! -L "${DRIVER}" && -x "${DRIVER}" ]] || {
  printf 'binding-check boundary received an invalid driver: %s\n' "${DRIVER}" >&2
  exit 2
}

readonly TEST_ROOT="$(mktemp -d -- "${TMPDIR:?}/binding-check-boundary.XXXXXX")"
cleanup() {
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

for name in \
  FE2O3_BACKEND \
  FE2O3_HSACO_DIR \
  FE2O3_PROTECTED_RELEASE_ACTION_V1 \
  FE2O3_AUTHORITY_CARGO_SHA256_V1; do
  log="${TEST_ROOT}/${name}.log"
  value="${TEST_ROOT}/hostile"
  if env "${name}=${value}" \
    "${DRIVER}" check --all-targets --locked -p "${PACKAGE}" \
    >"${log}" 2>&1; then
    printf 'binding-only check admitted hostile %s\n' "${name}" >&2
    exit 1
  fi
  rg -F "rejects authority-bearing environment ${name}" "${log}" >/dev/null || {
    printf 'binding-only check did not report the expected %s rejection\n' "${name}" >&2
    cat "${log}" >&2
    exit 1
  }
done

backend_log="${TEST_ROOT}/codegen-backend.log"
if env RUSTFLAGS='-Zcodegen-backend=/nonexistent/hostile-backend.so' \
  "${DRIVER}" check --all-targets --locked -p "${PACKAGE}" \
  >"${backend_log}" 2>&1; then
  printf '%s\n' 'binding-only check admitted a rustc codegen backend selector' >&2
  exit 1
fi
rg -F 'contains a codegen-backend selector' "${backend_log}" >/dev/null || {
  printf '%s\n' 'binding-only check did not reject the hostile backend selector' >&2
  cat "${backend_log}" >&2
  exit 1
}

[[ ! -e "${TEST_ROOT}/hostile" && ! -L "${TEST_ROOT}/hostile" ]] || {
  printf '%s\n' 'binding-only check created a backend, HSACO, or publication path' >&2
  exit 1
}

printf 'binding-only check boundary: backend, artifact, and publication authority rejected\n'
