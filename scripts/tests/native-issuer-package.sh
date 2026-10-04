#!/usr/bin/env bash
# Negative package/selector tests only. No Cargo, compiler, issuer or privileged runtime.
set -euo pipefail
readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly scratch="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-native-package-test.XXXXXXXX")"
trap 'chmod -R u+w "${scratch}"; rm -rf -- "${scratch}"' EXIT
readonly package="${scratch}/package" binary=fe2o3-compiler-execution-issuer-native
readonly commit=1111111111111111111111111111111111111111
readonly verifier="${repo_root}/scripts/verify-native-compiler-execution-issuer-package.sh"
mkdir -m 0700 -- "${package}"

reset_fixture() {
  chmod -R u+w "${package}"
  rm -f -- "${package}"/* "${scratch}/alias"
  printf 'inert non-ELF test image\n' >"${package}/${binary}"
  printf 'schema_version=1\nartifact_family=compiler-execution-issuer-native-v2\ngit_commit=%s\ntarget=x86_64-unknown-linux-musl\n' "${commit}" >"${package}/BUILD-INFO"
  (cd -- "${package}"; sha256sum -- "${binary}" BUILD-INFO) >"${package}/SHA256SUMS"
  chmod 0555 "${package}/${binary}"
  chmod 0444 "${package}/BUILD-INFO" "${package}/SHA256SUMS"
  digest="$(sha256sum -- "${package}/${binary}")"
  digest="${digest%% *}"
}
refuses() {
  local expected="$1"; shift
  if "$@" >"${scratch}/out" 2>"${scratch}/err"; then
    printf 'unexpected successful package validation\n' >&2; exit 1
  fi
  grep -Fq -- "${expected}" "${scratch}/err"
}
reset_fixture
refuses 'usage:' bash "${repo_root}/scripts/build-static-compiler-execution-issuer.sh" --infer-family
refuses 'usage:' bash "${verifier}" "${package}" bad-pin "${commit}"
refuses 'native image differs' bash "${verifier}" "${package}" "$(printf '%064d' 0)" "${commit}"
refuses 'native source/family metadata mismatch' bash "${verifier}" "${package}" "${digest}" 2222222222222222222222222222222222222222
chmod u+w "${package}/BUILD-INFO"
printf 'artifact_family=compiler-execution-issuer-v1\n' >"${package}/BUILD-INFO"
chmod 0444 "${package}/BUILD-INFO"
refuses 'native source/family metadata mismatch' bash "${verifier}" "${package}" "${digest}" "${commit}"
reset_fixture
chmod u+w "${package}/BUILD-INFO"
printf '\n' >>"${package}/BUILD-INFO"
chmod 0444 "${package}/BUILD-INFO"
refuses 'native source/family metadata mismatch' bash "${verifier}" "${package}" "${digest}" "${commit}"
reset_fixture
ln -- "${package}/${binary}" "${scratch}/alias"
refuses 'hardlinked package member' bash "${verifier}" "${package}" "${digest}" "${commit}"
reset_fixture
mv -- "${package}/${binary}" "${scratch}/alias"
ln -s -- "${scratch}/alias" "${package}/${binary}"
refuses 'invalid member' bash "${verifier}" "${package}" "${digest}" "${commit}"
reset_fixture
touch "${package}/unexpected"
refuses 'exactly the native image' bash "${verifier}" "${package}" "${digest}" "${commit}"
reset_fixture
chmod u+w "${package}/SHA256SUMS"
printf 'unsafe attacker-provided digest paths\n' >"${package}/SHA256SUMS"
chmod 0444 "${package}/SHA256SUMS"
refuses 'noncanonical or changed package digests' bash "${verifier}" "${package}" "${digest}" "${commit}"
printf 'native issuer package negative tests passed\n'
