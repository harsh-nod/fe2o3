#!/usr/bin/env bash
set -euo pipefail

# Prebuilt component qualification with test keys, not measured deployment or GPU authority.
if [[ ${EUID} -ne 0 ]]; then
  printf 'This isolated namespace controller requires root. Build all fixtures beforehand.\n' >&2
  exit 2
fi
for name in FE2O3_COMPOSED_COORDINATOR_TEST FE2O3_COMPOSED_CARGO_TEST \
    FE2O3_COMPOSED_STATIC_CONSUMER FE2O3_TEST_HSACO_WORKER_FIXTURE \
    FE2O3_STATIC_PREEXEC_LAUNCHER FE2O3_STATIC_COMPILER_EXECUTION_ISSUER \
    FE2O3_ROOT_ANCHOR_HELPER FE2O3_ROOT_ANCHOR_DAEMON; do
  value=${!name:?absolute prebuilt executable is required}
  if [[ $value != /* || ! -f $value || ! -x $value ]]; then
    printf 'Invalid executable: %s\n' "$name" >&2
    exit 2
  fi
done
: "${FE2O3_COMPOSED_RUST_LIBRARY_DIR:?absolute Rust runtime library directory is required}"
[[ $FE2O3_COMPOSED_RUST_LIBRARY_DIR == /* && -d $FE2O3_COMPOSED_RUST_LIBRARY_DIR ]]
deps=$(dirname -- "$FE2O3_COMPOSED_COORDINATOR_TEST")
coordinator=$(basename -- "$FE2O3_COMPOSED_COORDINATOR_TEST")

# Keep this controller alive: the child validates these original namespace objects.
# bwrap's private PID1 also contains descendants on harness panic or controller timeout.
/usr/bin/bwrap --die-with-parent --unshare-pid --unshare-ipc --unshare-uts --unshare-net \
  --ro-bind / / --tmpfs /tmp --chmod 1777 /tmp \
  --tmpfs /etc --tmpfs /run --tmpfs /var/lib \
  --ro-bind "/proc/$$/ns" /run/fe2o3-test-ns \
  --tmpfs /opt --ro-bind "$deps" /opt/test-deps \
  --ro-bind "$FE2O3_COMPOSED_RUST_LIBRARY_DIR" /opt/toolchain-lib \
  --proc /proc --dev /dev \
  --cap-drop ALL --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP \
  --cap-add CAP_CHOWN --cap-add CAP_FOWNER --cap-add CAP_DAC_OVERRIDE \
  --cap-add CAP_SYS_PTRACE --cap-add CAP_KILL \
  /usr/bin/env -i PATH=/usr/bin:/bin TMPDIR=/tmp \
  LD_LIBRARY_PATH=/opt/test-deps:/opt/toolchain-lib \
  FE2O3_COMPOSED_NAMESPACE_V1=1 \
  FE2O3_COMPOSED_CARGO_TEST="$FE2O3_COMPOSED_CARGO_TEST" \
  FE2O3_COMPOSED_STATIC_CONSUMER="$FE2O3_COMPOSED_STATIC_CONSUMER" \
  FE2O3_TEST_HSACO_WORKER_FIXTURE="$FE2O3_TEST_HSACO_WORKER_FIXTURE" \
  FE2O3_STATIC_PREEXEC_LAUNCHER="$FE2O3_STATIC_PREEXEC_LAUNCHER" \
  FE2O3_STATIC_COMPILER_EXECUTION_ISSUER="$FE2O3_STATIC_COMPILER_EXECUTION_ISSUER" \
  FE2O3_ROOT_ANCHOR_HELPER="$FE2O3_ROOT_ANCHOR_HELPER" \
  FE2O3_ROOT_ANCHOR_DAEMON="$FE2O3_ROOT_ANCHOR_DAEMON" \
  "/opt/test-deps/$coordinator" \
  --exact composed_startup::root_composed_production_application_startup \
  --ignored --nocapture --test-threads=1
printf 'composed root namespace exited successfully\n'
