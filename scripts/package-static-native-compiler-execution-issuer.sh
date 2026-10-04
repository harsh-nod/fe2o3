#!/usr/bin/env bash
# Build/package one explicitly native image, not a V1 deployment bundle or installer.
set -euo pipefail
umask 077
export LC_ALL=C
readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
fail() { printf 'native issuer package: %s\n' "$*" >&2; exit 1; }
[[ $# == 1 && -n "$1" ]] || fail 'usage: bash scripts/package-static-native-compiler-execution-issuer.sh NEW_OUTPUT_DIRECTORY'
readonly output="$(realpath -m -- "$1")"
[[ "${output}" != / && ! -e "${output}" && ! -L "${output}" ]] || fail 'output must not exist'
readonly parent="$(dirname -- "${output}")"
[[ -d "${parent}" && ! -L "${parent}" ]] || fail 'output parent must exist and not be a symlink'
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=normal)" ]] || fail 'requires a clean source checkout'
readonly commit="$(git -C "${repo_root}" rev-parse --verify HEAD)"
readonly target='x86_64-unknown-linux-musl'
readonly binary='fe2o3-compiler-execution-issuer-native'
target_dir="${FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR:-${repo_root}/target/static-native-issuer}"
target_dir="$(realpath -m -- "${target_dir}")"
readonly target_dir
readonly partial="$(mktemp -d "${parent}/.native-issuer-package.XXXXXXXX")"
cleanup() { chmod u+w "${partial}" 2>/dev/null || :; rm -rf -- "${partial}"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

# A serialized primary must own the build cache. These are additional finite
# child bounds, not permission to run a second concurrent guarded build.
timeout --signal=TERM --kill-after=10s 900s \
  prlimit --cpu=900:900 --as=8589934592:8589934592 --core=0:0 -- \
  env FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR="${target_dir}" CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
  bash "${repo_root}/scripts/build-static-compiler-execution-issuer.sh" --native
install -m 0555 -- "${target_dir}/${target}/release/${binary}" "${partial}/${binary}"
printf 'schema_version=1\nartifact_family=compiler-execution-issuer-native-v2\ngit_commit=%s\ntarget=%s\n' \
  "${commit}" "${target}" >"${partial}/BUILD-INFO"
chmod 0444 "${partial}/BUILD-INFO"
(cd -- "${partial}"; sha256sum -- "${binary}" BUILD-INFO) >"${partial}/SHA256SUMS"
chmod 0444 "${partial}/SHA256SUMS"
digest="$(sha256sum -- "${partial}/${binary}")"
digest="${digest%% *}"
bash "${repo_root}/scripts/verify-native-compiler-execution-issuer-package.sh" "${partial}" "${digest}" "${commit}"
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=normal)" \
  && "$(git -C "${repo_root}" rev-parse HEAD)" == "${commit}" ]] || fail 'source changed during packaging'
[[ ! -e "${output}" && ! -L "${output}" ]] || fail 'output appeared during build'
chmod 0555 "${partial}"
mv -T -- "${partial}" "${output}"
trap - EXIT INT TERM HUP
printf 'artifact_family=compiler-execution-issuer-native-v2\nimage_path=%s/%s\nimage_sha256=%s\ngit_commit=%s\n' \
  "${output}" "${binary}" "${digest}" "${commit}"
