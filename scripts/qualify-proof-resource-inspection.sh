#!/usr/bin/env bash
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C

readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly installed=/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5
[[ ${EUID} -eq 0 ]] || { printf 'qualification requires real root\n' >&2; exit 1; }

require_private() {
  [[ ${FE2O3_PROOF_INSTALL_PRIVATE:-} == 1 \
    && -n ${FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE:-} \
    && "$(readlink /proc/self/ns/pid)" != "$FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE" ]] || {
    printf 'setup phase requires the outer private namespace\n' >&2; exit 1;
  }
  for path in /etc /usr/libexec /opt /run /tmp; do
    [[ "$(stat -f -c %T "$path")" == tmpfs && "$(stat -c '%u:%g' "$path")" == 0:0 ]] || {
      printf 'setup path is not a private root tmpfs: %s\n' "$path" >&2; exit 1;
    }
  done
}

case "${1:-}" in
  '')
    for name in FE2O3_PROOF_INSTALL_TEST FE2O3_STATIC_PROOF_CUSTODIAN_DIR \
      FE2O3_PROOF_INSTALL_COORDINATOR FE2O3_PROOF_INSTALL_WORKER \
      FE2O3_PROOF_RUNTIME_INPUTS FE2O3_PROOF_VERUS_DIST FE2O3_PROOF_RUST_TOOLCHAIN; do
      [[ "${!name:-}" == /* && -e "${!name}" ]] || {
        printf 'missing absolute qualification input: %s\n' "${name}" >&2; exit 1;
      }
    done
    export FE2O3_PROOF_INSTALL_PRIVATE=1
    FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE="$(readlink /proc/self/ns/pid)"
    export FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE
    member=$(</proc/self/cgroup)
    [[ "$member" == 0::/* && "$member" != *$'\n'* ]]
    scope=$(mktemp -d "/sys/fs/cgroup/${member#0::/}/fe2o3-custodian-qual-XXXXXXXX")
    readonly scope
    cleanup() {
      local status=$?
      trap - EXIT
      printf '1\n' > "$scope/cgroup.kill"
      for attempt in {1..1000}; do
        if ! grep -q '^populated 1$' "$scope/cgroup.events"; then break; fi
        sleep 0.01
      done
      if grep -q '^populated 1$' "$scope/cgroup.events"; then
        printf 'owned qualification scope remains populated: %s\n' "$scope" >&2
        exit 99
      fi
      for child in "$scope"/fe2o3-proof-*; do
        [[ -d "$child" ]] || continue
        printf 'launcher left an empty owned scope: %s\n' "$child" >&2
        rmdir -- "$child"
        status=98
      done
      rmdir -- "$scope"
      printf 'outer cgroup removed: %s\n' "$scope"
      exit "$status"
    }
    trap cleanup EXIT
    trap 'exit 143' TERM
    trap 'exit 130' INT
    (
    printf '%s\n' "$BASHPID" > "$scope/cgroup.procs"
    exec timeout --kill-after=10s 600s bwrap --die-with-parent \
      --unshare-pid --unshare-ipc --unshare-uts --unshare-net \
      --ro-bind / / --tmpfs /etc --ro-bind /etc/alternatives /etc/alternatives \
      --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
      --tmpfs /usr/libexec --tmpfs /opt \
      --tmpfs /run --tmpfs /tmp --chmod 1777 /tmp --proc /proc --dev /dev \
      --bind /sys/fs/cgroup /sys/fs/cgroup \
      --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
      /bin/bash "$0" prepare
    ) &
    wait "$!"
    ;;
  prepare)
    require_private
    install -d -m 0755 /run/setup/overlay /run/setup/final-libs
    cd /run/setup
    printf '%s  %s\n' \
      3b8d5391b6b484a4c81fd000b6064885ad967ec3cb966bc57603f3fb3ebf0ed5 "$FE2O3_PROOF_RUNTIME_INPUTS/libc.deb" \
      7074b6a2f6367a10d280c00a1cb02e74277709180bab4f2491a2f355ab2d6c20 "$FE2O3_PROOF_RUNTIME_INPUTS/zlib.deb" \
      | sha256sum --check --strict
    dpkg-deb -x "$FE2O3_PROOF_RUNTIME_INPUTS/libc.deb" glibc
    dpkg-deb -x "$FE2O3_PROOF_RUNTIME_INPUTS/zlib.deb" zlib
    # Setup tools may use host DSOs; none of these aliases survive the final namespace.
    for source in /usr/lib/x86_64-linux-gnu/*; do
      ln -s "/run/host-lib/${source##*/}" "/run/setup/overlay/${source##*/}"
    done
    for name in ld-linux-x86-64.so.2 libc.so.6 libdl.so.2 libm.so.6 libpthread.so.0 librt.so.1; do
      cp --remove-destination "glibc/usr/lib/x86_64-linux-gnu/$name" "overlay/$name"
    done
    cp --remove-destination zlib/usr/lib/x86_64-linux-gnu/libz.so.1.3 overlay/libz.so.1.3
    for name in libgcc_s.so.1 libstdc++.so.6.0.33 libzstd.so.1.5.5 libcap-ng.so.0.0.0; do
      cp --remove-destination "/usr/lib/x86_64-linux-gnu/$name" "overlay/$name"
    done
    for name in ld-linux-x86-64.so.2 libc.so.6 libdl.so.2 libm.so.6 libpthread.so.0 librt.so.1 \
      libz.so.1.3 libgcc_s.so.1 libstdc++.so.6.0.33 libzstd.so.1.5.5 libcap-ng.so.0.0.0; do
      install -m 0555 "overlay/$name" "final-libs/$name"
    done
    chmod 0755 final-libs/ld-linux-x86-64.so.2
    for dir in overlay final-libs; do
      ln -sf libz.so.1.3 "$dir/libz.so.1"
      ln -sf libstdc++.so.6.0.33 "$dir/libstdc++.so.6"
      ln -sf libzstd.so.1.5.5 "$dir/libzstd.so.1"
      ln -sf libcap-ng.so.0.0.0 "$dir/libcap-ng.so.0"
    done
    exec bwrap --die-with-parent --bind / / --dev /dev \
      --ro-bind /usr/lib/x86_64-linux-gnu /run/host-lib \
      --ro-bind /run/setup/overlay /usr/lib/x86_64-linux-gnu \
      --tmpfs /usr/lib64 --symlink ../lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 /usr/lib64/ld-linux-x86-64.so.2 \
      --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
      /bin/bash "$0" provision
    ;;
  provision)
    require_private
    /bin/bash "$repo/scripts/functional-refinement-verus-runtime-v1.sh" provision \
      "$FE2O3_PROOF_VERUS_DIST" "$FE2O3_PROOF_RUST_TOOLCHAIN" \
      "$FE2O3_PROOF_RUNTIME_INPUTS/rustup-init" "$installed"
    install -d -m 0755 /usr/libexec/fe2o3
    for name in fe2o3-proof-manager fe2o3-application-proof-controller fe2o3-proof-custodian-provision; do
      install -m 0555 "$FE2O3_STATIC_PROOF_CUSTODIAN_DIR/$name" "/usr/libexec/fe2o3/$name"
    done
    install -m 0555 "$FE2O3_PROOF_INSTALL_COORDINATOR" /usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator
    install -m 0555 "$FE2O3_PROOF_INSTALL_WORKER" /usr/libexec/fe2o3/fe2o3-llvm-link-worker
    install -m 0555 "$FE2O3_PROOF_INSTALL_TEST" /usr/libexec/fe2o3/resource-qualification
    install -d -m 0755 /run/proof-inputs
    for name in envelope.bin payload.hsaco kernel.bin; do
      tar -xOf "$repo/docs/evidence/dev-application-proof-controller-2026-10-04/evidence.tar.gz" \
        "captures/inputs-2/$name" > "/run/proof-inputs/$name"
      chmod 0444 "/run/proof-inputs/$name"
    done
    printf '%s  %s\n' \
      46fe74c62eaf9ebd99158dadc8d8d2e1f8f37701d2731aeab48531dd00e9e90e /run/proof-inputs/envelope.bin \
      8b6c2e5b67ba2f76bb57d5f42aedbaf968f8dc12bb121daa5d7853cc55d158c6 /run/proof-inputs/payload.hsaco \
      9f75a0b7887881b7a13d2311965f20631565d60ef29375affc65cd2e2bead731 /run/proof-inputs/kernel.bin \
      | sha256sum --check --strict
    # No setup tools, user homes, loader cache/preload, or source aliases are visible here.
    exec bwrap --die-with-parent --bind / / --dev /dev \
      --bind /run/setup/final-libs /usr/lib/x86_64-linux-gnu \
      --tmpfs /run/setup --tmpfs /run/host-lib --tmpfs /home --tmpfs /root --tmpfs /etc \
      --tmpfs /tmp --chmod 1777 /tmp --chdir / --clearenv \
      --setenv FE2O3_PROOF_INSTALL_PRIVATE 1 \
      --setenv FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE "$FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE" \
      --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
      /usr/libexec/fe2o3/resource-qualification --exact provisioning::tests::root_fixed_resource_inspection_campaign \
      --ignored --nocapture --test-threads=1
    ;;
  *) exit 2 ;;
esac
