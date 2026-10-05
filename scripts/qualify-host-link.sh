#!/usr/bin/env bash
# Optional qualification observation, never an application admission mechanism.
# shellcheck disable=SC2034,SC2154
FE2O3_HOST_LINK_INPUT_MOUNTS=()
FE2O3_HOST_LINK_MOUNTS=()

prepare_host_link_input() {
    if [[ -z ${FE2O3_GENUINE_HOST_LINK_PROXY+x} && -z ${FE2O3_GENUINE_HOST_LINK_PROXY_SHA256+x} \
        && -z ${FE2O3_GENUINE_HOST_LINK_SHA256+x} ]]; then
        return
    fi
    [[ $campaign != resources && ${FE2O3_GENUINE_HOST_LINK_PROXY:-} == /* \
        && -f $FE2O3_GENUINE_HOST_LINK_PROXY && ! -L $FE2O3_GENUINE_HOST_LINK_PROXY \
        && ${FE2O3_GENUINE_HOST_LINK_PROXY_SHA256:-} =~ ^[0-9a-f]{64}$ ]] || {
        printf 'invalid genuine host-link observer configuration\n' >&2; return 1;
    }
    if [[ -n ${FE2O3_GENUINE_HOST_LINK_SHA256+x} \
        && ! ${FE2O3_GENUINE_HOST_LINK_SHA256} =~ ^[0-9a-f]{64}$ ]]; then
        printf 'invalid independent host-link premise pin\n' >&2; return 1;
    fi
    local observed
    observed=$(sha256sum -- "$FE2O3_GENUINE_HOST_LINK_PROXY") || return 1
    [[ ${observed%% *} == "$FE2O3_GENUINE_HOST_LINK_PROXY_SHA256" ]] || {
        printf 'host-link proxy differs from independent pin\n' >&2; return 1;
    }
    FE2O3_HOST_LINK_INPUT_MOUNTS=(
        --ro-bind "$FE2O3_GENUINE_HOST_LINK_PROXY" /run/qualification-host-link-input
        --setenv FE2O3_GENUINE_HOST_LINK_PROXY /run/qualification-host-link-input
    )
}

configure_host_link_observer() {
    [[ -n ${FE2O3_GENUINE_HOST_LINK_PROXY:-} ]] || return 0
    [[ $FE2O3_GENUINE_HOST_LINK_PROXY == /run/qualification-host-link-input ]] || return 1
    local rust_root=${FE2O3_AUTHORITY_RUSTC_PATH_V1%/bin/rustc}
    local -a expected=()
    if [[ -n ${FE2O3_GENUINE_HOST_LINK_SHA256:-} ]]; then
        expected=(--expected "$FE2O3_GENUINE_HOST_LINK_SHA256")
    fi
    /usr/bin/python3 -E -s -B "$repo/scripts/qualification_host_link.py" prepare \
        "$rust_root" "$FE2O3_GENUINE_HOST_LINK_PROXY" \
        "$FE2O3_GENUINE_HOST_LINK_PROXY_SHA256" "${expected[@]}" || return 1
    FE2O3_HOST_LINK_MOUNTS=(
        --ro-bind /run/qualification-host-link-v1/proxy /usr/bin/x86_64-linux-gnu-gcc-13
        --size 8589934592 --tmpfs /run/qualification-host-link-records
    )
}

set_genuine_campaign_command() {
    local test=provisioning::tests::genuine_application::root_genuine_application_campaign
    if [[ $campaign == genuine-two-gpu ]]; then
        test=provisioning::tests::genuine_application::root_genuine_two_gpu_application_campaign
    fi
    FE2O3_GENUINE_COMMAND=(
        /usr/bin/setpriv '--bounding-set=-all,+chown,+dac_override,+kill,+setgid,+setpcap,+setuid,+sys_ptrace'
        --inh-caps=-all --ambient-caps=-all
        /usr/libexec/fe2o3/resource-qualification --exact "$test"
        --ignored --nocapture --test-threads=1
    )
}

run_host_link_postflight() {
    local status=0
    "${FE2O3_GENUINE_COMMAND[@]}" || status=$?
    # A failed application is never converted to successful observation.
    [[ $status -eq 0 ]] || return "$status"
    /usr/bin/python3 -E -s -B "$repo/scripts/qualification_host_link.py" audit
}
