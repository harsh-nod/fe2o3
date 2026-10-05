#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SERVICE="${REPO_ROOT}/deployment/systemd/fe2o3-compiler-execution.service"
readonly V1_SERVICE="${REPO_ROOT}/deployment/systemd/fe2o3-compiler-execution-v1.service"
readonly LEGACY_SOCKET="${REPO_ROOT}/deployment/systemd/fe2o3-compiler-execution.socket"
readonly SYSUSERS="${REPO_ROOT}/deployment/sysusers.d/fe2o3-compiler-execution.conf"
readonly TMPFILES="${REPO_ROOT}/deployment/tmpfiles.d/fe2o3-compiler-execution.conf"
readonly ENTRYPOINT="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/native_entrypoint.rs"
readonly INHERITED="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/native_inherited_adapter.rs"
readonly PROVISIONER="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/native_provisioner.rs"
readonly ACTIVATION_TESTS="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/native_activation_tests.rs"
readonly COORDINATOR_LIFECYCLE="${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/lifecycle.rs"
readonly SERVICE_LIFECYCLE="${REPO_ROOT}/crates/fe2o3-compiler-execution-lifecycle/src/lib.rs"
readonly SUPERVISOR_DEPLOYMENT="${REPO_ROOT}/crates/fe2o3-compiler-execution-supervisor/src/deployment.rs"
readonly SUPERVISOR_LISTENER="${REPO_ROOT}/crates/fe2o3-compiler-execution-supervisor/src/listener.rs"
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

[[ ! -e "${LEGACY_SOCKET}" ]] || fail 'legacy root-listening socket unit still exists'

mapfile -t open_files < <(sed -n 's/^OpenFile=//p' "${SERVICE}")
readonly expected_open_files=(
  '/run/fe2o3:runtime-root:read-only'
  '/var/lib/fe2o3/compiler-execution:supervisor-root:read-only'
  '/var/lib/fe2o3/external-anchor:anchor-root:read-only'
  '/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3:supervisor:read-only'
  '/usr/libexec/fe2o3/fe2o3-static-preexec-launcher:launcher:read-only'
  '/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional:issuer:read-only'
  '/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3:anchor-helper:read-only'
  '/usr/libexec/fe2o3/fe2o3-external-anchor-service-v3:anchor-daemon:read-only'
  '/etc/fe2o3/compiler-execution/supervisor-deployment-v3:supervisor-deployment:read-only'
  '/etc/fe2o3/compiler-execution/issuer-policy-v3:issuer-policy:read-only'
  '/etc/fe2o3/compiler-execution/anchor-deployment-v3:anchor-deployment:read-only'
  '/etc/fe2o3/compiler-execution/anchor-provisioning-v3:anchor-provisioning:read-only'
  '/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v3:issuer-key-seed:read-only'
  '/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v3:anchor-key-seed:read-only'
)
[[ "${#open_files[@]}" -eq "${#expected_open_files[@]}" ]] || fail 'OpenFile count is not 14'
for index in "${!expected_open_files[@]}"; do
  [[ "${open_files[index]}" == "${expected_open_files[index]}" ]] ||
    fail "OpenFile ${index} changed"
done

activation_names=''
for open_file in "${open_files[@]}"; do
  without_options="${open_file%:read-only}"
  [[ -z "${activation_names}" ]] || activation_names+=':'
  activation_names+="${without_options##*:}"
done
grep -Fq -- "${activation_names}" "${ACTIVATION_TESTS}" || fail 'native activation role test changed'
grep -Fq -- 'root::listener(runtime_root.into(), credentials.gid(), b)' "${INHERITED}" ||
  fail 'coordinator bound-listener construction is missing'
if sed '/^#\[cfg(test)\]/,$d' "${INHERITED}" | grep -Fq -- 'listen('; then
  fail 'root coordinator must not activate the production listener'
fi
grep -Fq -- 'listen(&self.socket.descriptor, LISTENER_BACKLOG_V1)' "${SUPERVISOR_LISTENER}" ||
  fail 'protected supervisor listener activation is missing'
launch_line="$(grep -n -m1 -F -- 'runtime.start(b)?;' "${ENTRYPOINT}" | cut -d: -f1)"
ready_line="$(grep -n -m1 -F -- 'runtime.publish(b)?;' "${ENTRYPOINT}" | cut -d: -f1)"
[[ -n "${launch_line}" && -n "${ready_line}" && "${launch_line}" -lt "${ready_line}" ]] ||
  fail 'systemd readiness must follow supervisor bootstrap readiness'
for path in \
  /usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3 \
  /usr/libexec/fe2o3/fe2o3-static-preexec-launcher \
  /usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional \
  /usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3 \
  /usr/libexec/fe2o3/fe2o3-external-anchor-service-v3; do
  grep -Fq -- "\"${path}\"" "${PROVISIONER}" || fail "provisioner path ${path} changed"
done
for name in \
  supervisor-deployment-v3 \
  issuer-policy-v3 \
  anchor-deployment-v3 \
  anchor-provisioning-v3 \
  issuer-signing-key-seed-v3 \
  anchor-signing-key-seed-v3; do
  grep -Fq -- "\"${name}\"" "${PROVISIONER}" || fail "provisioner file ${name} changed"
  grep -Fq -- "/etc/fe2o3/compiler-execution/${name}:" "${SERVICE}" ||
    fail "service file ${name} changed"
done
grep -Fq -- '"client-profile-v3"' "${PROVISIONER}" ||
  fail 'provisioner client profile is missing'
grep -RFq -- '"/etc/fe2o3/compiler-execution/client-profile-v3"' "$(dirname "${PROTOCOL}")" ||
  fail 'canonical client-profile path is missing'
if grep -Fq -- '/etc/fe2o3/compiler-execution/client-profile-v3:' "${SERVICE}"; then
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
grep -Fq -- 'Lease::open(b)?' "${PROVISIONER}" ||
  fail 'provisioner lifecycle lease is missing'
lifecycle_lease_line="$(grep -n -m1 -F -- 'let (lifecycle, c) = Lease::open(&supervisor_root, b)?;' "${INHERITED}" | cut -d: -f1)"
supervisor_lifecycle_line="$(grep -n -m1 -F -- 'let (supervisor_lifecycle, c) = Lease::open(&supervisor_root, b)?;' "${INHERITED}" | cut -d: -f1)"
anchor_lifecycle_line="$(grep -n -m1 -F -- 'let (anchor_lifecycle, c) = Lease::open(&anchor_root, b)?;' "${INHERITED}" | cut -d: -f1)"
issuer_seed_line="$(grep -n -m1 -F -- 'source::read_seed(&issuer_seed, b)?;' "${INHERITED}" | cut -d: -f1)"
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
require_line "${SERVICE}" 'Type=notify'
require_line "${SERVICE}" 'NotifyAccess=main'
require_line "${SERVICE}" 'RuntimeDirectory=fe2o3'
require_line "${SERVICE}" 'RuntimeDirectoryMode=0755'
if grep -Eq '^(Requires|After|Sockets)=.*fe2o3-compiler-execution\.socket' "${SERVICE}"; then
  fail 'service retains a legacy socket-activation dependency'
fi
require_line "${SERVICE}" 'StartLimitIntervalSec=0'
require_line "${SERVICE}" 'KillMode=mixed'
require_line "${SERVICE}" 'Restart=on-failure'
require_line "${SERVICE}" 'RestartSec=1'
require_line "${SERVICE}" 'RestrictAddressFamilies=AF_UNIX'
# Source contract only: no effective drop-in, kernel, LSM or service admission.
# Reject duplicate/reset directives as well as missing exact assignments.
paired_sandbox_contract() {
  awk -v version="${1:-v3}" '
    BEGIN {
      expected["User"] = "root"
      expected["Group"] = "root"
      expected["CapabilityBoundingSet"] = "CAP_CHOWN CAP_DAC_READ_SEARCH CAP_KILL CAP_SETFCAP CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE"
      expected["AmbientCapabilities"] = ""
      expected["NoNewPrivileges"] = "no"
      expected["PrivateDevices"] = "yes"
      expected["PrivateTmp"] = "yes"
      expected["PrivateUsers"] = "no"
      expected["ProtectClock"] = "yes"
      expected["ProtectControlGroups"] = "yes"
      expected["ProtectHome"] = "yes"
      expected["ProtectHostname"] = "yes"
      expected["ProtectKernelLogs"] = "yes"
      expected["ProtectKernelModules"] = "yes"
      expected["ProtectKernelTunables"] = "yes"
      expected["ProtectSystem"] = "strict"
      expected["Slice"] = "system.slice"
      expected["Delegate"] = ""
      expected["ReadWritePaths"] = "/var/lib/fe2o3/compiler-execution /var/lib/fe2o3/external-anchor /sys/fs/cgroup/system.slice/%n"
      expected["RestrictAddressFamilies"] = "AF_UNIX"
      expected["RestrictNamespaces"] = "no"
      expected["SystemCallFilter"] = "~unshare:EPERM setns:EPERM"
      expected["RestrictRealtime"] = "yes"
      expected["RestrictSUIDSGID"] = "yes"
      expected["LockPersonality"] = "yes"
      expected["SystemCallArchitectures"] = "native"
      expected["UMask"] = "0077"
      expected["LimitCORE"] = "0"
      expected["TasksMax"] = "256"
      expected["KillMode"] = "mixed"
      expected["TimeoutStartSec"] = "300"
      expected["TimeoutStopSec"] = "30"
      if (version == "v1") {
        expected["CapabilityBoundingSet"] = "CAP_CHOWN CAP_KILL CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE"
        expected["ReadWritePaths"] = "/var/lib/fe2o3/compiler-execution /var/lib/fe2o3/external-anchor"
        expected["RestrictNamespaces"] = "yes"
        delete expected["Slice"]
        delete expected["Delegate"]
        delete expected["SystemCallFilter"]
      }
    }
    /^[[:space:]]*[#;]/ { next }
    /^[[:space:]]*\[/ { section = $0; next }
    /^[[:space:]]*[A-Za-z]+[[:space:]]*=/ {
      equal = index($0, "=")
      key = substr($0, 1, equal - 1)
      gsub(/[[:space:]]/, "", key)
      value = substr($0, equal + 1)
      if (key in expected) {
        seen[key]++
        if (section != "[Service]" || value != expected[key]) bad = 1
      }
      if (key ~ /^(DelegateSubgroup|BindPaths|BindReadOnlyPaths|RootDirectory|RootImage|TemporaryFileSystem|MountImages|ExtensionImages|ExtensionDirectories)$/) bad = 1
      if (version == "v1" && key ~ /^(Slice|Delegate|SystemCallFilter|ProtectProc|ProcSubset)$/) bad = 1
    }
    END {
      for (key in expected) if (seen[key] != 1) bad = 1
      exit bad
    }
  '
}

paired_sandbox_contract < "${SERVICE}" || fail 'paired creator/subtree sandbox changed'
paired_sandbox_contract v1 < "${V1_SERVICE}" || fail 'V1 root-only namespace custody sandbox changed'
for replacement in \
  'CapabilityBoundingSet=CAP_CHOWN CAP_KILL CAP_SETGID CAP_SETPCAP CAP_SETUID' \
  'CapabilityBoundingSet=CAP_CHOWN CAP_KILL CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE CAP_SYS_ADMIN' \
  'AmbientCapabilities=CAP_SYS_PTRACE' \
  'RestrictNamespaces=no'; do
  key="${replacement%%=*}"
  if awk -v key="${key}" -v replacement="${replacement}" \
      'index($0, key "=") == 1 { print replacement; next } { print }' "${V1_SERVICE}" |
      paired_sandbox_contract v1; then
    fail "V1 sandbox oracle accepted ${replacement}"
  fi
done
if awk '{ print } /^CapabilityBoundingSet=/ { print "CapabilityBoundingSet=CAP_SYS_ADMIN" }' "${V1_SERVICE}" |
    paired_sandbox_contract v1; then
  fail 'V1 sandbox oracle accepted an additional capability assignment'
fi
for replacement in \
  'ReadWritePaths=/sys/fs/cgroup' \
  'ReadWritePaths=/sys/fs/cgroup/system.slice' \
  'ReadWritePaths=/sys/fs/cgroup/system.slice/other.service' \
  'RestrictNamespaces=yes' \
  'RestrictNamespaces=user' \
  'Delegate=yes' \
  'SystemCallFilter=' \
  'SystemCallFilter=~unshare:EPERM setns:EPERM clone3:ENOSYS' \
  'CapabilityBoundingSet=CAP_CHOWN CAP_DAC_READ_SEARCH CAP_KILL CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE' \
  'CapabilityBoundingSet=CAP_SYS_ADMIN'; do
  key="${replacement%%=*}"
  if awk -v key="${key}" -v replacement="${replacement}" \
      'index($0, key "=") == 1 { print replacement; next } { print }' "${SERVICE}" |
      paired_sandbox_contract; then
    fail "sandbox oracle accepted ${replacement}"
  fi
done
if awk '{ print } /^ReadWritePaths=/ { print "ReadWritePaths=/sys/fs/cgroup" }' "${SERVICE}" |
    paired_sandbox_contract; then
  fail 'sandbox oracle accepted an additional broad writable subtree'
fi
readonly SPAWN_SYSCALL="${REPO_ROOT}/crates/fe2o3-protected-service-spawn/src/syscall.rs"
grep -Fq -- 'self.compiler.is_some() || mapping_gate.is_some()' "${SPAWN_SYSCALL}" ||
  fail 'mandatory compiler/mapped-helper namespace confinement selection changed'
grep -Fq -- 'staged.requires_namespace_confinement(mapping_gate)' "${SPAWN_SYSCALL}" ||
  fail 'child namespace confinement call is missing'
grep -Fq -- '!namespace_restrictions::install()' "${SPAWN_SYSCALL}" ||
  fail 'child namespace installation no longer refuses on failure'
grep -Fq -- 'native V3 indirect launch requires the original-root FD12 route' \
  "${REPO_ROOT}/crates/fe2o3-compiler-execution-coordinator/src/native_launch_adapter.rs" ||
  fail 'missing original-root launch guard was silently removed'

require_line "${SYSUSERS}" 'u fe2o3-compiler - "fe2o3 compiler-execution supervisor" /var/lib/fe2o3/compiler-execution -'
require_line "${SYSUSERS}" 'u fe2o3-anchor - "fe2o3 external monotonic anchor" /var/lib/fe2o3/external-anchor -'
require_line "${TMPFILES}" 'd /run/fe2o3 0755 root root -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3 0755 root root -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3/compiler-execution 0700 fe2o3-compiler fe2o3-compiler -'
require_line "${TMPFILES}" 'd /var/lib/fe2o3/external-anchor 0700 fe2o3-anchor fe2o3-anchor -'
require_line "${TMPFILES}" 'f /var/lib/fe2o3/compiler-execution-lifecycle-v1 0400 root root -'

printf 'native V3 activation source contract is exact; startup remains guarded\n'
