#!/usr/bin/env bash
# Negative argument/preflight tests only. No Cargo, compiler or issuer execution.
set -euo pipefail
readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly scratch="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-native-ready-test.XXXXXXXX")"
trap 'chmod -R u+w "${scratch}"; rm -rf -- "${scratch}"' EXIT
readonly builder="${repo_root}/scripts/build-native-ready-fixture.sh"
# Even a regressed output check must not reach an installed compiler.
export RUSTUP_HOME="${scratch}/no-toolchains"

refuses() {
  local expected="$1"; shift
  if "$@" >"${scratch}/out" 2>"${scratch}/err"; then
    printf 'unexpected successful fixture build preflight\n' >&2; exit 1
  fi
  grep -Fq -- "${expected}" "${scratch}/err"
}

refuses 'usage:' bash "${builder}"
refuses 'usage:' bash "${builder}" "${scratch}/missing" v3 extra
for family in '' v1 V3 --v3 'v2 v3'; do
  refuses 'family must be v2 or v3' bash "${builder}" "${scratch}/missing" "${family}"
done

mkdir -m 0700 -- "${scratch}/nonempty" "${scratch}/bad-mode"
mkdir -- "${scratch}/nonempty/.hidden"
chmod 0755 "${scratch}/bad-mode"
ln -s -- "${scratch}/nonempty" "${scratch}/alias"

# Every accepted selector still reaches the unchanged output admission checks.
# No invocation below supplies a valid output directory, so none may build.
for family in default v2 v3; do
  selector=()
  if [[ "${family}" != default ]]; then
    selector=("${family}")
  fi
  refuses 'output must be an existing directory, not a symlink' \
    bash "${builder}" "${scratch}/missing" "${selector[@]}"
  refuses 'output must be an existing directory, not a symlink' \
    bash "${builder}" "${scratch}/alias" "${selector[@]}"
  refuses 'output must be owned by the caller and have mode 0700' \
    bash "${builder}" "${scratch}/bad-mode" "${selector[@]}"
  refuses 'output must be empty' \
    bash "${builder}" "${scratch}/nonempty" "${selector[@]}"
done
printf 'native readiness fixture argument/preflight tests passed\n'
