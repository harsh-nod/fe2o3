#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SERVICE="$REPO_ROOT/deployment/systemd/fe2o3-proof-manager.service"

fail() {
  printf 'proof-manager systemd contract failed: %s\n' "$*" >&2
  exit 1
}

for expected in \
  'Type=exec' \
  'ExecStart=/usr/libexec/fe2o3/fe2o3-proof-manager' \
  'User=root' \
  'Group=root' \
  'RuntimeDirectory=fe2o3-proof-custodian' \
  'RuntimeDirectoryMode=0700' \
  'Delegate=yes' \
  'KillMode=control-group' \
  'Restart=no' \
  'NoNewPrivileges=no' \
  'AmbientCapabilities=' \
  'CapabilityBoundingSet=CAP_CHOWN CAP_DAC_OVERRIDE CAP_KILL CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE' \
  'UMask=0077' \
  'ReadWritePaths=/run/fe2o3-proof-custodian' \
  'InaccessiblePaths=-/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v1 -/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v1 -/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v3 -/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v3'; do
  grep -Fqx -- "$expected" "$SERVICE" || fail "missing $expected"
  key="${expected%%=*}"
  [[ $(grep -cE "^[[:space:]]*${key}[[:space:]]*=" "$SERVICE") -eq 1 ]] ||
    fail "duplicate or reset $key"
done

# The fixed controller must enter an unfiltered profile. Parent seccomp cannot
# be removed in the child, and coordinator loss must not stop offered custody.
if grep -Eq '^(SystemCall[^=]*|RestrictAddressFamilies|RestrictNamespaces|RestrictRealtime|RestrictSUIDSGID|LockPersonality|MemoryDenyWriteExecute|ProtectClock|ProtectHostname|ProtectKernel[^=]*|PrivateDevices|SecureBits|OpenFile|Sockets|EnvironmentFile)=' "$SERVICE"; then
  fail 'manager adds inherited confinement or external descriptor/environment inputs'
fi
if grep -Eq '^(Requires|Requisite|BindsTo|PartOf|StopPropagatedFrom)=' "$SERVICE"; then
  fail 'manager custody depends on another service lifetime'
fi
if grep -Eq '^(ExecStartPre|ExecStartPost|ExecStop|ExecStopPost)=' "$SERVICE"; then
  fail 'manager has an unreviewed lifecycle hook'
fi

printf 'proof-manager independent custody and unfiltered-parent unit contract is exact\n'
