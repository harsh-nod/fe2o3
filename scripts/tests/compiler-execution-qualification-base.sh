#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly repo_root
readonly builder="${repo_root}/scripts/build-compiler-execution-qualification-base.sh"
readonly package_lock="${repo_root}/scripts/compiler-execution-qualification-base-packages-v1.lock"
readonly qualification_target="${repo_root}/deployment/qualification/systemd/fe2o3-qualification.target"

fail() {
  printf 'compiler-execution qualification-base contract failed: %s\n' "$*" >&2
  exit 1
}

if [[ $# -ne 0 && $# -ne 2 ]]; then
  printf 'usage: %s [FIRST_BUNDLE SECOND_BUNDLE]\n' "$0" >&2
  exit 2
fi

bash -n "${builder}"
set +e
usage="$(${builder} 2>&1)"
status=$?
set -e
[[ ${status} -eq 2 && "${usage}" == usage:* ]] || fail 'builder argument gate changed'

python3 - "${package_lock}" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
raw = path.read_bytes()
if not raw.endswith(b"\n") or b"\r" in raw or b"\0" in raw:
    raise SystemExit("package lock must be canonical newline-terminated text")
lines = raw.decode("ascii").splitlines()
expected_prefix = [
    "fe2o3-compiler-execution-qualification-base-packages-v1",
    "distribution\tubuntu",
    "distribution_version\t24.04",
    "architecture\tamd64",
]
if lines[:4] != expected_prefix:
    raise SystemExit("package lock identity changed")
if len(lines) < 6 or not re.fullmatch(r"package_count\t[1-9][0-9]*", lines[4]):
    raise SystemExit("package lock count is noncanonical")
count = int(lines[4].split("\t")[1])
records = []
for line in lines[5:]:
    fields = line.split("\t")
    if len(fields) != 5 or fields[0] != "package":
        raise SystemExit("package lock record is noncanonical")
    _, name, version, architecture, digest = fields
    if not re.fullmatch(r"[a-z0-9][a-z0-9+.-]*", name):
        raise SystemExit(f"invalid package name: {name}")
    if not version or architecture not in {"amd64", "all"}:
        raise SystemExit(f"invalid package identity: {name}")
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise SystemExit(f"invalid package digest: {name}")
    records.append(fields[1:])
names = [record[0] for record in records]
if len(records) != count or not 0 < count <= 256:
    raise SystemExit("package lock count does not match its records")
if names != sorted(set(names)):
    raise SystemExit("package lock records are not uniquely sorted")
required = {
    "base-files", "base-passwd", "bash", "coreutils", "dbus-broker",
    "init-system-helpers", "libnss-systemd", "mount", "passwd", "systemd",
    "systemd-sysv", "util-linux",
}
if not required.issubset(names):
    raise SystemExit("package lock omits a fixed root package")
PY

for expected in \
  'Requires=basic.target' \
  'After=basic.target' \
  'Wants=fe2o3-compiler-execution.socket' \
  'AllowIsolate=yes'; do
  grep -Fqx -- "${expected}" "${qualification_target}" ||
    fail "qualification target is missing ${expected}"
done

for expected in \
  "[[ \"\$(dpkg --print-architecture)\" == amd64 ]]" \
  "== 4.6.1" \
  "[[ \"\${resolved_packages[*]}\" == \"\${packages[*]}\" ]]" \
  "sha256sum \"\${deb}\"" \
  '-noappend -all-root -no-xattrs -no-progress -no-exports' \
  '-comp zstd -b 131072 -processors 1 -reproducible' \
  "-mkfs-time \"\${source_epoch}\" -all-time \"\${source_epoch}\"" \
  "mv -T -- \"\${partial}\" \"\${output}\""; do
  grep -Fq -- "${expected}" "${builder}" || fail "builder is missing ${expected}"
done

if [[ $# -eq 0 ]]; then
  printf 'compiler-execution qualification-base source contract is exact\n'
  exit 0
fi

for tool in cmp find sha256sum stat unsquashfs; do
  command -v "${tool}" >/dev/null 2>&1 || fail "bundle verification requires ${tool}"
done

expected_commit="$(git -C "${repo_root}" rev-parse --verify HEAD)"
readonly expected_commit
expected_epoch="$(git -C "${repo_root}" show -s --format=%ct HEAD)"
readonly expected_epoch

verify_bundle() {
  local bundle="$1"
  local image="${bundle}/qualification-base-v1.squashfs"
  local info="${bundle}/BASE-INFO"
  local sums="${bundle}/SHA256SUMS"
  local inventory summary

  [[ "${bundle}" == /* && -d "${bundle}" && ! -L "${bundle}" ]] ||
    fail "bundle is not an absolute real directory: ${bundle}"
  [[ "$(stat -c %a -- "${bundle}")" == 700 ]] || fail 'bundle mode is not 0700'
  inventory="$(find "${bundle}" -mindepth 1 -maxdepth 1 -printf '%y %f\n' | sort)"
  [[ "${inventory}" == $'f BASE-INFO\nf SHA256SUMS\nf qualification-base-v1.squashfs' ]] ||
    fail 'bundle inventory is not exact'
  for file in "${info}" "${sums}" "${image}"; do
    [[ -f "${file}" && ! -L "${file}" ]] || fail "bundle member is not regular: ${file}"
    [[ "$(stat -c %a -- "${file}")" == 444 ]] || fail "bundle member mode changed: ${file}"
    [[ "$(stat -c %h -- "${file}")" == 1 ]] || fail "bundle member is linked: ${file}"
  done
  (
    cd -- "${bundle}"
    sha256sum --check --strict SHA256SUMS >/dev/null
  ) || fail 'bundle digest verification failed'
  [[ "$(stat -c %s -- "${image}")" -le $((512 * 1024 * 1024)) ]] ||
    fail 'base image exceeds the V1 byte bound'

  grep -Fqx -- 'fe2o3-compiler-execution-qualification-base-v1' "${info}" ||
    fail 'base information header changed'
  grep -Fqx -- $'git_commit\t'"${expected_commit}" "${info}" ||
    fail 'base information commit differs from checkout'
  grep -Fqx -- $'source_date_epoch\t'"${expected_epoch}" "${info}" ||
    fail 'base information epoch differs from checkout'
  grep -Fqx -- $'distribution\tubuntu' "${info}" || fail 'base distribution changed'
  grep -Fqx -- $'distribution_version\t24.04' "${info}" ||
    fail 'base distribution version changed'
  grep -Fqx -- $'architecture\tamd64' "${info}" || fail 'base architecture changed'
  grep -Fqx -- $'mksquashfs_version\t4.6.1' "${info}" ||
    fail 'base SquashFS tool version changed'
  grep -Fqx -- $'package_count\t71' "${info}" || fail 'base package count changed'
  diff -u <(tail -n +6 "${package_lock}") <(tail -n +9 "${info}") >/dev/null ||
    fail 'embedded package identities differ from the package lock'

  summary="$(unsquashfs -s "${image}")"
  grep -Fqx -- 'Compression zstd' <<<"${summary}" || fail 'image compression changed'
  grep -Fqx -- 'Block size 131072' <<<"${summary}" || fail 'image block size changed'
  grep -Fqx -- 'Xattrs are not stored' <<<"${summary}" || fail 'image stores xattrs'
  grep -Fqx -- 'Filesystem is not exportable via NFS' <<<"${summary}" ||
    fail 'image export policy changed'

  cmp -s "${info}" \
    <(unsquashfs -cat "${image}" usr/share/fe2o3/qualification-base/BASE-INFO 2>/dev/null) ||
    fail 'embedded base information changed'
  cmp -s "${qualification_target}" \
    <(unsquashfs -cat "${image}" usr/lib/systemd/system/fe2o3-qualification.target 2>/dev/null) ||
    fail 'embedded qualification target changed'
}

verify_bundle "$1"
verify_bundle "$2"
for member in BASE-INFO SHA256SUMS qualification-base-v1.squashfs; do
  cmp -s -- "$1/${member}" "$2/${member}" || fail "repeated build changed ${member}"
done

printf 'compiler-execution qualification-base builds are byte-reproducible\n'
