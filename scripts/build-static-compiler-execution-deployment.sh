#!/usr/bin/env bash
set -euo pipefail

umask 077

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_root="${FE2O3_STATIC_DEPLOYMENT_TARGET_DIR:-${repo_root}/target/static-deployment}"
readonly target="x86_64-unknown-linux-musl"

if [[ $# -ne 1 || -z "$1" ]]; then
  printf 'usage: %s OUTPUT_DIRECTORY\n' "$0" >&2
  exit 2
fi

output="$(realpath -m -- "$1")"
readonly output
output_parent="$(dirname -- "${output}")"
readonly output_parent
output_name="$(basename -- "${output}")"
readonly output_name
if [[ "${output}" == / || "${output_name}" == . || "${output_name}" == .. ]]; then
  printf 'deployment bundle output is unsafe\n' >&2
  exit 2
fi
if [[ -e "${output}" || -L "${output}" ]]; then
  printf 'deployment bundle output already exists: %s\n' "${output}" >&2
  exit 1
fi
if [[ -n "$(git -C "${repo_root}" status --porcelain --untracked-files=normal)" ]]; then
  printf 'deployment bundles require a clean source checkout\n' >&2
  exit 1
fi
commit="$(git -C "${repo_root}" rev-parse --verify HEAD)"
readonly commit
source_epoch="$(git -C "${repo_root}" show -s --format=%ct HEAD)"
readonly source_epoch
export LC_ALL=C
export TZ=UTC
export SOURCE_DATE_EPOCH="${source_epoch}"
export CARGO_INCREMENTAL=0

mkdir -p -- "${output_parent}"
readonly partial="${output_parent}/.${output_name}.partial.$$.${RANDOM}"
if [[ -e "${partial}" || -L "${partial}" ]]; then
  printf 'deployment bundle temporary path already exists\n' >&2
  exit 1
fi
mkdir -m 0700 -- "${partial}"

cleanup() {
  if [[ -d "${partial}" ]]; then
    find "${partial}" -xdev -depth -delete
  fi
}
trap cleanup EXIT INT TERM HUP

if [[ -L "${target_root}" ]]; then
  printf 'deployment target root must not be a symlink\n' >&2
  exit 1
fi
mkdir -p -- "${target_root}"
chmod 0700 -- "${target_root}"

FE2O3_STATIC_COORDINATOR_TARGET_DIR="${target_root}/coordinator" \
  "${repo_root}/scripts/build-static-compiler-execution-coordinator.sh"
FE2O3_STATIC_SUPERVISOR_TARGET_DIR="${target_root}/supervisor" \
  "${repo_root}/scripts/build-static-compiler-execution-supervisor.sh"
FE2O3_STATIC_ISSUER_TARGET_DIR="${target_root}/issuer" \
  "${repo_root}/scripts/build-static-compiler-execution-issuer.sh"
FE2O3_STATIC_ANCHOR_HELPER_TARGET_DIR="${target_root}/anchor-helper" \
  "${repo_root}/scripts/build-static-external-anchor-provisioning-helper.sh"
FE2O3_STATIC_ANCHOR_TARGET_DIR="${target_root}/anchor" \
  "${repo_root}/scripts/build-static-external-anchor-service.sh"
FE2O3_STATIC_PROVISIONER_TARGET_DIR="${target_root}/provisioner" \
  "${repo_root}/scripts/build-static-compiler-execution-provisioner.sh"
FE2O3_STATIC_DEPLOYMENT_VERIFIER_TARGET_DIR="${target_root}/deployment-verifier" \
  "${repo_root}/scripts/build-static-compiler-execution-deployment-verifier.sh"

cmake \
  -S "${repo_root}/tools/fe2o3-static-preexec-launcher" \
  -B "${target_root}/launcher" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=/usr/bin/cc
cmake --build "${target_root}/launcher" --parallel
ctest --test-dir "${target_root}/launcher" --output-on-failure

readonly image_dir="${partial}/usr/libexec/fe2o3"
readonly systemd_dir="${partial}/systemd"
readonly sysusers_dir="${partial}/sysusers.d"
readonly tmpfiles_dir="${partial}/tmpfiles.d"
readonly manifest_generator="${target_root}/deployment-verifier/${target}/release/fe2o3-compiler-execution-manifest"
readonly deployment_verifier="${target_root}/deployment-verifier/${target}/release/fe2o3-compiler-execution-deployment-verify"
install -d -m 0700 -- "${image_dir}" "${systemd_dir}" "${sysusers_dir}" "${tmpfiles_dir}"

install -m 0555 -- \
  "${target_root}/coordinator/${target}/release/fe2o3-compiler-execution-coordinator" \
  "${image_dir}/fe2o3-compiler-execution-coordinator"
install -m 0555 -- \
  "${target_root}/supervisor/${target}/release/fe2o3-compiler-execution-supervisor" \
  "${image_dir}/fe2o3-compiler-execution-supervisor"
install -m 0555 -- \
  "${target_root}/launcher/fe2o3-static-preexec-launcher" \
  "${image_dir}/fe2o3-static-preexec-launcher"
install -m 0555 -- \
  "${target_root}/issuer/${target}/release/fe2o3-compiler-execution-issuer" \
  "${image_dir}/fe2o3-compiler-execution-issuer"
install -m 0555 -- \
  "${target_root}/anchor-helper/${target}/release/fe2o3-external-anchor-provisioning-helper" \
  "${image_dir}/fe2o3-external-anchor-provisioning-helper"
install -m 0555 -- \
  "${target_root}/anchor/${target}/release/fe2o3-external-anchor-service" \
  "${image_dir}/fe2o3-external-anchor-service"
install -m 0555 -- \
  "${target_root}/provisioner/${target}/release/fe2o3-compiler-execution-provision" \
  "${image_dir}/fe2o3-compiler-execution-provision"

install -m 0444 -- \
  "${repo_root}/deployment/systemd/fe2o3-compiler-execution.service" \
  "${systemd_dir}/fe2o3-compiler-execution.service"
install -m 0444 -- \
  "${repo_root}/deployment/systemd/fe2o3-compiler-execution.socket" \
  "${systemd_dir}/fe2o3-compiler-execution.socket"
install -m 0444 -- \
  "${repo_root}/deployment/sysusers.d/fe2o3-compiler-execution.conf" \
  "${sysusers_dir}/fe2o3-compiler-execution.conf"
install -m 0444 -- \
  "${repo_root}/deployment/tmpfiles.d/fe2o3-compiler-execution.conf" \
  "${tmpfiles_dir}/fe2o3-compiler-execution.conf"

printf 'schema_version=1\ngit_commit=%s\nsource_date_epoch=%s\ntarget=%s\n' \
  "${commit}" "${source_epoch}" "${target}" >"${partial}/BUILD-INFO"
chmod 0444 "${partial}/BUILD-INFO"

(
  cd -- "${partial}"
  find . -type f ! -name SHA256SUMS -print0 \
    | LC_ALL=C sort -z \
    | xargs -0 sha256sum
) >"${partial}/SHA256SUMS"
chmod 0444 "${partial}/SHA256SUMS"

(
  cd -- "${partial}"
  sha256sum --check --strict SHA256SUMS
)

manifest_report="$("${manifest_generator}" "${partial}" "${commit}" "${target}")"
readonly manifest_report
manifest_sha256="$(
  printf '%s\n' "${manifest_report}" \
    | /usr/bin/sed -n 's/^manifest_sha256=\([0-9a-f]\{64\}\)$/\1/p'
)"
readonly manifest_sha256
manifest_byte_len="$(
  printf '%s\n' "${manifest_report}" \
    | /usr/bin/sed -n 's/^manifest_byte_len=\([1-9][0-9]*\)$/\1/p'
)"
readonly manifest_byte_len
if [[ -z "${manifest_sha256}" || -z "${manifest_byte_len}" \
  || "${manifest_report}" != "manifest_sha256=${manifest_sha256}"$'\n'"manifest_byte_len=${manifest_byte_len}" ]]; then
  printf 'deployment manifest generator returned a noncanonical report\n' >&2
  exit 1
fi

verification_report="$("${deployment_verifier}" "${partial}" "${manifest_sha256}" "${commit}")"
readonly verification_report
expected_verification_report="$(
  printf 'verified_git_commit=%s\nverified_target=%s\nverified_manifest_sha256=%s\nverified_file_count=13' \
    "${commit}" "${target}" "${manifest_sha256}"
)"
readonly expected_verification_report
if [[ "${verification_report}" != "${expected_verification_report}" ]]; then
  printf 'deployment verifier returned a noncanonical report\n' >&2
  exit 1
fi

mv -- "${partial}" "${output}"
trap - EXIT INT TERM HUP
printf 'bundle_path=%s\n' "${output}"
printf 'manifest_sha256=%s\n' "${manifest_sha256}"
printf 'git_commit=%s\n' "${commit}"
