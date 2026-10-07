#!/usr/bin/env bash
set -euo pipefail

family=v3
if [[ ${1-} == --v1 || ${1-} == --v3 ]]; then
  family="${1#--}"
  shift
fi
if [[ $# -ne 1 || -z "$1" || "$1" == --* ]]; then
  printf 'usage: %s [--v1|--v3] OUTPUT_DIRECTORY\n' "$0" >&2
  exit 2
fi
readonly family
# The whole family is selected together; no individual image or schema override.
if [[ ${family} == v1 ]]; then
  schema_version=1
  content_file_count=13
  entrypoint_suffix=-v1
  service_suffix=-v1
  image_suffix=""
  issuer_suffix=""
  issuer_arguments=()
  manifest_arguments=()
  cache_suffix=-v1
else
  schema_version=3
  content_file_count=12
  entrypoint_suffix=""
  service_suffix=""
  image_suffix=-v3
  issuer_suffix=-conditional
  issuer_arguments=(--conditional)
  manifest_arguments=(--v3)
  cache_suffix=""
fi
readonly schema_version content_file_count entrypoint_suffix service_suffix
readonly image_suffix issuer_suffix issuer_arguments manifest_arguments cache_suffix

readonly jobs="${CARGO_BUILD_JOBS-1}"
if [[ ! "${jobs}" =~ ^([1-9]|1[0-6])$ ]]; then
  printf 'CARGO_BUILD_JOBS must be a canonical integer from 1 through 16\n' >&2
  exit 2
fi
# One explicit build-worker bound, including every child Cargo invocation.
export CARGO_BUILD_JOBS="${jobs}"
export CMAKE_BUILD_PARALLEL_LEVEL="${jobs}"

umask 077

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_root="${FE2O3_STATIC_DEPLOYMENT_TARGET_DIR:-${repo_root}/target/static-deployment${cache_suffix}}"
readonly cargo_target_dir="${target_root}/cargo"
readonly target="x86_64-unknown-linux-musl"

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

# Serial helpers share Cargo's fingerprinted dependencies, never premeasured images.
FE2O3_STATIC_COORDINATOR_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-compiler-execution-coordinator.sh" "${family}"
if [[ ${family} == v1 ]]; then
  FE2O3_STATIC_CLIENT_CHECK_TARGET_DIR="${cargo_target_dir}" \
    "${repo_root}/scripts/build-static-compiler-execution-client-check.sh"
fi
FE2O3_STATIC_SUPERVISOR_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-compiler-execution-supervisor.sh" "${family}"
FE2O3_STATIC_ISSUER_TARGET_DIR="${cargo_target_dir}" \
FE2O3_STATIC_CONDITIONAL_ISSUER_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-compiler-execution-issuer.sh" "${issuer_arguments[@]}"
FE2O3_STATIC_ANCHOR_HELPER_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-external-anchor-provisioning-helper.sh" "${family}"
FE2O3_STATIC_ANCHOR_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-external-anchor-service.sh" "${family}"
FE2O3_STATIC_PROVISIONER_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-compiler-execution-provisioner.sh" "${family}"
FE2O3_STATIC_DEPLOYMENT_VERIFIER_TARGET_DIR="${cargo_target_dir}" \
  "${repo_root}/scripts/build-static-compiler-execution-deployment-verifier.sh"

cmake \
  -S "${repo_root}/tools/fe2o3-static-preexec-launcher" \
  -B "${target_root}/launcher" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=/usr/bin/cc
cmake --build "${target_root}/launcher" --parallel "${jobs}"
ctest --test-dir "${target_root}/launcher" --output-on-failure

readonly usr_dir="${partial}/usr"
readonly libexec_dir="${usr_dir}/libexec"
readonly image_dir="${libexec_dir}/fe2o3"
readonly systemd_dir="${partial}/systemd"
readonly sysusers_dir="${partial}/sysusers.d"
readonly tmpfiles_dir="${partial}/tmpfiles.d"
readonly manifest_generator="${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-manifest"
readonly deployment_verifier="${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-deployment-verify"
install -d -m 0700 -- \
  "${usr_dir}" \
  "${libexec_dir}" \
  "${image_dir}" \
  "${systemd_dir}" \
  "${sysusers_dir}" \
  "${tmpfiles_dir}"

install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-coordinator${entrypoint_suffix}" \
  "${image_dir}/fe2o3-compiler-execution-coordinator"
if [[ ${family} == v1 ]]; then
  install -m 0555 -- \
    "${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-client-check" \
    "${image_dir}/fe2o3-compiler-execution-client-check"
fi
install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-supervisor${image_suffix}" \
  "${image_dir}/fe2o3-compiler-execution-supervisor${image_suffix}"
install -m 0555 -- \
  "${target_root}/launcher/fe2o3-static-preexec-launcher" \
  "${image_dir}/fe2o3-static-preexec-launcher"
install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-issuer${issuer_suffix}" \
  "${image_dir}/fe2o3-compiler-execution-issuer${issuer_suffix}"
install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-external-anchor-provisioning-helper${image_suffix}" \
  "${image_dir}/fe2o3-external-anchor-provisioning-helper${image_suffix}"
install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-external-anchor-service${image_suffix}" \
  "${image_dir}/fe2o3-external-anchor-service${image_suffix}"
install -m 0555 -- \
  "${cargo_target_dir}/${target}/release/fe2o3-compiler-execution-provision${entrypoint_suffix}" \
  "${image_dir}/fe2o3-compiler-execution-provision"

install -m 0444 -- \
  "${repo_root}/deployment/systemd/fe2o3-compiler-execution${service_suffix}.service" \
  "${systemd_dir}/fe2o3-compiler-execution.service"
install -m 0444 -- \
  "${repo_root}/deployment/sysusers.d/fe2o3-compiler-execution.conf" \
  "${sysusers_dir}/fe2o3-compiler-execution.conf"
install -m 0444 -- \
  "${repo_root}/deployment/tmpfiles.d/fe2o3-compiler-execution.conf" \
  "${tmpfiles_dir}/fe2o3-compiler-execution.conf"

printf 'schema_version=%s\ngit_commit=%s\nsource_date_epoch=%s\ntarget=%s\n' \
  "${schema_version}" "${commit}" "${source_epoch}" "${target}" >"${partial}/BUILD-INFO"
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

manifest_report="$("${manifest_generator}" "${manifest_arguments[@]}" "${partial}" "${commit}" "${target}")"
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

verification_report="$("${deployment_verifier}" "${manifest_arguments[@]}" "${partial}" "${manifest_sha256}" "${commit}")"
readonly verification_report
expected_verification_report="$(
  printf 'verified_git_commit=%s\nverified_target=%s\nverified_manifest_sha256=%s\nverified_file_count=%s' \
    "${commit}" "${target}" "${manifest_sha256}" "${content_file_count}"
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
