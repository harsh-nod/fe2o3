#!/usr/bin/env bash
set -euo pipefail

[[ ${EUID} -eq 0 ]] || { printf 'private proof installation qualification requires real root\n' >&2; exit 1; }
for name in FE2O3_PROOF_INSTALL_TEST FE2O3_STATIC_PROOF_CUSTODIAN_DIR \
  FE2O3_PROOF_INSTALL_COORDINATOR FE2O3_PROOF_INSTALL_WORKER; do
  [[ -n "${!name:-}" && "${!name}" == /* && -e "${!name}" ]] || {
    printf 'missing absolute qualification input: %s\n' "${name}" >&2; exit 1;
  }
done
export FE2O3_PROOF_INSTALL_PRIVATE=1
FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE="$(readlink /proc/self/ns/pid)"
export FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE

# Only the private namespace receives writable installed paths. No host unit is started.
exec /usr/bin/timeout --kill-after=10s 180s /usr/bin/bwrap --die-with-parent \
  --unshare-pid --unshare-ipc --unshare-uts --unshare-net \
  --ro-bind / / --tmpfs /etc --tmpfs /usr/libexec \
  --tmpfs /run --tmpfs /tmp --chmod 1777 /tmp --proc /proc --dev /dev \
  --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
  "${FE2O3_PROOF_INSTALL_TEST}" --exact provisioning::tests::root_fixed_install_campaign \
  --ignored --nocapture --test-threads=1
