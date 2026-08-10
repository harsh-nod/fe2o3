#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly TOOL="${ROOT}/scripts/parity-row-evidence.sh"

if [[ "${FE2O3_RUN_PRIVILEGED_IMMUTABLE_TEST:-}" != 1 ]]; then
  printf '%s\n' \
    'privileged immutable ingestion test requires FE2O3_RUN_PRIVILEGED_IMMUTABLE_TEST=1' >&2
  exit 77
fi
if ((EUID != 0)); then
  printf '%s\n' 'privileged immutable ingestion test must run as root' >&2
  exit 1
fi

filesystem="${FE2O3_IMMUTABLE_TEST_FILESYSTEM:-ext4}"
case "${filesystem}" in
  ext4 | xfs) ;;
  *)
    printf 'unsupported immutable test filesystem: %s\n' "${filesystem}" >&2
    exit 2
    ;;
esac

required=(awk chattr findmnt git losetup mount mountpoint openssl sha256sum stat truncate umount)
if [[ "${filesystem}" == ext4 ]]; then
  required+=(mkfs.ext4)
else
  required+=(mkfs.xfs)
fi
for command in "${required[@]}"; do
  command -v "${command}" >/dev/null || {
    printf 'required privileged immutable test command is unavailable: %s\n' \
      "${command}" >&2
    exit 1
  }
done

control="$(mktemp -d "${FE2O3_IMMUTABLE_TEST_TEMP_ROOT:-/var/tmp}/fe2o3-immutable.XXXXXX")"
readonly control
image="${control}/archive.img"
mount_root="${control}/mount"
loop_device=""
archive=""

cleanup() {
  set +e
  if [[ -n "${archive}" && -e "${archive}" ]]; then
    chattr -i "${archive}" 2>/dev/null
    find "${archive}" -depth -exec chattr -i {} + 2>/dev/null
  fi
  if mountpoint -q "${mount_root}"; then
    umount "${mount_root}"
  fi
  if [[ -n "${loop_device}" ]]; then
    losetup -d "${loop_device}" 2>/dev/null
  fi
  rm -rf "${control}"
}
trap cleanup EXIT

mkdir "${mount_root}"
truncate -s "${FE2O3_IMMUTABLE_TEST_IMAGE_SIZE:-768M}" "${image}"
loop_device="$(losetup --find --show "${image}")"
if [[ "${filesystem}" == ext4 ]]; then
  mkfs.ext4 -q -F "${loop_device}"
else
  mkfs.xfs -q -f "${loop_device}"
fi
mount -t "${filesystem}" -o nodev,nosuid,noexec "${loop_device}" "${mount_root}"
actual_filesystem="$(findmnt -n -o FSTYPE --target "${mount_root}")"
[[ "${actual_filesystem}" == "${filesystem}" ]] || {
  printf 'mounted filesystem mismatch: expected %s, found %s\n' \
    "${filesystem}" "${actual_filesystem}" >&2
  exit 1
}

repo="${control}/repo"
keys="${control}/operator-keys"
trust="${control}/trust"
destination="${control}/ingested"
mkdir -p "${keys}"
git init -q "${repo}"
git -C "${repo}" config user.email immutable-test@example.invalid
git -C "${repo}" config user.name 'Immutable Ingestion Test'
printf 'baseline\n' >"${repo}/README"
git -C "${repo}" add README
git -C "${repo}" commit -qm baseline
baseline="$(git -C "${repo}" rev-parse HEAD)"
printf 'source\n' >>"${repo}/README"
git -C "${repo}" commit -qam source
source_commit="$(git -C "${repo}" rev-parse HEAD)"
source_tree="$(git -C "${repo}" rev-parse 'HEAD^{tree}')"
git -C "${repo}" checkout -q --detach

openssl genpkey -algorithm ED25519 -out "${keys}/attestor-private.pem"
openssl genpkey -algorithm ED25519 -out "${keys}/reviewer-private.pem"
chmod 600 "${keys}/attestor-private.pem" "${keys}/reviewer-private.pem"
openssl pkey -in "${keys}/attestor-private.pem" -pubout \
  -out "${keys}/attestor-public.pem"
openssl pkey -in "${keys}/reviewer-private.pem" -pubout \
  -out "${keys}/reviewer-public.pem"
"${TOOL}" bootstrap-production-trust \
  --output-root "${trust}" \
  --attestor-public-key "${keys}/attestor-public.pem" \
  --attestor-key-id privileged-attestor \
  --reviewer-public-key "${keys}/reviewer-public.pem" \
  --reviewer-key-id privileged-reviewer

archive="${mount_root}/archive"
mkdir -p "${archive}/logs" "${archive}/results" "${archive}/toolchains"
printf 'privileged immutable production fixture\n' >"${archive}/logs/unit.log"
printf 'toolchain closure fixture\n' >"${archive}/toolchains/toolchain.tsv"
result_id="$(printf '%s' "${filesystem}-${source_commit}" | sha256sum | awk '{ print $1 }')"
unsigned_result="${control}/result.unsigned.tsv"
{
  printf 'signed_result_schema_version\t2\n'
  printf 'result_id\t%s\n' "${result_id}"
  printf 'row_id\t04\n'
  printf 'from_status\tMissing\n'
  printf 'to_status\tPartial\n'
  printf 'baseline_commit\t%s\n' "${baseline}"
  printf 'source_commit\t%s\n' "${source_commit}"
  printf 'source_tree\t%s\n' "${source_tree}"
  printf 'evidence_class\tunit\n'
  printf 'target\tgfx942\n'
  printf 'hardware_lane\tmi300x-gfx942-privileged\n'
  printf 'execution_mode\tproduction\n'
  printf 'queue_manifest_path\t-\n'
  printf 'queue_manifest_sha256\t-\n'
  printf 'queue_id\t-\n'
  printf 'timeout_seconds\t0\n'
  printf 'toolchain_count\t1\n'
  printf 'toolchain\t0000\tfixture\ttoolchains/toolchain.tsv\t%s\t%s\n' \
    "$(stat -c %s "${archive}/toolchains/toolchain.tsv")" \
    "$(sha256sum "${archive}/toolchains/toolchain.tsv" | awk '{ print $1 }')"
  printf 'command_count\t1\n'
  printf 'command\t0000\t74727565\t0\n'
  printf 'log\t0000\tlogs/unit.log\t%s\t%s\n' \
    "$(stat -c %s "${archive}/logs/unit.log")" \
    "$(sha256sum "${archive}/logs/unit.log" | awk '{ print $1 }')"
  printf 'artifact_count\t0\n'
} >"${unsigned_result}"
"${TOOL}" sign \
  --repo "${repo}" \
  --private-key "${keys}/attestor-private.pem" \
  --key-id privileged-attestor \
  --domain production \
  --role attestor \
  "${unsigned_result}" "${archive}/results/unit.tsv"

manifest="${archive}/promotion.tsv"
{
  printf 'promotion_manifest_schema_version\t2\n'
  printf 'baseline_commit\t%s\n' "${baseline}"
  printf 'source_commit\t%s\n' "${source_commit}"
  printf 'source_tree\t%s\n' "${source_tree}"
  printf 'target\tgfx942\n'
  printf 'hardware_lane\tmi300x-gfx942-privileged\n'
  printf 'result_count\t1\n'
  printf 'result\t0000\t04\tMissing\tPartial\tunit\tresults/unit.tsv\t%s\t%s\n' \
    "$(sha256sum "${archive}/results/unit.tsv" | awk '{ print $1 }')" \
    "${result_id}"
} >"${manifest}"
printf 'evidence_set_sha256\t%s\n' \
  "$(sha256sum "${manifest}" | awk '{ print $1 }')" >>"${manifest}"
printf 'authorization_count\t0\n' >>"${manifest}"
manifest_digest="$(sha256sum "${manifest}" | awk '{ print $1 }')"

find "${archive}" -type f -exec chattr +i {} +
find "${archive}" -depth -type d -exec chattr +i {} +
"${TOOL}" ingest-archive \
  --repo "${repo}" \
  --source-root "${archive}" \
  --destination-root "${destination}" \
  --trusted-root "${trust}" \
  --trust-policy "${trust}/docs/parity-evidence/trust-policy-v2.tsv" \
  --manifest promotion.tsv \
  --expected-manifest-sha256 "${manifest_digest}" \
  --expected-baseline "${baseline}" \
  --expected-source "${source_commit}" \
  --expected-tree "${source_tree}" \
  --expected-target gfx942 \
  --expected-lane mi300x-gfx942-privileged
"${TOOL}" validate-archive \
  --repo "${repo}" \
  --archive-root "${destination}" \
  --trusted-root "${trust}" \
  --trust-policy "${trust}/docs/parity-evidence/trust-policy-v2.tsv" \
  --manifest promotion.tsv

printf 'privileged production immutable ingestion passed on %s\n' "${filesystem}"
