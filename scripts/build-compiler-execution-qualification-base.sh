#!/usr/bin/env bash
set -euo pipefail

umask 077
export LC_ALL=C
export TZ=UTC

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root

fail() {
  printf 'qualification base build failed: %s\n' "$*" >&2
  exit 1
}

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
[[ "${output}" != / && "${output_name}" != . && "${output_name}" != .. ]] ||
  fail 'output directory is unsafe'
[[ ! -e "${output}" && ! -L "${output}" ]] || fail "output already exists: ${output}"
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=normal)" ]] ||
  fail 'qualification base images require a clean source checkout'

for tool in apt-cache apt-get dpkg dpkg-deb mksquashfs unsquashfs sha256sum; do
  command -v "${tool}" >/dev/null 2>&1 || fail "required tool is missing: ${tool}"
done

# shellcheck disable=SC1091
source /etc/os-release
[[ "${ID:-}" == ubuntu && "${VERSION_ID:-}" == 24.04 ]] ||
  fail 'the V1 builder requires Ubuntu 24.04'
[[ "$(dpkg --print-architecture)" == amd64 ]] || fail 'the V1 builder requires amd64'
[[ "$(mksquashfs -version 2>&1 | awk 'NR == 1 { print $3 }')" == 4.6.1 ]] ||
  fail 'the V1 builder requires mksquashfs 4.6.1'

commit="$(git -C "${repo_root}" rev-parse --verify HEAD)"
readonly commit
source_epoch="$(git -C "${repo_root}" show -s --format=%ct HEAD)"
readonly source_epoch

mkdir -p -- "${output_parent}"
work="$(mktemp -d "${output_parent}/.${output_name}.work.XXXXXXXX")"
readonly work
partial="${output_parent}/.${output_name}.partial.$$"
readonly partial
[[ ! -e "${partial}" && ! -L "${partial}" ]] || fail 'partial output already exists'
mkdir -m 0700 -- "${partial}"

cleanup() {
  if [[ -d "${work}" ]]; then
    find "${work}" -xdev -depth -delete
  fi
  if [[ -d "${partial}" ]]; then
    find "${partial}" -xdev -depth -delete
  fi
}
trap cleanup EXIT INT TERM HUP

readonly package_roots=(
  base-files
  base-passwd
  bash
  coreutils
  dbus-broker
  init-system-helpers
  libnss-systemd
  mount
  passwd
  systemd
  systemd-sysv
  util-linux
)

mapfile -t resolved_packages < <(
  {
    printf '%s\n' "${package_roots[@]}"
    apt-cache depends --recurse --important \
      --no-recommends --no-suggests --no-conflicts --no-breaks \
      --no-replaces --no-enhances "${package_roots[@]}" |
      sed -n -E 's/^[[:space:]|]*(Pre)?Depends:[[:space:]]*([^ <][^ ]*).*/\2/p' |
      sed 's/:any$//'
  } | sort -u
)
readonly resolved_packages
[[ "${#resolved_packages[@]}" -gt 0 && "${#resolved_packages[@]}" -le 256 ]] ||
  fail 'resolved package count is outside the V1 bound'

package_lock="${repo_root}/scripts/compiler-execution-qualification-base-packages-v1.lock"
readonly package_lock
[[ -f "${package_lock}" && ! -L "${package_lock}" ]] || fail 'package lock is missing'
qualification_target="${repo_root}/deployment/qualification/systemd/fe2o3-qualification.target"
readonly qualification_target
[[ -f "${qualification_target}" && ! -L "${qualification_target}" ]] ||
  fail 'qualification target is missing'
mapfile -t lock_lines <"${package_lock}"
readonly lock_lines
[[ "${#lock_lines[@]}" -ge 6 ]] || fail 'package lock is truncated'
[[ "${lock_lines[0]}" == 'fe2o3-compiler-execution-qualification-base-packages-v1' ]] ||
  fail 'package lock header changed'
[[ "${lock_lines[1]}" == $'distribution\tubuntu' ]] || fail 'package lock distribution changed'
[[ "${lock_lines[2]}" == $'distribution_version\t24.04' ]] ||
  fail 'package lock distribution version changed'
[[ "${lock_lines[3]}" == $'architecture\tamd64' ]] ||
  fail 'package lock architecture changed'
[[ "${lock_lines[4]}" =~ ^package_count$'\t'([1-9][0-9]*)$ ]] ||
  fail 'package lock count is noncanonical'
locked_package_count="${BASH_REMATCH[1]}"
readonly locked_package_count
[[ "${locked_package_count}" -le 256 ]] || fail 'package lock count exceeds the V1 bound'

packages=()
declare -A expected_versions=()
declare -A expected_architectures=()
declare -A expected_sha256=()
previous_package=''
for line in "${lock_lines[@]:5}"; do
  IFS=$'\t' read -r record package version architecture sha256 extra <<<"${line}"
  [[ "${record}" == package && -n "${package}" && -n "${version}" &&
    ( "${architecture}" == amd64 || "${architecture}" == all ) &&
    "${sha256}" =~ ^[0-9a-f]{64}$ && -z "${extra:-}" ]] ||
    fail 'package lock record is noncanonical'
  [[ "${package}" =~ ^[a-z0-9][a-z0-9+.-]*$ ]] ||
    fail "package lock name is noncanonical: ${package}"
  [[ -z "${previous_package}" || "${package}" > "${previous_package}" ]] ||
    fail 'package lock records are not strictly sorted'
  packages+=("${package}")
  expected_versions["${package}"]="${version}"
  expected_architectures["${package}"]="${architecture}"
  expected_sha256["${package}"]="${sha256}"
  previous_package="${package}"
done
readonly packages
[[ "${#packages[@]}" -eq "${locked_package_count}" ]] ||
  fail 'package lock count does not match its records'
[[ "${resolved_packages[*]}" == "${packages[*]}" ]] ||
  fail 'current dependency closure differs from the checked-in package lock'

downloads="${work}/downloads"
root="${work}/root"
readonly downloads root
mkdir -m 0755 -- "${downloads}"
mkdir -m 0700 -- "${root}"

package_specs=()
for package in "${packages[@]}"; do
  package_specs+=("${package}=${expected_versions[${package}]}")
done
readonly package_specs

(
  cd -- "${downloads}"
  apt-get download "${package_specs[@]}" >&2
)

declare -A deb_paths=()
declare -A observed_packages=()
shopt -s nullglob
debs=("${downloads}"/*.deb)
shopt -u nullglob
[[ "${#debs[@]}" -eq "${#packages[@]}" ]] ||
  fail 'downloaded package count differs from the resolved closure'
for deb in "${debs[@]}"; do
  package="$(dpkg-deb -f "${deb}" Package)"
  version="$(dpkg-deb -f "${deb}" Version)"
  architecture="$(dpkg-deb -f "${deb}" Architecture)"
  [[ -n "${expected_versions[${package}]+present}" ]] ||
    fail "download returned an unexpected package: ${package}"
  [[ -z "${observed_packages[${package}]+present}" ]] ||
    fail "download returned a duplicate package: ${package}"
  [[ "${version}" == "${expected_versions[${package}]}" ]] ||
    fail "download returned the wrong version for ${package}"
  [[ "${architecture}" == "${expected_architectures[${package}]}" ]] ||
    fail "download returned the wrong architecture for ${package}"
  [[ "$(sha256sum "${deb}" | cut -d' ' -f1)" == "${expected_sha256[${package}]}" ]] ||
    fail "download returned the wrong digest for ${package}"
  observed_packages["${package}"]=1
  deb_paths["${package}"]="${deb}"
done

base_info="${work}/BASE-INFO"
readonly base_info
{
  printf 'fe2o3-compiler-execution-qualification-base-v1\n'
  printf 'git_commit\t%s\n' "${commit}"
  printf 'distribution\tubuntu\n'
  printf 'distribution_version\t24.04\n'
  printf 'architecture\tamd64\n'
  printf 'source_date_epoch\t%s\n' "${source_epoch}"
  printf 'mksquashfs_version\t4.6.1\n'
  printf 'package_count\t%s\n' "${#packages[@]}"
  for package in "${packages[@]}"; do
    deb="${deb_paths[${package}]}"
    printf 'package\t%s\t%s\t%s\t%s\n' \
      "${package}" \
      "${expected_versions[${package}]}" \
      "${expected_architectures[${package}]}" \
      "${expected_sha256[${package}]}"
  done
} >"${base_info}"

for package in "${packages[@]}"; do
  dpkg-deb --extract "${deb_paths[${package}]}" "${root}"
done

install -d -m 0755 "${root}/etc" "${root}/etc/systemd/system" \
  "${root}/usr/share/fe2o3/qualification-base" \
  "${root}/usr/lib/systemd/system"
install -m 0444 "${base_info}" \
  "${root}/usr/share/fe2o3/qualification-base/BASE-INFO"
printf 'root:x:0:0:root:/root:/bin/bash\n' >"${root}/etc/passwd"
printf 'root:x:0:\n' >"${root}/etc/group"
printf 'root:!*:19700:0:99999:7:::\n' >"${root}/etc/shadow"
printf 'root:!::\n' >"${root}/etc/gshadow"
printf 'passwd: files\ngroup: files\nshadow: files\nhosts: files dns\n' \
  >"${root}/etc/nsswitch.conf"
printf 'fe2o3-qualification\n' >"${root}/etc/hostname"
: >"${root}/etc/machine-id"
: >"${root}/etc/fstab"
chmod 0444 "${root}/etc/passwd" "${root}/etc/group" \
  "${root}/etc/nsswitch.conf" "${root}/etc/hostname" \
  "${root}/etc/machine-id" "${root}/etc/fstab"
chmod 0400 "${root}/etc/shadow" "${root}/etc/gshadow"

install -m 0444 "${qualification_target}" \
  "${root}/usr/lib/systemd/system/fe2o3-qualification.target"

unexpected="$(find "${root}" -xdev ! -type d ! -type f ! -type l -print -quit)"
[[ -z "${unexpected}" ]] || fail "base root contains an unsupported object: ${unexpected}"

image="${partial}/qualification-base-v1.squashfs"
readonly image
mksquashfs "${root}" "${image}" \
  -noappend -all-root -no-xattrs -no-progress -no-exports \
  -comp zstd -b 131072 -processors 1 -reproducible \
  -mkfs-time "${source_epoch}" -all-time "${source_epoch}" >/dev/null
chmod 0444 "${image}"

summary="${work}/squashfs-summary"
readonly summary
unsquashfs -s "${image}" >"${summary}"
grep -Fqx 'Compression zstd' "${summary}" || fail 'base image compression changed'
grep -Fqx 'Block size 131072' "${summary}" || fail 'base image block size changed'
grep -Fqx 'Xattrs are not stored' "${summary}" || fail 'base image stores xattrs'
grep -Fqx 'Filesystem is not exportable via NFS' "${summary}" ||
  fail 'base image export policy changed'

install -m 0444 "${base_info}" "${partial}/BASE-INFO"
(
  cd -- "${partial}"
  sha256sum BASE-INFO qualification-base-v1.squashfs >SHA256SUMS
)
chmod 0444 "${partial}/SHA256SUMS"

image_sha256="$(sha256sum "${image}" | cut -d' ' -f1)"
readonly image_sha256
image_bytes="$(stat -c %s "${image}")"
readonly image_bytes
[[ "${image_bytes}" -gt 0 && "${image_bytes}" -le $((512 * 1024 * 1024)) ]] ||
  fail 'base image exceeds the V1 byte bound'

find "${work}" -xdev -depth -delete
mv -T -- "${partial}" "${output}"
trap - EXIT INT TERM HUP
printf 'base_bundle_path=%s\n' "${output}"
printf 'base_image_sha256=%s\n' "${image_sha256}"
printf 'base_image_bytes=%s\n' "${image_bytes}"
printf 'git_commit=%s\n' "${commit}"
printf 'source_date_epoch=%s\n' "${source_epoch}"
printf 'package_count=%s\n' "${#packages[@]}"
