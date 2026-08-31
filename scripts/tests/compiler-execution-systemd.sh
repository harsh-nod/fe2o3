#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SERVICE="${REPO_ROOT}/deployment/systemd/fe2o3-compiler-execution.service"
readonly SOCKET="${REPO_ROOT}/deployment/systemd/fe2o3-compiler-execution.socket"
readonly SYSUSERS="${REPO_ROOT}/deployment/sysusers.d/fe2o3-compiler-execution.conf"
readonly TMPFILES="${REPO_ROOT}/deployment/tmpfiles.d/fe2o3-compiler-execution.conf"
readonly ENTRYPOINT="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/entrypoint.rs"
readonly INHERITED="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/inherited.rs"
readonly PROVISIONER="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/provisioning_entrypoint.rs"
readonly COORDINATOR_LIFECYCLE="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/lifecycle.rs"
readonly SERVICE_LIFECYCLE="${REPO_ROOT}/crates/fe2o3-compiler-execution-lifecycle/src/lib.rs"
readonly SUPERVISOR_DEPLOYMENT="${REPO_ROOT}/crates/fe2o3-compiler-execution-supervisor/src/deployment.rs"
readonly ANCHOR_HELPER="${REPO_ROOT}/crates/fe2o3-external-anchor-provisioner/src/entrypoint.rs"
readonly ANCHOR_SERVICE="${REPO_ROOT}/crates/fe2o3-external-anchor-service/src/entrypoint.rs"
readonly PROTOCOL="${REPO_ROOT}/crates/fe2o3-compiler-execution-protocol/src/lib.rs"
readonly COORDINATOR_MANIFEST="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/Cargo.toml"

fail() {
  printf 'compiler-execution systemd contract failed: %s\n' "$*" >&2
  exit 1
}

require_line() {
  local file="$1"
  local expected="$2"
  grep -Fqx -- "${expected}" "${file}" || fail "missing ${expected} in ${file}"
}

require_line "${SOCKET}" 'ListenSequentialPacket=/run/fe2o3/compiler-execution-supervisor.sock'
require_line "${SOCKET}" 'FileDescriptorName=compiler-execution-listener'
require_line "${SOCKET}" 'SocketUser=root'
require_line "${SOCKET}" 'SocketGroup=fe2o3-compiler'
require_line "${SOCKET}" 'SocketMode=0660'
require_line "${SOCKET}" 'DirectoryMode=0755'
require_line "${SOCKET}" 'FlushPending=no'

mapfile -t open_files < <(sed -n 's/^OpenFile=//p' "${SERVICE}")
readonly expected_open_files=(
  '/var/lib/fe2o3/compiler-execution:supervisor-root:read-only'
  '/var/lib/fe2o3/external-anchor:anchor-root:read-only'
  '/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor:supervisor:read-only'
  '/usr/libexec/fe2o3/fe2o3-static-preexec-launcher:launcher:read-only'
  '/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer:issuer:read-only'
  '/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper:anchor-helper:read-only'
  '/usr/libexec/fe2o3/fe2o3-external-anchor-service:anchor-daemon:read-only'
  '/etc/fe2o3/compiler-execution/supervisor-deployment-v1:supervisor-deployment:read-only'
  '/etc/fe2o3/compiler-execution/issuer-policy-v1:issuer-policy:read-only'
  '/etc/fe2o3/compiler-execution/anchor-deployment-v1:anchor-deployment:read-only'
  '/etc/fe2o3/compiler-execution/anchor-provisioning-v1:anchor-provisioning:read-only'
  '/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v1:issuer-key-seed:read-only'
  '/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v1:anchor-key-seed:read-only'
)
[[ "${#open_files[@]}" -eq "${#expected_open_files[@]}" ]] || fail 'OpenFile count is not 13'
for index in "${!expected_open_files[@]}"; do
  [[ "${open_files[index]}" == "${expected_open_files[index]}" ]] ||
    fail "OpenFile ${index} changed"
done

activation_names='compiler-execution-listener'
for open_file in "${open_files[@]}"; do
  without_options="${open_file%:read-only}"
  activation_names+=":${without_options##*:}"
done
grep -Fq -- "${activation_names}" "${ENTRYPOINT}" || fail 'entrypoint activation names changed'
for path in \
  /usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor \
  /usr/libexec/fe2o3/fe2o3-static-preexec-launcher \
  /usr/libexec/fe2o3/fe2o3-compiler-execution-issuer \
  /usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper \
  /usr/libexec/fe2o3/fe2o3-external-anchor-service; do
  grep -Fq -- "\"${path}\"" "${PROVISIONER}" || fail "provisioner path ${path} changed"
done
for name in \
  supervisor-deployment-v1 \
  issuer-policy-v1 \
  anchor-deployment-v1 \
  anchor-provisioning-v1 \
  issuer-signing-key-seed-v1 \
  anchor-signing-key-seed-v1; do
  grep -Fq -- "\"${name}\"" "${PROVISIONER}" || fail "provisioner file ${name} changed"
  grep -Fq -- "/etc/fe2o3/compiler-execution/${name}:" "${SERVICE}" ||
    fail "service file ${name} changed"
done
grep -Fq -- '"client-profile-v1"' "${PROVISIONER}" ||
  fail 'provisioner client profile is missing'
grep -Fq -- '"/etc/fe2o3/compiler-execution/client-profile-v1"' "${PROTOCOL}" ||
  fail 'canonical client-profile path is missing'
if grep -Fq -- '/etc/fe2o3/compiler-execution/client-profile-v1:' "${SERVICE}"; then
  fail 'public client profile must not add a coordinator activation descriptor'
fi
grep -Fq -- 'name = "fe2o3-compiler-execution-provision"' "${COORDINATOR_MANIFEST}" ||
  fail 'provisioner binary target is missing'
grep -Fq -- '"/var/lib/fe2o3/compiler-execution"' "${PROTOCOL}" ||
  fail 'canonical supervisor state-root path is missing'
grep -Fq -- '"/var/lib/fe2o3/compiler-execution-lifecycle-v1"' "${PROTOCOL}" ||
  fail 'canonical lifecycle-lock path is missing'
if grep -Fq -- '/var/lib/fe2o3/compiler-execution-lifecycle-v1:' "${SERVICE}"; then
  fail 'lifecycle lock must derive from supervisor-root instead of adding an activation descriptor'
fi
grep -Fq -- 'RetainedProvisioningLifecycleLeaseV1::admit(' "${PROVISIONER}" ||
  fail 'provisioner lifecycle lease is missing'
lifecycle_lease_line="$(grep -n -m1 -F -- 'CompilerExecutionLifecycleLeaseV1::admit_service_from_root(&supervisor_root)' "${INHERITED}" | cut -d: -f1)"
supervisor_lifecycle_line="$(grep -n -m1 -F -- 'CompilerExecutionServiceLifecycleLeaseV1::open(&supervisor_root)' "${INHERITED}" | cut -d: -f1)"
anchor_lifecycle_line="$(grep -n -m1 -F -- 'CompilerExecutionServiceLifecycleLeaseV1::open(&anchor_root)' "${INHERITED}" | cut -d: -f1)"
issuer_seed_line="$(grep -n -m1 -F -- 'let mut seed = read_seed(File::from(issuer_key_seed)' "${INHERITED}" | cut -d: -f1)"
[[ -n "${lifecycle_lease_line}" && -n "${issuer_seed_line}" && "${lifecycle_lease_line}" -lt "${issuer_seed_line}" ]] ||
  fail 'service lifecycle lease must precede issuer key admission'
[[ -n "${supervisor_lifecycle_line}" && -n "${anchor_lifecycle_line}" &&
  "${supervisor_lifecycle_line}" -lt "${issuer_seed_line}" &&
  "${anchor_lifecycle_line}" -lt "${issuer_seed_line}" ]] ||
  fail 'independent child lifecycle leases must precede issuer key admission'
grep -Fq -- 'COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1: RawFd = 12' "${SUPERVISOR_DEPLOYMENT}" ||
  fail 'supervisor lifecycle descriptor contract changed'
grep -Fq -- 'EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1: RawFd = 6' "${ANCHOR_HELPER}" ||
  fail 'anchor-helper lifecycle descriptor contract changed'
grep -Fq -- 'EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1: RawFd = 5' "${ANCHOR_SERVICE}" ||
  fail 'anchor-service lifecycle descriptor contract changed'
grep -Fq -- 'PRIVATE_LIFECYCLE_PARENT_FD_V1: RawFd = 259' "${ANCHOR_SERVICE}" ||
  fail 'anchor-service private lifecycle-parent descriptor changed'
if sed '/^#\[cfg(test)\]/,$d' "${COORDINATOR_LIFECYCLE}" "${SERVICE_LIFECYCLE}" |
  grep -Fq -- 'FlockOperation::Unlock'; then
  fail 'production lifecycle custody must release only by last close'
fi
require_line "${SERVICE}" 'Sockets=fe2o3-compiler-execution.socket'
require_line "${SERVICE}" 'StartLimitIntervalSec=0'
require_line "${SERVICE}" 'KillMode=mixed'
require_line "${SERVICE}" 'Restart=on-failure'
require_line "${SERVICE}" 'RestartSec=1'
require_line "${SERVICE}" 'RestrictAddressFamilies=AF_UNIX'

require_line "${SYSUSERS}" 'u fe2o3-compiler - "fe2o3 compiler-execution supervisor" /var/lib/fe2o3/compiler-execution -'
require_line "${SYSUSERS}" 'u fe2o3-anchor - "fe2o3 external monotonic anchor" /var/lib/fe2o3/external-anchor -'
require_line "${TMPFILES}" 'd /run/fe2o3 0755 root root -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3 0755 root root -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3/compiler-execution 0700 fe2o3-compiler fe2o3-compiler -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3/external-anchor 0700 fe2o3-anchor fe2o3-anchor -'
require_line "${TMPFILES}" 'f /var/lib/fe2o3/compiler-execution-lifecycle-v1 0400 root root -'

printf 'compiler-execution systemd descriptor and filesystem policy is exact\n'
