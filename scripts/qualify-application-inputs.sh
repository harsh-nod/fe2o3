#!/usr/bin/env bash
# Sourced by the real-root qualification harness; never grants runtime authority.
# The harness supplies repo/campaign and consumes the mount arrays.
# shellcheck disable=SC2034,SC2154
readonly FE2O3_APPLICATION_INPUT_MOUNT=/run/qualification-input-v1/copied
readonly FE2O3_APPLICATION_TRANSPORT_MOUNT=/run/qualification-transport-v1
readonly FE2O3_APPLICATION_DRIVER_MOUNT=/run/qualification-driver-v1
FE2O3_APPLICATION_INPUT_MOUNTS=()
FE2O3_APPLICATION_PROJECTION=()

set_application_input_paths() {
    local root=$1 variable relative
    while IFS='|' read -r variable relative; do
        printf -v "$variable" '%s/%s' "$root" "$relative"
        export "${variable?}"
    done <<'PATHS'
FE2O3_GENUINE_APPLICATION|source/crates/cargo-fe2o3/tests/fixtures/conditional-custodian-application
FE2O3_GENUINE_CARGO_FE2O3|tools/cargo-fe2o3
CARGO|rust/bin/cargo
FE2O3_AUTHORITY_RUSTC_PATH_V1|rust/bin/rustc
FE2O3_BACKEND|tools/librustc_codegen_fe2o3.so
FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1|tools/binding-trampoline
FE2O3_GENUINE_CARGO_REGISTRY|cargo/registry
FE2O3_GENUINE_CARGO_GIT|cargo/git
FE2O3_GENUINE_JQ|tools/jq
PATHS
}

prepare_application_input_bundle() {
    if [[ -z ${FE2O3_GENUINE_INPUT_BUNDLE+x} && -z ${FE2O3_GENUINE_INPUT_SHA256+x} ]]; then
        return
    fi
    [[ $campaign != resources && ${FE2O3_GENUINE_INPUT_BUNDLE:-} == /* \
        && ${FE2O3_GENUINE_INPUT_SHA256:-} =~ ^[0-9a-f]{64}$ ]] || {
        printf 'invalid genuine application bundle configuration\n' >&2; return 1;
    }
    FE2O3_GENUINE_INPUT_SOURCE_ROOT=$(/usr/bin/python3 -E -s -B \
        "$repo/scripts/qualification_input_bundle.py" verify-application \
        "$FE2O3_GENUINE_INPUT_BUNDLE" "$FE2O3_GENUINE_INPUT_SHA256") || return 1
    export FE2O3_GENUINE_INPUT_SOURCE_ROOT
    set_application_input_paths "$FE2O3_GENUINE_INPUT_BUNDLE/data"
}

configure_application_input_mounts() {
    [[ -n ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]] || return 0
    FE2O3_APPLICATION_INPUT_MOUNTS=(
        --ro-bind "$FE2O3_GENUINE_INPUT_BUNDLE" "$FE2O3_APPLICATION_TRANSPORT_MOUNT"
        --ro-bind "$repo" "$FE2O3_APPLICATION_DRIVER_MOUNT"
        --setenv FE2O3_GENUINE_INPUT_BUNDLE "$FE2O3_APPLICATION_INPUT_MOUNT"
    )
    set_application_input_paths "$FE2O3_APPLICATION_INPUT_MOUNT/data"
}

configure_application_projection() {
    [[ -n ${FE2O3_GENUINE_INPUT_BUNDLE:-} ]] || return 0
    local confirmed path part variable
    # Recheck the fresh root-owned stage before hiding source aliases. Production
    # resource custody remains independent of this qualification transport.
    confirmed=$(/usr/bin/python3 -E -s -B "$repo/scripts/qualification_input_bundle.py" \
        verify-application "$FE2O3_GENUINE_INPUT_BUNDLE" \
        "$FE2O3_GENUINE_INPUT_SHA256") || return 1
    [[ $confirmed == "$FE2O3_GENUINE_INPUT_SOURCE_ROOT" ]] || return 1
    FE2O3_APPLICATION_PROJECTION=(--tmpfs /home --chmod 0555 /home --tmpfs /root)
    path=/home
    local -a parts
    IFS=/ read -r -a parts <<< "${confirmed#/home/}"
    for part in "${parts[@]}"; do
        path+="/$part"
        FE2O3_APPLICATION_PROJECTION+=(--dir "$path" --chmod 0555 "$path")
    done
    FE2O3_APPLICATION_PROJECTION+=(
        --ro-bind "$FE2O3_APPLICATION_INPUT_MOUNT/data/source" "$confirmed"
        --setenv FE2O3_GENUINE_APPLICATION
        "$confirmed/crates/cargo-fe2o3/tests/fixtures/conditional-custodian-application"
    )
    for variable in FE2O3_PROOF_INSTALL_TEST FE2O3_STATIC_PROOF_CUSTODIAN_DIR \
        FE2O3_PROOF_INSTALL_COORDINATOR FE2O3_PROOF_INSTALL_WORKER \
        FE2O3_PROOF_RUNTIME_INPUTS FE2O3_PROOF_VERUS_DIST FE2O3_PROOF_RUST_TOOLCHAIN \
        FE2O3_COMPILER_INSTALL_DIR FE2O3_COMPILER_INSTALL_LAUNCHER; do
        FE2O3_APPLICATION_PROJECTION+=(--unsetenv "$variable")
    done
}
