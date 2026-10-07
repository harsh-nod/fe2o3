#!/usr/bin/env bash
set -euo pipefail
export PATH=/usr/bin:/bin LANG=C LC_ALL=C

readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly installed=/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5
[[ ${EUID} -eq 0 ]] || { printf 'qualification requires real root\n' >&2; exit 1; }
readonly campaign=${FE2O3_PROOF_INSTALL_CAMPAIGN:-resources}
[[ $campaign == resources || $campaign == genuine || $campaign == genuine-two-gpu \
  || $campaign == genuine-device-roster ]] || exit 2
source "$repo/scripts/qualify-two-gpu-mounts.sh"
validate_two_gpu_case
source "$repo/scripts/qualify-device-roster-mounts.sh"
validate_device_roster_case
source "$repo/scripts/qualify-application-inputs.sh"
source "$repo/scripts/qualify-host-link.sh"
FE2O3_GPU_MOUNTS=()
if [[ $campaign == genuine-two-gpu && -n ${1:-} ]]; then
  configure_two_gpu_mounts
elif [[ $campaign == genuine-device-roster && -n ${1:-} ]]; then
  configure_device_roster_mounts
fi

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
    campaign_mounts=()
    prepare_application_input_bundle
    prepare_host_link_input
    for name in FE2O3_PROOF_INSTALL_TEST FE2O3_STATIC_PROOF_CUSTODIAN_DIR \
      FE2O3_PROOF_INSTALL_COORDINATOR FE2O3_PROOF_INSTALL_WORKER \
      FE2O3_PROOF_RUNTIME_INPUTS FE2O3_PROOF_VERUS_DIST FE2O3_PROOF_RUST_TOOLCHAIN; do
      [[ "${!name:-}" == /* && -e "${!name}" ]] || {
        printf 'missing absolute qualification input: %s\n' "${name}" >&2; exit 1;
      }
    done
    if [[ $campaign != resources ]]; then
      for name in FE2O3_COMPILER_INSTALL_DIR FE2O3_COMPILER_INSTALL_LAUNCHER \
        FE2O3_GENUINE_APPLICATION FE2O3_GENUINE_CARGO_FE2O3 FE2O3_GENUINE_CARGO_REGISTRY \
        FE2O3_GENUINE_CARGO_GIT \
        FE2O3_GENUINE_JQ; do
        [[ ${!name:-} == /* && -e ${!name} ]] || {
          printf 'missing genuine campaign input: %s\n' "$name" >&2; exit 1;
        }
      done
      [[ -d /usr/libexec/gcc ]]
      campaign_mounts=(--ro-bind /usr/libexec/gcc /usr/libexec/gcc)
      if [[ -z ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]]; then
        campaign_mounts+=(--ro-bind "$FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1" \
          /run/authority-binding-trampoline --setenv FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1 \
          /run/authority-binding-trampoline)
      fi
    fi
    if [[ $campaign == genuine-two-gpu ]]; then
      # Resolve UIDs through the canonical read-only topology parser before any
      # namespace hides /dev. Only bounded numeric minors become mount paths.
      FE2O3_GENUINE_GPU_SELECTION=$(timeout --kill-after=2s 30s "$FE2O3_PROOF_INSTALL_TEST" \
        --exact provisioning::tests::genuine_application::observe_two_gpu_selection \
        --ignored --nocapture --test-threads=1 | read_two_gpu_selection)
      export FE2O3_GENUINE_GPU_SELECTION
      configure_two_gpu_mounts
    elif [[ $campaign == genuine-device-roster ]]; then
      FE2O3_GENUINE_GPU_ROSTER_SELECTION=$(timeout --kill-after=2s 30s "$FE2O3_PROOF_INSTALL_TEST" \
        --exact provisioning::tests::genuine_application::observe_device_roster_selection \
        --ignored --nocapture --test-threads=1 | read_device_roster_selection)
      export FE2O3_GENUINE_GPU_ROSTER_SELECTION
      configure_device_roster_mounts
    fi
    # Audit portable proof inputs before allocating any private installation state.
    # Later package/source/installed checks retain their independent authority.
    /bin/bash "$repo/scripts/functional-refinement-verus-runtime-v1.sh" \
      audit-qualification-source "$FE2O3_PROOF_VERUS_DIST" \
      "$FE2O3_PROOF_RUST_TOOLCHAIN" "$FE2O3_PROOF_RUNTIME_INPUTS"
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
        printf 'draining empty owned scope: %s\n' "$child"
        rmdir -- "$child"
        [[ $campaign != resources ]] || status=98
      done
      rmdir -- "$scope"
      printf 'outer cgroup removed: %s\n' "$scope"
      exit "$status"
    }
    trap cleanup EXIT
    trap 'exit 143' TERM
    trap 'exit 130' INT
    configure_application_input_mounts
    setup_script=$0
    setup_phase=prepare
    if [[ -n ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]]; then
      setup_script="$FE2O3_APPLICATION_DRIVER_MOUNT/scripts/qualify-proof-resource-inspection.sh"
      setup_phase=stage
    fi
    (
    printf '%s\n' "$BASHPID" > "$scope/cgroup.procs"
    exec timeout --kill-after=10s 1800s bwrap --die-with-parent \
      --unshare-pid --unshare-ipc --unshare-uts --unshare-net \
      --ro-bind / / --tmpfs /etc --ro-bind /etc/alternatives /etc/alternatives \
      --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
      --tmpfs /usr/libexec --tmpfs /opt \
      --tmpfs /run --tmpfs /var/lib --tmpfs /tmp --chmod 1777 /tmp --proc /proc --dev /dev \
      "${FE2O3_GPU_MOUNTS[@]}" \
      "${FE2O3_APPLICATION_INPUT_MOUNTS[@]}" \
      "${campaign_mounts[@]}" \
      "${FE2O3_HOST_LINK_INPUT_MOUNTS[@]}" \
      --bind /sys/fs/cgroup /sys/fs/cgroup \
      --cap-add CAP_CHOWN --cap-add CAP_DAC_OVERRIDE --cap-add CAP_KILL \
      --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
      /bin/bash "$setup_script" "$setup_phase"
    ) &
    wait "$!"
    ;;
  stage)
    require_private
    [[ $campaign != resources && $FE2O3_GENUINE_INPUT_BUNDLE == "$FE2O3_APPLICATION_INPUT_MOUNT" ]]
    confirmed=$(/usr/bin/python3 -E -s -B "$repo/scripts/qualification_input_bundle.py" \
      stage-application "$FE2O3_APPLICATION_TRANSPORT_MOUNT" \
      "$FE2O3_GENUINE_INPUT_SHA256" /run/qualification-input-v1)
    [[ $confirmed == "$FE2O3_GENUINE_INPUT_SOURCE_ROOT" ]]
    exec /bin/bash "$FE2O3_APPLICATION_INPUT_MOUNT/data/source/scripts/qualify-proof-resource-inspection.sh" prepare
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
    input_readonly=()
    if [[ -n ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]]; then
      input_readonly=(--ro-bind "$FE2O3_GENUINE_INPUT_BUNDLE" "$FE2O3_GENUINE_INPUT_BUNDLE"
        --tmpfs "$FE2O3_APPLICATION_TRANSPORT_MOUNT" --tmpfs "$FE2O3_APPLICATION_DRIVER_MOUNT")
    fi
    exec bwrap --die-with-parent --bind / / --dev /dev \
      "${FE2O3_GPU_MOUNTS[@]}" \
      "${input_readonly[@]}" \
      --ro-bind /usr/lib/x86_64-linux-gnu /run/host-lib \
      --ro-bind /run/setup/overlay /usr/lib/x86_64-linux-gnu \
      --tmpfs /usr/lib64 --symlink ../lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 /usr/lib64/ld-linux-x86-64.so.2 \
      --cap-add CAP_CHOWN --cap-add CAP_DAC_OVERRIDE --cap-add CAP_KILL \
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
    if [[ $campaign != resources ]]; then
      for name in fe2o3-compiler-execution-provision fe2o3-compiler-execution-supervisor \
        fe2o3-compiler-execution-issuer fe2o3-external-anchor-provisioning-helper \
        fe2o3-external-anchor-service; do
        install -m 0555 "$FE2O3_COMPILER_INSTALL_DIR/$name" "/usr/libexec/fe2o3/$name"
      done
      install -m 0555 "$FE2O3_COMPILER_INSTALL_LAUNCHER" /usr/libexec/fe2o3/fe2o3-static-preexec-launcher
      configure_application_projection
      application_script=$0
      if [[ -n ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]]; then
        application_script="$FE2O3_GENUINE_INPUT_SOURCE_ROOT/scripts/qualify-proof-resource-inspection.sh"
      fi
      # Bundle mode hides host homes. Legacy mode retains them; both still use
      # the separately qualified host setup/linker premise.
      exec bwrap --die-with-parent --bind / / --dev /dev --tmpfs /etc \
        "${FE2O3_GPU_MOUNTS[@]}" \
        "${FE2O3_APPLICATION_PROJECTION[@]}" \
        --ro-bind /etc/alternatives /etc/alternatives \
        --size 8589934592 --tmpfs /run/application-target \
        --cap-add CAP_CHOWN --cap-add CAP_DAC_OVERRIDE --cap-add CAP_KILL \
        --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
        /bin/bash "$application_script" genuine
    fi
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
  genuine)
    require_private
    [[ $campaign != resources ]]
    [[ "$(stat -f -c %T /var/lib)" == tmpfs ]]
    printf '%s\n' \
      'root:x:0:0:root:/root:/bin/false' \
      'fe2o3-client:x:1000:1000:application:/run/application-home:/bin/false' \
      'fe2o3-compiler:x:61000:1000:compiler:/nonexistent:/bin/false' \
      'fe2o3-anchor:x:61001:61001:anchor:/nonexistent:/bin/false' \
      'fe2o3-proof:x:61002:61003:proof:/nonexistent:/bin/false' > /etc/passwd
    printf '%s\n' 'root:x:0:' 'fe2o3-compiler:x:1000:' \
      'fe2o3-anchor:x:61001:' 'fe2o3-proof:x:61003:' > /etc/group
    install -d -m 0755 /run/fe2o3 /etc/fe2o3/compiler-execution /var/lib/fe2o3
    install -d -m 0700 -o 61000 -g 1000 /var/lib/fe2o3/compiler-execution
    install -d -m 0700 -o 61001 -g 61001 /var/lib/fe2o3/external-anchor
    install -m 0400 /dev/null /var/lib/fe2o3/compiler-execution-lifecycle-v1
    install -d -m 0700 /run/fe2o3-proof-custodian
    install -d -m 0700 -o 61002 -g 61003 /run/candidates
    install -d -m 0700 -o 1000 -g 1000 /run/application-home /run/application-home/.cargo
    chown 1000:1000 /run/application-target
    chmod 0700 /run/application-target
    export FE2O3_PRODUCTION_BUILD_CONFIG_V1=/run/genuine-build-config.json
    "$FE2O3_GENUINE_JQ" -cnjS --arg cwd "$FE2O3_GENUINE_APPLICATION" \
      --arg sha "$(sha256sum /usr/libexec/fe2o3/fe2o3-llvm-link-worker | cut -d ' ' -f 1)" \
      --argjson bytes "$(stat -c %s /usr/libexec/fe2o3/fe2o3-llvm-link-worker)" \
      '{candidate_output_max_bytes:1048576,format:"fe2o3-production-build-config-v1",
        limits:{stderr_bytes:65536,stdout_bytes:4194304,timeout_ms:60000},
        link_options:[{name:"code-object-version",value:"6"},{name:"opt-level",value:"2"},
          {name:"strip-debug",value:"false"},{name:"verify-each",value:"true"}],providers:[],
        units:[{crate_name:"fe2o3_conditional_custodian_application",source:"src/lib.rs",working_directory:$cwd}],
        worker:{byte_len:$bytes,llvm_build_identity:"7.2.4",path:"/usr/libexec/fe2o3/fe2o3-llvm-link-worker",
          sha256:$sha,worker_build_identity:"fe2o3-worker-v1-sha256-f36a39930e3f3075570d6862fcf09bb5cd3b62e86a9f0344df2cf8a6f1d03575"}}' \
      > "$FE2O3_PRODUCTION_BUILD_CONFIG_V1"
    chmod 0444 "$FE2O3_PRODUCTION_BUILD_CONFIG_V1"
    umask 077
    set_genuine_campaign_command
    configure_host_link_observer
    if [[ -n ${FE2O3_GENUINE_HOST_LINK_PROXY:-} ]]; then
      FE2O3_GENUINE_COMMAND=(/bin/bash "$0" genuine-link-run)
    fi
    # Keep Cargo's cache lock private, with existing offline registry content mounted read-only.
    exec bwrap --die-with-parent --bind / / --dev /dev \
      "${FE2O3_GPU_MOUNTS[@]}" \
      "${FE2O3_HOST_LINK_MOUNTS[@]}" \
      --ro-bind "$FE2O3_GENUINE_CARGO_REGISTRY" /run/application-home/.cargo/registry \
      --ro-bind "$FE2O3_GENUINE_CARGO_GIT" /run/application-home/.cargo/git \
      --cap-drop ALL \
      --cap-add CAP_CHOWN --cap-add CAP_DAC_OVERRIDE --cap-add CAP_KILL \
      --chdir / --cap-add CAP_SETUID --cap-add CAP_SETGID --cap-add CAP_SETPCAP --cap-add CAP_SYS_PTRACE \
      "${FE2O3_GENUINE_COMMAND[@]}"
    ;;
  genuine-link-run)
    require_private
    [[ $campaign != resources && -n ${FE2O3_GENUINE_HOST_LINK_PROXY:-} ]]
    [[ "$(stat -f -c %T /run/qualification-host-link-records)" == tmpfs ]]
    chmod 0700 /run/qualification-host-link-records
    chown 1000:1000 /run/qualification-host-link-records
    umask 077
    set_genuine_campaign_command
    run_host_link_postflight
    ;;
  *) exit 2 ;;
esac
