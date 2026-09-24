#!/usr/bin/env bash
# An externally pinned native package; this does not grant supervisor/deployment authority.
set -euo pipefail
export LC_ALL=C
readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
fail() { printf 'native issuer verification: %s\n' "$*" >&2; exit 1; }
[[ $# == 3 && -d "$1" && ! -L "$1" && "$2" =~ ^[0-9a-f]{64}$ && "$3" =~ ^[0-9a-f]{40}$ ]] \
  || fail 'usage: bash scripts/verify-native-compiler-execution-issuer-package.sh DIRECTORY EXPECTED_IMAGE_SHA256 EXPECTED_COMMIT'
readonly package="$(cd -- "$1" && pwd -P)" expected="$2" commit="$3"
readonly binary='fe2o3-compiler-execution-issuer-native'
shopt -s nullglob dotglob
files=("${package}"/*)
[[ ${#files[@]} == 3 ]] || fail 'package must contain exactly the native image, BUILD-INFO and SHA256SUMS'
for name in "${binary}" BUILD-INFO SHA256SUMS; do
  [[ -f "${package}/${name}" && ! -L "${package}/${name}" ]] || fail "invalid member ${name}"
  [[ "$(stat -c %h -- "${package}/${name}")" == 1 ]] || fail 'hardlinked package member'
done
[[ "$(stat -c %s -- "${package}/${binary}")" -gt 0 \
  && "$(stat -c %s -- "${package}/${binary}")" -le 268435456 \
  && "$(stat -c %s -- "${package}/BUILD-INFO")" -le 512 \
  && "$(stat -c %s -- "${package}/SHA256SUMS")" -le 512 ]] || fail 'package member exceeds its bound'
[[ "$(stat -c %a -- "${package}/${binary}")" == 555 \
  && "$(stat -c %a -- "${package}/BUILD-INFO")" == 444 \
  && "$(stat -c %a -- "${package}/SHA256SUMS")" == 444 ]] || fail 'package member modes changed'
# Compare exact bytes rather than accepting unknown/duplicate fields or another family.
cmp -s "${package}/BUILD-INFO" <(printf 'schema_version=1\nartifact_family=compiler-execution-issuer-native-v2\ngit_commit=%s\ntarget=x86_64-unknown-linux-musl\n' "${commit}") \
  || fail 'native source/family metadata mismatch'
actual="$(sha256sum -- "${package}/${binary}")"
[[ "${actual%% *}" == "${expected}" ]] || fail 'native image differs from the externally supplied pin'
cmp -s "${package}/SHA256SUMS" <(cd -- "${package}"; sha256sum -- "${binary}" BUILD-INFO) \
  || fail 'noncanonical or changed package digests'
readonly report="$(mktemp "${TMPDIR:-/tmp}/fe2o3-native-issuer-readelf.XXXXXXXX")"
trap 'rm -f -- "${report}"' EXIT
timeout --signal=TERM --kill-after=2s 30s \
  bash "${repo_root}/scripts/check-static-compiler-execution-issuer-image.sh" "${package}/${binary}" "${report}"
printf 'verified_family=compiler-execution-issuer-native-v2\nverified_image_sha256=%s\nverified_git_commit=%s\n' "${expected}" "${commit}"
