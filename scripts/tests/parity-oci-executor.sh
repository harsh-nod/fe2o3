#!/usr/bin/env bash

set -Eeuo pipefail
umask 022

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
readonly OUTPUT_STAGING="${TEST_ROOT}/staging/output"

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
output_staging_root	${OUTPUT_STAGING}
artifact_stream_protocol	fe2o3-artifact-stream-v1
source_file_limit	1024
source_byte_limit	16777216
source_index_limit	1048576
source_export_timeout_seconds	30
operator_uid	$(id -u)
operator_gid	$(id -g)
oci_layout_path	${OCI_LAYOUT}
oci_index_sha256	$(sha256 "${OCI_LAYOUT}/index.json")
oci_index_size	$(size "${OCI_LAYOUT}/index.json")
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
environment_count	6
environment	0000	HIP_VISIBLE_DEVICES	36636564313634376132393635343563
environment	0001	HOME	2f6e6f6e6578697374656e74
environment	0002	HOSTNAME	6665326f332d65766964656e6365
environment	0003	LC_ALL	43
environment	0004	PATH	2f6f70742f6665326f332f62696e
environment	0005	ROCR_VISIBLE_DEVICES	36636564313634376132393635343563
source_mount	/workspace
request_mount	/run/fe2o3/request.tsv
output_mount	/evidence
tmp_mount	/tmp
output_limit_bytes	16777216
tmp_limit_bytes	67108864
shm_limit_bytes	33554432
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

install_image_config() {
  local source="$1"
  cp "${source}" "${TEST_ROOT}/config.json"
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
}

verify() {
  "${EXECUTOR}" verify \
    --request "${REQUEST}" \
    --request-owner-uid "$(id -u)" \
    --request-owner-gid "$(id -g)" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv \
    --policy-size "$(size "${POLICY}")" \
    --policy-sha256 "$(sha256 "${POLICY}")" \
    --trusted-owner-uid "$(id -u)" \
    --trusted-owner-gid "$(id -g)" \
    --trust-file-contract descriptor-stable
}

plan() {
  "${EXECUTOR}" plan \
    --request "${REQUEST}" \
    --request-owner-uid "$(id -u)" \
    --request-owner-gid "$(id -g)" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv \
    --policy-size "$(size "${POLICY}")" \
    --policy-sha256 "$(sha256 "${POLICY}")" \
    --trusted-owner-uid "$(id -u)" \
    --trusted-owner-gid "$(id -g)" \
    --trust-file-contract descriptor-stable
}

preflight() {
  "${EXECUTOR}" preflight \
    --request "${REQUEST}" \
    --request-owner-uid "$(id -u)" \
    --request-owner-gid "$(id -g)" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv \
    --policy-size "$(size "${POLICY}")" \
    --policy-sha256 "$(sha256 "${POLICY}")" \
    --trusted-owner-uid "$(id -u)" \
    --trusted-owner-gid "$(id -g)" \
    --trust-file-contract descriptor-stable
}

verify_request_path() {
  local path="$1"
  local owner_uid="${2:-$(id -u)}"
  local owner_gid="${3:-$(id -g)}"
  "${EXECUTOR}" verify \
    --request "${path}" \
    --request-owner-uid "${owner_uid}" \
    --request-owner-gid "${owner_gid}" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv \
    --policy-size "$(size "${POLICY}")" \
    --policy-sha256 "$(sha256 "${POLICY}")" \
    --trusted-owner-uid "$(id -u)" \
    --trusted-owner-gid "$(id -g)" \
    --trust-file-contract descriptor-stable
}

mkdir -p "${TRUSTED_ROOT}/profiles" "${TRUSTED_ROOT}/seccomp" \
  "${OCI_LAYOUT}/blobs/sha256" "${SOURCE_REPO}/scripts/evidence/jobs" \
  "${SOURCE_STAGING}" "${OUTPUT_STAGING}"
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
  "${layer_digest}" >"${TEST_ROOT}/config.base.json"
install_image_config "${TEST_ROOT}/config.base.json"

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
request_snapshot="$(awk -F $'\t' '$1 == "request_snapshot" { print $2 }' <<<"${plan_output}")"
source_manifest="$(awk -F $'\t' '$1 == "source_manifest" { print $2 }' <<<"${plan_output}")"
artifact_stream="$(awk -F $'\t' '$1 == "artifact_stream_path" { print $2 }' <<<"${plan_output}")"
stderr_stream="$(awk -F $'\t' '$1 == "stderr_stream_path" { print $2 }' <<<"${plan_output}")"
if [[ -z "${source_snapshot}" || -e "${source_snapshot}/.git" ]]; then
  printf 'immutable source snapshot is missing or contains .git\n' >&2
  exit 1
fi
if grep -F 'candidate worktree mutation' \
  "${source_snapshot}/scripts/evidence/jobs/row-04.sh" >/dev/null; then
  printf 'source snapshot consumed candidate worktree bytes\n' >&2
  exit 1
fi
grep -F $'container_name\tfe2o3-evidence-3333333333333333333333333333333333333333333333333333333333333333' \
  <<<"${plan_output}" >/dev/null
grep -F $'artifact_stream_protocol\tfe2o3-artifact-stream-v1' \
  <<<"${plan_output}" >/dev/null
if [[ "${artifact_stream}" != "${OUTPUT_STAGING}/"* || \
  "${stderr_stream}" != "${OUTPUT_STAGING}/"* || \
  ! -f "${artifact_stream}" || ! -f "${stderr_stream}" || \
  "$(stat -c '%a' "${artifact_stream}")" != 600 || \
  "$(stat -c '%a' "${stderr_stream}")" != 600 ]]; then
  printf 'durable bounded stream staging is invalid\n' >&2
  exit 1
fi
git -C "${SOURCE_REPO}" -c core.fsmonitor=false checkout -q -- \
  scripts/evidence/jobs/row-04.sh
PYTHONDONTWRITEBYTECODE=1 python3 - "${EXECUTOR}" "${source_snapshot}" "${source_manifest}" \
  "${request_snapshot}" "${artifact_stream}" "${stderr_stream}" <<'PY'
import importlib.util
import os
from pathlib import Path
import sys

(
    module_path,
    source_text,
    manifest_text,
    request_text,
    artifact_text,
    log_text,
) = sys.argv[1:]
spec = importlib.util.spec_from_file_location("parity_oci_executor", module_path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

source = Path(source_text)
manifest = Path(manifest_text)
request = Path(request_text)
root = source.parent
root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
source_fd = os.open(source, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
request_fd = os.open(request, os.O_RDONLY | os.O_NOFOLLOW)
source_info = os.fstat(source_fd)
request_info = os.fstat(request_fd)
snapshot = module.SourceSnapshot(
    source,
    manifest,
    request,
    root_fd,
    source_fd,
    request_fd,
    source_info.st_dev,
    source_info.st_ino,
    request_info.st_dev,
    request_info.st_ino,
    1,
    1,
    "0" * 64,
)

renamed_source = source.with_name(source.name + ".renamed")
source.rename(renamed_source)
source.mkdir(mode=0o700)
try:
    try:
        module.verify_retained_snapshot(snapshot)
    except module.ExecutorError as error:
        assert "replaced" in str(error)
    else:
        raise AssertionError("directory rename swap was accepted")
finally:
    source.rmdir()
    renamed_source.rename(source)

renamed_request = request.with_name(request.name + ".renamed")
request.rename(renamed_request)
request.symlink_to(renamed_request)
try:
    try:
        module.verify_retained_snapshot(snapshot)
    except module.ExecutorError as error:
        assert "replaced" in str(error)
    else:
        raise AssertionError("request symlink swap was accepted")
finally:
    request.unlink()
    renamed_request.rename(request)
    snapshot.close()

durability = root / "source-durability-fixture"
(durability / "alpha" / "beta").mkdir(parents=True, mode=0o700)
(durability / "alpha" / "gamma").mkdir(mode=0o700)
(durability / "kernel.rs").write_bytes(b"fn kernel() {}\n")
(durability / "alpha" / "beta" / "one").write_bytes(b"one\n")
(durability / "alpha" / "gamma" / "two").write_bytes(b"two\n")
real_fchmod = module.os.fchmod
real_fsync = module.os.fsync


def descriptor_name(file_fd):
    return Path(os.readlink(f"/proc/self/fd/{file_fd}")).name


file_events = []


def record_file_chmod(file_fd, mode):
    file_events.append(("chmod", descriptor_name(file_fd), mode))
    real_fchmod(file_fd, mode)


def record_file_fsync(file_fd):
    file_events.append(("fsync", descriptor_name(file_fd)))
    real_fsync(file_fd)


kernel_fd = os.open(durability / "kernel.rs", os.O_RDONLY | os.O_NOFOLLOW)
module.os.fchmod = record_file_chmod
module.os.fsync = record_file_fsync
try:
    module.finalize_source_file(kernel_fd, 0o444, "file-order fixture")
finally:
    module.os.fchmod = real_fchmod
    module.os.fsync = real_fsync
    os.close(kernel_fd)
assert file_events == [
    ("chmod", "kernel.rs", 0o444),
    ("fsync", "kernel.rs"),
]
assert (durability / "kernel.rs").stat().st_mode & 0o777 == 0o444

directory_events = []


def record_directory_chmod(file_fd, mode):
    directory_events.append(("chmod", descriptor_name(file_fd), mode))
    real_fchmod(file_fd, mode)


def record_directory_fsync(file_fd):
    directory_events.append(("fsync", descriptor_name(file_fd)))
    real_fsync(file_fd)


durability_fd = os.open(durability, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
durability_root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
module.os.fchmod = record_directory_chmod
module.os.fsync = record_directory_fsync
try:
    module.finalize_source_directories(
        durability_fd,
        {"alpha", "alpha/beta", "alpha/gamma"},
        durability_root_fd,
    )
finally:
    module.os.fchmod = real_fchmod
    module.os.fsync = real_fsync
assert directory_events == [
    ("chmod", "beta", 0o555),
    ("fsync", "beta"),
    ("chmod", "gamma", 0o555),
    ("fsync", "gamma"),
    ("chmod", "alpha", 0o555),
    ("fsync", "alpha"),
    ("chmod", durability.name, 0o555),
    ("fsync", durability.name),
    ("fsync", root.name),
]


def reject_file_fsync(file_fd):
    del file_fd
    raise OSError("injected source file fsync failure")


kernel_fd = os.open(durability / "kernel.rs", os.O_RDONLY | os.O_NOFOLLOW)
module.os.fsync = reject_file_fsync
try:
    try:
        module.finalize_source_file(kernel_fd, 0o444, "file-failure fixture")
    except module.ExecutorError as error:
        assert "cannot durably finalize file-failure fixture" in str(error)
    else:
        raise AssertionError("source file fsync failure escaped or was accepted")
finally:
    module.os.fsync = real_fsync
    os.close(kernel_fd)


def reject_nested_fsync(file_fd):
    if descriptor_name(file_fd) == "beta":
        raise OSError("injected nested directory fsync failure")
    real_fsync(file_fd)


module.os.fsync = reject_nested_fsync
try:
    try:
        module.finalize_source_directories(
            durability_fd,
            {"alpha", "alpha/beta", "alpha/gamma"},
            durability_root_fd,
        )
    except module.ExecutorError as error:
        assert "source directory alpha/beta" in str(error)
    else:
        raise AssertionError("nested source directory fsync failure escaped or was accepted")
finally:
    module.os.fsync = real_fsync
    os.close(durability_root_fd)
    os.close(durability_fd)

artifact = Path(artifact_text)
log = Path(log_text)
output = artifact.parent
output_root = output.parent
output_root_fd = os.open(output_root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
output_fd = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
artifact_fd = os.open(artifact, os.O_WRONLY | os.O_NOFOLLOW)
log_fd = os.open(log, os.O_WRONLY | os.O_NOFOLLOW)
output_info = os.fstat(output_fd)
stage = module.OutputStage(
    output,
    artifact,
    log,
    output_root_fd,
    output_fd,
    artifact_fd,
    log_fd,
    output_info.st_dev,
    output_info.st_ino,
)
renamed_output = output.with_name(output.name + ".renamed")
output.rename(renamed_output)
output.symlink_to(renamed_output, target_is_directory=True)
try:
    try:
        module.verify_retained_output(stage)
    except module.ExecutorError as error:
        assert "replaced" in str(error)
    else:
        raise AssertionError("output symlink swap was accepted")
finally:
    output.unlink()
    renamed_output.rename(output)
    stage.close()

class Fixture:
    pass


profile = Fixture()
profile.output_staging_root = str(output_root)
request = Fixture()
request.request_id = "4" * 64
sync_order = []
real_fsync = module.os.fsync


def record_fsync(file_fd):
    sync_order.append(Path(os.readlink(f"/proc/self/fd/{file_fd}")).name)


module.os.fsync = record_fsync
durable_stage = module.stage_output(profile, request)
try:
    assert sync_order == [
        "artifacts.stream",
        "stderr.log",
        f"execution-{request.request_id}",
        output_root.name,
    ]
finally:
    durable_stage.close()

request.request_id = "5" * 64
sync_calls = 0


def fail_second_fsync(file_fd):
    del file_fd
    global sync_calls
    sync_calls += 1
    if sync_calls == 2:
        raise OSError("fixture fsync failure")


module.os.fsync = fail_second_fsync
try:
    module.stage_output(profile, request)
except module.ExecutorError as error:
    assert "cannot durably initialize output staging" in str(error)
    assert sync_calls == 2
else:
    raise AssertionError("output fsync failure was accepted")
finally:
    module.os.fsync = real_fsync
PY
PYTHONDONTWRITEBYTECODE=1 python3 - "${EXECUTOR}" <<'PY'
import importlib.util
import sys
import time

module_path = sys.argv[1]
spec = importlib.util.spec_from_file_location("parity_oci_executor_bounds", module_path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
environment = {"HOME": "/nonexistent", "LC_ALL": "C", "PATH": "/nonexistent"}

try:
    module.run_bounded(
        ["/bin/sh", "-c", "while :; do printf 0123456789; done"],
        label="overflow fixture",
        environment=environment,
        timeout_seconds=5,
        stdout_limit=1024,
        stderr_limit=1024,
    )
except module.ExecutorError as error:
    assert "exceeds protected limit" in str(error)
else:
    raise AssertionError("unbounded subprocess output was accepted")

started = time.monotonic()
try:
    module.run_bounded(
        ["/bin/sh", "-c", "/bin/sleep 30 & wait"],
        label="timeout fixture",
        environment=environment,
        timeout_seconds=1,
        stdout_limit=1024,
        stderr_limit=1024,
    )
except module.ExecutorError as error:
    assert "protected timeout" in str(error)
    assert time.monotonic() - started < 5
else:
    raise AssertionError("subprocess timeout was accepted")

diff_id = "sha256:" + "a" * 64
module.validate_runtime_rootfs(
    {"Type": "layers", "Layers": [diff_id]}, (diff_id,)
)
for malformed in (
    None,
    [],
    {"Layers": [diff_id]},
    {"Type": "layers", "Layers": diff_id},
    {"Type": "layers", "Layers": [1]},
):
    try:
        module.validate_runtime_rootfs(malformed, (diff_id,))
    except module.ExecutorError:
        pass
    else:
        raise AssertionError("malformed runtime RootFS.Layers was accepted")

deep_json = b'{"nested":' * 2048 + b'{}' + b'}' * 2048
try:
    module.strict_json_object(deep_json, "deep fixture")
except module.ExecutorError:
    pass
else:
    raise AssertionError("recursive JSON parser input was accepted")
PY
plan_arguments="$({
  while IFS=$'\t' read -r key _ encoded; do
    if [[ "${key}" == argument ]]; then
      printf '%s\n' "$(printf '%s' "${encoded}" | xxd -r -p)"
    fi
  done
} <<<"${plan_output}")"
grep -F -- $'--network\nnone' <<<"${plan_arguments}" >/dev/null
grep -F -- '--pull=never' <<<"${plan_arguments}" >/dev/null
grep -F -- '--no-healthcheck' <<<"${plan_arguments}" >/dev/null
grep -F -- '--cgroupns=private' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--shm-size\n33554432' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--read-only\n--cap-drop\nALL' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--security-opt\nno-new-privileges=true' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--log-driver\nnone' <<<"${plan_arguments}" >/dev/null
grep -F 'readonly,bind-recursive=readonly' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--device\n/dev/dri/renderD128:/dev/dri/renderD128:rwm' <<<"${plan_arguments}" >/dev/null
grep -F -- $'--device\n/dev/kfd:/dev/kfd:rwm' <<<"${plan_arguments}" >/dev/null
grep -F -- 'org.fe2o3.evidence.request-id=3333333333333333333333333333333333333333333333333333333333333333' \
  <<<"${plan_arguments}" >/dev/null
if grep -F -- '--privileged' <<<"${plan_arguments}" >/dev/null || \
  grep -F -- '/var/run/docker.sock' <<<"${plan_arguments}" >/dev/null || \
  grep -F -- 'docker cp' <<<"${plan_arguments}" >/dev/null; then
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

chmod 775 "${TRUSTED_ROOT}"
expect_failure writable_trusted_root 'ownership, mode, type, or link contract is unsafe' verify
chmod 755 "${TRUSTED_ROOT}"

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

sed -i 's/oci_index_size\t[0-9]*/oci_index_size\t999999999999999999999/' \
  "${PROFILE}"
write_policy
expect_failure numeric_bound 'malformed OCI index binding' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

printf '{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":["sha256:%s"]},"config":{"Env":[],"Volumes":{"/candidate":{}}}}\n' \
  "${layer_digest}" >"${TEST_ROOT}/config.adversarial.json"
install_image_config "${TEST_ROOT}/config.adversarial.json"
expect_failure image_volumes 'must not declare volumes' verify

printf '{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":["sha256:%s"]},"config":{"Env":[],"Healthcheck":{"Test":["CMD","true"]}}}\n' \
  "${layer_digest}" >"${TEST_ROOT}/config.adversarial.json"
install_image_config "${TEST_ROOT}/config.adversarial.json"
expect_failure image_healthcheck 'must not declare a healthcheck' verify

printf '{"architecture":"amd64","os":"linux","rootfs":[],"config":{"Env":[]}}\n' \
  >"${TEST_ROOT}/config.adversarial.json"
install_image_config "${TEST_ROOT}/config.adversarial.json"
expect_failure rootfs_type 'lacks a layer rootfs' verify

printf '{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":"sha256:%s"},"config":{"Env":[]}}\n' \
  "${layer_digest}" >"${TEST_ROOT}/config.adversarial.json"
install_image_config "${TEST_ROOT}/config.adversarial.json"
expect_failure rootfs_layers_type 'lacks a layer rootfs' verify

install_image_config "${TEST_ROOT}/config.base.json"
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

cp "${OCI_LAYOUT}/index.json" "${TEST_ROOT}/index.good"
python3 - "${OCI_LAYOUT}/index.json" <<'PY'
import json
from pathlib import Path
import sys

value = {"leaf": True}
for _ in range(40):
    value = {"nested": value}
Path(sys.argv[1]).write_text(json.dumps(value) + "\n", encoding="ascii")
PY
write_profile \
  "${manifest_digest}" "$(size "${TEST_ROOT}/manifest.json")" \
  "${config_digest}" "$(size "${TEST_ROOT}/config.json")" \
  "${layer_digest}" "$(size "${TEST_ROOT}/layer")"
expect_failure json_depth 'JSON exceeds structural limits' verify
cp "${TEST_ROOT}/index.good" "${OCI_LAYOUT}/index.json"
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

printf '{"schemaVersion":2,"schemaVersion":2,"manifests":[]}\n' \
  >"${OCI_LAYOUT}/index.json"
write_profile \
  "${manifest_digest}" "$(size "${TEST_ROOT}/manifest.json")" \
  "${config_digest}" "$(size "${TEST_ROOT}/config.json")" \
  "${layer_digest}" "$(size "${TEST_ROOT}/layer")"
expect_failure json_duplicate 'JSON contains a duplicate key' verify
cp "${TEST_ROOT}/index.good" "${OCI_LAYOUT}/index.json"
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i \
  's/environment\t0002\tHOSTNAME\t6665326f332d65766964656e6365/environment\t0002\tHOSTNAME\t77726f6e67/' \
  "${PROFILE}"
write_policy
expect_failure nondeterministic_hostname 'lacks the clean GPU baseline' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
write_policy

sed -i \
  's/environment\t0005\tROCR_VISIBLE_DEVICES\t36636564313634376132393635343563/environment\t0005\tROCR_VISIBLE_DEVICES\t30/' \
  "${PROFILE}"
write_policy
expect_failure gpu_visibility 'does not match protected GPU identity' verify
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
expect_failure profile_mutation 'profile size differs from its external binding' verify
cp "${TEST_ROOT}/profile.good" "${PROFILE}"
cp "${TEST_ROOT}/policy.good" "${POLICY}"

mv "${PROFILE}" "${TEST_ROOT}/profile.real"
ln -s "${TEST_ROOT}/profile.real" "${PROFILE}"
expect_failure profile_symlink 'cannot open protected OCI executor profile' verify
rm "${PROFILE}"
mv "${TEST_ROOT}/profile.real" "${PROFILE}"

chmod 664 "${PROFILE}"
expect_failure profile_writable 'OCI executor profile ownership, mode, type, or link contract is unsafe' verify
chmod 644 "${PROFILE}"

ln "${PROFILE}" "${TEST_ROOT}/profile.hardlink"
expect_failure profile_hardlink 'OCI executor profile ownership, mode, type, or link contract is unsafe' verify
rm "${TEST_ROOT}/profile.hardlink"

cp "${POLICY}" "${TEST_ROOT}/policy.anchor.good"
pinned_policy_size="$(size "${POLICY}")"
pinned_policy_digest="$(sha256 "${POLICY}")"
printf '# mutation\n' >>"${POLICY}"
expect_failure stale_external_policy_pin 'differs from its external binding' \
  "${EXECUTOR}" verify \
    --request "${REQUEST}" \
    --request-owner-uid "$(id -u)" \
    --request-owner-gid "$(id -g)" \
    --trusted-root "${TRUSTED_ROOT}" \
    --policy policy.tsv \
    --policy-size "${pinned_policy_size}" \
    --policy-sha256 "${pinned_policy_digest}" \
    --trusted-owner-uid "$(id -u)" \
    --trusted-owner-gid "$(id -g)" \
    --trust-file-contract descriptor-stable
mv "${TEST_ROOT}/policy.anchor.good" "${POLICY}"

chmod 664 "${POLICY}"
expect_failure policy_writable 'OCI executor policy ownership, mode, type, or link contract is unsafe' verify
chmod 644 "${POLICY}"

ln "${POLICY}" "${TEST_ROOT}/policy.hardlink"
expect_failure policy_hardlink 'OCI executor policy ownership, mode, type, or link contract is unsafe' verify
rm "${TEST_ROOT}/policy.hardlink"

mv "${POLICY}" "${TEST_ROOT}/policy.real"
ln -s "${TEST_ROOT}/policy.real" "${POLICY}"
expect_failure policy_symlink 'cannot open protected OCI executor policy' verify
rm "${POLICY}"
mv "${TEST_ROOT}/policy.real" "${POLICY}"

cp "${POLICY}" "${TEST_ROOT}/policy.test-domain"
sed -i 's/trust_domain\ttest/trust_domain\tproduction/' "${POLICY}"
expect_failure production_anchor_unavailable 'requires an external Linux immutable-file contract' verify
mv "${TEST_ROOT}/policy.test-domain" "${POLICY}"

ln -s "${REQUEST}" "${TEST_ROOT}/request.link"
expect_failure request_symlink 'cannot open OCI execution request without following links' \
  verify_request_path "${TEST_ROOT}/request.link"
rm "${TEST_ROOT}/request.link"

ln "${REQUEST}" "${TEST_ROOT}/request.hardlink"
expect_failure request_hardlink 'owner/mode/type/link/size contract' verify
rm "${TEST_ROOT}/request.hardlink"

chmod 666 "${REQUEST}"
expect_failure request_writable 'owner/mode/type/link/size contract' verify
chmod 644 "${REQUEST}"

wrong_request_uid="$(( $(id -u) + 1 ))"
expect_failure request_owner 'owner/mode/type/link/size contract' \
  verify_request_path "${REQUEST}" "${wrong_request_uid}" "$(id -g)"

cp "${REQUEST}" "${TEST_ROOT}/request.bounded"
truncate -s 1048577 "${REQUEST}"
expect_failure request_oversized 'owner/mode/type/link/size contract' verify
mv "${TEST_ROOT}/request.bounded" "${REQUEST}"

mkfifo "${TEST_ROOT}/request.fifo"
expect_failure request_fifo 'owner/mode/type/link/size contract' \
  verify_request_path "${TEST_ROOT}/request.fifo"
rm "${TEST_ROOT}/request.fifo"

PYTHONDONTWRITEBYTECODE=1 python3 - "${EXECUTOR}" "${REQUEST}" "${TRUSTED_ROOT}" <<'PY'
import importlib.util
import os
from pathlib import Path
import sys

module_path, request_text, trusted_text = sys.argv[1:]
spec = importlib.util.spec_from_file_location("parity_oci_executor_races", module_path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

request = Path(request_text)
real_read = module.read_descriptor_bound


def mutate_request(file_fd, maximum, label):
    raw = real_read(file_fd, maximum, label)
    os.utime(request, None)
    return raw


module.read_descriptor_bound = mutate_request
try:
    module.read_request_file(request, os.getuid(), os.getgid())
except module.ExecutorError as error:
    assert "changed while being read" in str(error)
else:
    raise AssertionError("request metadata race was accepted")
finally:
    module.read_descriptor_bound = real_read

trusted = Path(trusted_text)
policy = trusted / "policy.tsv"
anchor = module.TrustAnchor(
    policy.stat().st_size,
    module.sha256_file(policy),
    os.getuid(),
    os.getgid(),
    "descriptor-stable",
)
root = module.open_trusted_root(trusted, anchor)


def mutate_policy(file_fd, maximum, label):
    raw = real_read(file_fd, maximum, label)
    os.utime(policy, None)
    return raw


module.read_descriptor_bound = mutate_policy
try:
    module.read_trusted_file(
        root,
        "policy.tsv",
        anchor,
        expected_size=anchor.policy_size,
        expected_digest=anchor.policy_digest,
        label="race policy",
    )
except module.ExecutorError as error:
    assert "changed or differs" in str(error)
else:
    raise AssertionError("trusted policy metadata race was accepted")
finally:
    module.read_descriptor_bound = real_read
    root.close()
PY

# Runtime unavailability cannot produce even an ObservedRuntimeRequest. This
# command fails before host/image claims and cannot issue an execution receipt.
expect_failure operator_unavailable 'OCI runtime control plane unavailable' preflight

printf 'parity OCI executor authorization tests passed\n'
