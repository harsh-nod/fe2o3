#!/usr/bin/env bash
# Test-only fixed-path admission inside a private mount namespace. No host install.
set -Eeuo pipefail

case ${1:-} in
  run)
    test "$EUID" -eq 0
    test "$#" -eq 3
    binary=$(realpath -e -- "$2")
    libraries=$(realpath -e -- "$3")
    test -f "$binary"
    test -x "$binary"
    test -d "$libraries"
    case "$binary:$libraries" in /run/*|*:/run/*) exit 2 ;; esac
    host_inode=$(stat -Lc %i /proc/thread-self/ns/mnt)
    exec /usr/bin/unshare --mount --propagation private /bin/bash "$0" inside \
      "$host_inode" "$binary" "$libraries"
    ;;
  inside)
    test "$EUID" -eq 0
    test "$#" -eq 4
    test "$(stat -Lc %i /proc/thread-self/ns/mnt)" != "$2"
    mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /etc
    mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /run
    mkdir /run/fe2o3-native-root-qualification
    touch /run/fe2o3-native-root-qualification/tests
    mkdir /run/fe2o3-native-root-qualification/lib
    mount --bind "$3" /run/fe2o3-native-root-qualification/tests
    mount -o remount,bind,ro,exec,nosuid,nodev /run/fe2o3-native-root-qualification/tests
    mount --bind "$4" /run/fe2o3-native-root-qualification/lib
    mount -o remount,bind,ro,nosuid,nodev /run/fe2o3-native-root-qualification/lib
    export FE2O3_ROOT_NATIVE_PRIVATE_FIXTURE=1
    export FE2O3_ROOT_NATIVE_HOST_MNTNS_INODE="$2"
    export LD_LIBRARY_PATH=/run/fe2o3-native-root-qualification/lib
    exec /usr/bin/timeout --signal=TERM --kill-after=5s 120s \
      /run/fe2o3-native-root-qualification/tests \
      --exact compiler_execution_production_deployment::root_native_v3::tests::private_root_fixed_native_installation_positive_and_replacement_controls \
      --ignored --nocapture --test-threads=1
    ;;
  *) exit 2 ;;
esac
