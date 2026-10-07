#!/usr/bin/env bash
# Real-root fixed-path qualification, confined to private tmpfs mounts.
set -Eeuo pipefail

case ${1:-} in
  run|run-manager)
    test "$EUID" -eq 0
    test "$#" -eq 4
    binary=$(realpath -e -- "$2")
    libraries=$(realpath -e -- "$3")
    controller=$(realpath -e -- "$4")
    test -f "$binary" && test -x "$binary"
    test -d "$libraries" && test -f "$controller"
    test -d /usr/libexec
    for input in "$binary" "$libraries" "$controller"; do
      case "$input" in /etc/*|/run/*|/usr/libexec/*) exit 2 ;; esac
    done
    host_inode=$(stat -Lc %i /proc/thread-self/ns/mnt)
    mode=inside
    if [[ $1 == run-manager ]]; then mode=inside-manager; fi
    exec /usr/bin/unshare --mount --propagation private /bin/bash "$0" "$mode" \
      "$host_inode" "$binary" "$libraries" "$controller"
    ;;
  inside|inside-manager)
    test "$EUID" -eq 0
    test "$#" -eq 5
    test "$(stat -Lc %i /proc/thread-self/ns/mnt)" != "$2"
    mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /etc
    mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /usr/libexec
    mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /run
    mkdir /run/fe2o3-native-proof-qualification
    touch /run/fe2o3-native-proof-qualification/tests
    mkdir /run/fe2o3-native-proof-qualification/lib
    mount --bind "$3" /run/fe2o3-native-proof-qualification/tests
    mount -o remount,bind,ro,exec,nosuid,nodev /run/fe2o3-native-proof-qualification/tests
    mount --bind "$4" /run/fe2o3-native-proof-qualification/lib
    mount -o remount,bind,ro,nosuid,nodev /run/fe2o3-native-proof-qualification/lib
    export FE2O3_NATIVE_PROOF_PRIVATE_FIXTURE=1
    export FE2O3_NATIVE_PROOF_HOST_MNTNS_INODE="$2"
    export FE2O3_NATIVE_PROOF_CONTROLLER_IMAGE="$5"
    export LD_LIBRARY_PATH=/run/fe2o3-native-proof-qualification/lib
    test_name=deployment::native::tests::private_root::fixed_native_proof_deployment_positive_and_replacement_controls
    if [[ $1 == inside-manager ]]; then
      test_name=deployment::native::tests::private_root::manager::fixed_native_manager_installation_and_original_owner_controls
    fi
    listing=$(/usr/bin/timeout --kill-after=1s 10s \
      /run/fe2o3-native-proof-qualification/tests --list --ignored --exact "$test_name")
    [[ "$listing" == "$test_name: test"$'\n\n1 test, 0 benchmarks' ]]
    exec /usr/bin/timeout --signal=TERM --kill-after=5s 120s \
      /run/fe2o3-native-proof-qualification/tests --exact \
      "$test_name" \
      --ignored --nocapture --test-threads=1
    ;;
  *) exit 2 ;;
esac
