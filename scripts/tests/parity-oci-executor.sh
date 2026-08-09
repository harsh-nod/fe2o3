#!/usr/bin/env bash

set -Eeuo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly EXECUTOR="${REPO_ROOT}/scripts/parity-oci-executor.py"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
readonly TRUSTED_ROOT="${TEST_ROOT}/trusted"
readonly SOURCE_REPO="${TEST_ROOT}/source"
readonly OCI_LAYOUT="${TEST_ROOT}/image"
readonly REQUEST="${TEST_ROOT}/request.tsv"
readonly PROFILE="${TRUSTED_ROOT}/profiles/test-v1.tsv"
readonly POLICY="${TRUSTED_ROOT}/policy.tsv"
readonly SECCOMP="${TRUSTED_ROOT}/seccomp/default.json"
readonly RUNTIME="${TEST_ROOT}/runtime"
GIT="$(command -v git)"
readonly GIT
readonly SOURCE_STAGING="${TEST_ROOT}/staging/source"

cleanup() {
  chmod -R u+w -- "${TEST_ROOT}" 2>/dev/null || true
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

sha256() {
  sha256sum -- "$1" | cut -d' ' -f1
}

size() {
  stat -c '%s' -- "$1"
}

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local output
  if output="$("$@" 2>&1)"; then
    printf 'expected %s to fail\n' "${name}" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected}"* ]]; then
    printf '%s failed for the wrong reason:\n%s\n' "${name}" "${output}" >&2
    exit 1
  fi
}

write_policy() {
  local profile_digest
  profile_digest="$(sha256 "${PROFILE}")"
  {
    printf 'oci_executor_policy_schema_version\t1\n'
    printf 'trust_domain\ttest\n'
    printf 'profile_count\t1\n'
    printf 'profile\t0000\tmi300x-test-v1\tprofiles/test-v1.tsv\t%s\t%s\n' \
      "$(size "${PROFILE}")" "${profile_digest}"
  } >"${POLICY}"
}

write_profile() {
  local manifest_digest="$1"
  local manifest_size="$2"
  local config_digest="$3"
  local config_size="$4"
  local layer_digest="$5"
  local layer_size="$6"
  cat >"${PROFILE}" <<EOF
oci_executor_profile_schema_version	1
profile_id	mi300x-test-v1
execution_mode	test
target	gfx942
hardware_lane	mi300x-gfx942-test
runtime_path	${RUNTIME}
runtime_size	$(size "${RUNTIME}")
runtime_sha256	$(sha256 "${RUNTIME}")
runtime_version_sha256	3333333333333333333333333333333333333333333333333333333333333333
runtime_info_sha256	4444444444444444444444444444444444444444444444444444444444444444
git_path	${GIT}
git_size	$(size "${GIT}")
git_sha256	$(sha256 "${GIT}")
git_version_sha256	$(env -i HOME=/nonexistent LC_ALL=C PATH=/nonexistent "${GIT}" --version | sha256sum | cut -d' ' -f1)
git_objects_path	${SOURCE_REPO}/.git/objects
source_staging_root	${SOURCE_STAGING}
source_file_limit	1024
source_byte_limit	16777216
source_index_limit	1048576
source_export_timeout_seconds	30
oci_layout_path	${OCI_LAYOUT}
oci_index_sha256	$(sha256 "${OCI_LAYOUT}/index.json")
image_reference	example.invalid/fe2o3-evidence@sha256:${manifest_digest}
image_manifest_digest	sha256:${manifest_digest}
image_manifest_size	${manifest_size}
image_config_digest	sha256:${config_digest}
image_config_size	${config_size}
image_layer_count	1
image_layer	0000	sha256:${layer_digest}	${layer_size}
entrypoint_count	1
entrypoint	0000	/opt/fe2o3/bin/evidence-entrypoint
command_count	2
command	0000	--request
command	0001	/run/fe2o3/request.tsv
environment_count	5
environment	0000	HOME	2f6e6f6e6578697374656e74
environment	0001	HOSTNAME	6665326f332d65766964656e6365
environment	0002	LC_ALL	43
environment	0003	PATH	2f6f70742f6665326f332f62696e
environment	0004	ROCR_VISIBLE_DEVICES	30
source_mount	/workspace
request_mount	/run/fe2o3/request.tsv
output_mount	/evidence
tmp_mount	/tmp
output_limit_bytes	16777216
tmp_limit_bytes	67108864
log_limit_bytes	4194304
memory_limit_bytes	8589934592
pids_limit	1024
cpu_limit_milli	8000
container_uid	65534
container_gid	65534
supplemental_gid	993
network_mode	none
read_only_root	true
cap_drop	ALL
no_new_privileges	true
seccomp_profile_path	seccomp/default.json
seccomp_profile_size	$(size "${SECCOMP}")
seccomp_profile_sha256	$(sha256 "${SECCOMP}")
device_count	2
device	0000	/dev/dri/renderD128	226	128	rwm
device	0001	/dev/kfd	235	0	rwm
host_machine_id_sha256	0000000000000000000000000000000000000000000000000000000000000000
host_kernel_release	6.8.0-test
host_kernel_notes_sha256	1111111111111111111111111111111111111111111111111111111111111111
amdgpu_module_path	/opt/test/amdgpu.ko
amdgpu_module_sha256	2222222222222222222222222222222222222222222222222222222222222222
gpu_pci_slot	0000:05:00.0
gpu_pci_id	1002:74A1
gpu_unique_id	6ced1647a296545c
EOF
  write_policy
}

verify() {
  "${EXECUTOR}" verify \
    --request "${REQUEST}" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv
}

plan() {
  "${EXECUTOR}" plan \
    --request "${REQUEST}" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv
}

preflight() {
  "${EXECUTOR}" preflight \
    --request "${REQUEST}" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv
}

mkdir -p "${TRUSTED_ROOT}/profiles" "${TRUSTED_ROOT}/seccomp" \
  "${OCI_LAYOUT}/blobs/sha256" "${SOURCE_REPO}/scripts/evidence/jobs" \
  "${SOURCE_STAGING}"
printf '%s\n' '#!/usr/bin/env bash' 'set -Eeuo pipefail' 'printf ok > /evidence/result.txt' \
  >"${SOURCE_REPO}/scripts/evidence/jobs/row-04.sh"
chmod 755 "${SOURCE_REPO}/scripts/evidence/jobs/row-04.sh"
git -C "${SOURCE_REPO}" init -q
git -C "${SOURCE_REPO}" config user.name test
git -C "${SOURCE_REPO}" config user.email test@example.invalid
git -C "${SOURCE_REPO}" add scripts/evidence/jobs/row-04.sh
git -C "${SOURCE_REPO}" commit -qm fixture
printf '#!/usr/bin/env bash\nprintf executed > %q\n' "${TEST_ROOT}/fsmonitor-executed" \
  >"${TEST_ROOT}/candidate-fsmonitor"
chmod 755 "${TEST_ROOT}/candidate-fsmonitor"
git -C "${SOURCE_REPO}" config core.fsmonitor "${TEST_ROOT}/candidate-fsmonitor"

printf '#!/usr/bin/env bash\nprintf "fixture runtime access denied\\n" >&2\nexit 1\n' \
  >"${RUNTIME}"
chmod 755 "${RUNTIME}"
printf '{"defaultAction":"SCMP_ACT_ERRNO","architectures":["SCMP_ARCH_X86_64"],"syscalls":[]}\n' \
  >"${SECCOMP}"
printf '{"imageLayoutVersion":"1.0.0"}\n' >"${OCI_LAYOUT}/oci-layout"
printf 'layer-fixture\n' >"${TEST_ROOT}/layer"
layer_digest="$(sha256 "${TEST_ROOT}/layer")"
cp "${TEST_ROOT}/layer" "${OCI_LAYOUT}/blobs/sha256/${layer_digest}"
printf '{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":["sha256:%s"]},"config":{"Env":[]}}\n' \
  "${layer_digest}" >"${TEST_ROOT}/config.json"
config_digest="$(sha256 "${TEST_ROOT}/config.json")"
cp "${TEST_ROOT}/config.json" "${OCI_LAYOUT}/blobs/sha256/${config_digest}"
printf '{"schemaVersion":2,"config":{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:%s","size":%s},"layers":[{"mediaType":"application/vnd.oci.image.layer.v1.tar","digest":"sha256:%s","size":%s}]}\n' \
  "${config_digest}" "$(size "${TEST_ROOT}/config.json")" \
  "${layer_digest}" "$(size "${TEST_ROOT}/layer")" >"${TEST_ROOT}/manifest.json"
manifest_digest="$(sha256 "${TEST_ROOT}/manifest.json")"
cp "${TEST_ROOT}/manifest.json" "${OCI_LAYOUT}/blobs/sha256/${manifest_digest}"
printf '{"schemaVersion":2,"manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:%s","size":%s}]}\n' \
  "${manifest_digest}" "$(size "${TEST_ROOT}/manifest.json")" >"${OCI_LAYOUT}/index.json"

write_profile \
  "${manifest_digest}" "$(size "${TEST_ROOT}/manifest.json")" \
  "${config_digest}" "$(size "${TEST_ROOT}/config.json")" \
  "${layer_digest}" "$(size "${TEST_ROOT}/layer")"

source_commit="$(git -C "${SOURCE_REPO}" rev-parse HEAD)"
source_tree="$(git -C "${SOURCE_REPO}" rev-parse 'HEAD^{tree}')"
job_digest="$(sha256 "${SOURCE_REPO}/scripts/evidence/jobs/row-04.sh")"
cat >"${REQUEST}" <<EOF
oci_execution_request_schema_version	1
request_id	3333333333333333333333333333333333333333333333333333333333333333
profile_id	mi300x-test-v1
source_commit	${source_commit}
source_tree	${source_tree}
job_id	row-04-hardware
job_path	scripts/evidence/jobs/row-04.sh
job_sha256	${job_digest}
EOF

output="$(verify)"
grep -F $'authorized_profile\tmi300x-test-v1' <<<"${output}" >/dev/null
grep -F $'authorization_source\tprotected-policy' <<<"${output}" >/dev/null

printf '# candidate worktree mutation\n' >>"${SOURCE_REPO}/scripts/evidence/jobs/row-04.sh"
plan_output="$(plan)"
if [[ -e "${TEST_ROOT}/fsmonitor-executed" ]]; then
  printf 'candidate Git fsmonitor executed during immutable export\n' >&2
  exit 1
fi
source_snapshot="$(awk -F $'\t' '$1 == "source_snapshot" { print $2 }' <<<"${plan_output}")"
if [[ -z "${source_snapshot}" || -e "${source_snapshot}/.git" ]]; then
  printf 'immutable source snapshot is missing or contains .git\n' >&2
  exit 1
fi
if grep -F 'candidate worktree mutation' \
  "${source_snapshot}/scripts/evidence/jobs/row-04.sh" >/dev/null; then
  printf 'source snapshot consumed candidate worktree bytes\n' >&2
  exit 1
fi
git -C "${SOURCE_REPO}" -c core.fsmonitor=false checkout -q -- \
  scripts/evidence/jobs/row-04.sh
plan_arguments="$({
  while IFS=$'\t' read -r key _ encoded; do
    if [[ "${key}" == argument ]]; then
      printf '%s\n' "$(printf '%s' "${encoded}" | xxd -r -p)"
    fi
  done
} <<<"${plan_output}")"
grep -F -- $'--network\nnone' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--read-only\n--cap-drop\nALL' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--security-opt\nno-new-privileges=true' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--log-driver\nnone' <<<"${plan_arguments}" >/dev/null
grep -F 'readonly,bind-recursive=readonly' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--device\n/dev/dri/renderD128:/dev/dri/renderD128:rwm' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--device\n/dev/kfd:/dev/kfd:rwm' <<<"${plan_arguments}" >/dev/null
if grep -F -- '--privileged' <<<"${plan_arguments}" >/dev/null || \
  grep -F -- '/var/run/docker.sock' <<<"${plan_arguments}" >/dev/null; then
  printf 'OCI plan exposed privilege or runtime control socket\n' >&2
  exit 1
fi

cp "${REQUEST}" "${TEST_ROOT}/request.good"
printf 'execution_closure\tverified\n' >>"${REQUEST}"
expect_failure candidate_verified 'unexpected trailing field' verify
cp "${TEST_ROOT}/request.good" "${REQUEST}"

sed -i 's/profile_id\tmi300x-test-v1/profile_id\tcandidate-profile/' "${REQUEST}"
expect_failure candidate_profile 'not authorized by protected policy' verify
cp "${TEST_ROOT}/request.good" "${REQUEST}"

cp "${PROFILE}" "${TEST_ROOT}/profile.good"
sed -i 's/network_mode\tnone/network_mode\thost/' "${PROFILE}"
write_policy
expect_failure host_network 'network must be disabled' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i 's/cap_drop\tALL/cap_drop\tSYS_ADMIN/' "${PROFILE}"
write_policy
expect_failure capabilities 'drop every capability' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i 's|image_reference\texample.invalid/|image_reference\t-invalid/|' "${PROFILE}"
write_policy
expect_failure unsafe_image_reference 'malformed OCI image identity' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i 's|tmp_mount\t/tmp|tmp_mount\t/workspace/tmp|' "${PROFILE}"
write_policy
expect_failure overlapping_mount 'invalid or duplicate executor mount' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i \
  's/environment\t0001\tHOSTNAME\t6665326f332d65766964656e6365/environment\t0001\tHOSTNAME\t77726f6e67/' \
  "${PROFILE}"
write_policy
expect_failure nondeterministic_hostname 'lacks the clean GPU baseline' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

printf 'corruption\n' >>"${OCI_LAYOUT}/blobs/sha256/${layer_digest}"
expect_failure layer_mutation 'OCI layer binding mismatch' verify
cp "${TEST_ROOT}/layer" "${OCI_LAYOUT}/blobs/sha256/${layer_digest}"

mv "${OCI_LAYOUT}/blobs/sha256" "${TEST_ROOT}/sha256.real"
ln -s "${TEST_ROOT}/sha256.real" "${OCI_LAYOUT}/blobs/sha256"
expect_failure oci_parent_symlink 'path contains a symlink' verify
rm "${OCI_LAYOUT}/blobs/sha256"
mv "${TEST_ROOT}/sha256.real" "${OCI_LAYOUT}/blobs/sha256"

cp "${POLICY}" "${TEST_ROOT}/policy.good"
printf '0' >>"${PROFILE}"
expect_failure profile_mutation 'profile binding mismatch' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
cp "${TEST_ROOT}/policy.good" "${POLICY}"

mv "${PROFILE}" "${TEST_ROOT}/profile.real"
ln -s "${TEST_ROOT}/profile.real" "${PROFILE}"
expect_failure profile_symlink 'path contains a symlink' verify
rm "${PROFILE}"
mv "${TEST_ROOT}/profile.real" "${PROFILE}"

ln -s "${REQUEST}" "${TEST_ROOT}/request.link"
expect_failure request_symlink 'not a single-link regular file' \
  "${EXECUTOR}" verify \
    --request "${TEST_ROOT}/request.link" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv

# Runtime unavailability cannot cross the RuntimeReadyRequest boundary. This
# command fails before host/image claims and cannot issue an execution receipt.
expect_failure operator_unavailable 'OCI runtime control plane unavailable' preflight

printf 'parity OCI executor authorization tests passed\n'
