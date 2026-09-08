#!/usr/bin/env bash

# Target selection belongs to the compiler transaction. The application recovers
# the exact target and artifact from its authenticated V2/V5 carrier.
fe2o3_run_general_gemm_target() {
    local processor=${FE2O3_GENERAL_GEMM_TARGET:?missing FE2O3_GENERAL_GEMM_TARGET}
    if [[ $processor != gfx942 && $processor != gfx950 ]]; then
        printf 'FE2O3_GENERAL_GEMM_TARGET must be gfx942 or gfx950\n' >&2
        return 2
    fi

    local compile_only=${FE2O3_EXAMPLE_COMPILE_ONLY:-0}
    if [[ $compile_only != 0 && $compile_only != 1 ]]; then
        printf 'FE2O3_EXAMPLE_COMPILE_ONLY must be 0 or 1\n' >&2
        return 2
    fi
    local timeout_seconds=${FE2O3_HARDWARE_TIMEOUT_SECONDS:-600}
    if [[ ! $timeout_seconds =~ ^[0-9]+$ ]] || \
        (( timeout_seconds < 1 || timeout_seconds > 900 )); then
        printf 'FE2O3_HARDWARE_TIMEOUT_SECONDS must be an integer in 1..900\n' >&2
        return 2
    fi

    local example_dir cargo_fe2o3 timeout_bin scratch_root scratch subcommand
    example_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
    cargo_fe2o3=${FE2O3_CARGO_FE2O3:-$(command -v cargo-fe2o3 2>/dev/null || true)}
    timeout_bin=$(command -v timeout 2>/dev/null || true)
    if [[ -z $cargo_fe2o3 || $cargo_fe2o3 != /* || ! -f $cargo_fe2o3 || \
        ! -x $cargo_fe2o3 || -L $cargo_fe2o3 ]]; then
        printf 'FE2O3_CARGO_FE2O3 must name an absolute regular executable\n' >&2
        return 1
    fi
    if [[ -z $timeout_bin ]]; then
        printf 'GNU timeout is required\n' >&2
        return 1
    fi

    scratch_root=${FE2O3_RUN_SCRATCH_ROOT:-${TMPDIR:-/tmp}}
    scratch=$(mktemp -d "$scratch_root/fe2o3-general-gemm-$processor.XXXXXX")
    chmod 700 "$scratch"
    FE2O3_GENERAL_GEMM_RUN_SCRATCH=$scratch
    trap 'rm -rf -- "${FE2O3_GENERAL_GEMM_RUN_SCRATCH:?}"' EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    subcommand=run
    if [[ $compile_only == 1 ]]; then
        subcommand=build
    else
        fe2o3_tutorial_hardware_begin
    fi
    local environment=(-i LANG=C LC_ALL=C TZ=UTC "FE2O3_TARGET=$processor")
    local name
    for name in \
        CARGO \
        FE2O3_AUTHORITY_BACKEND_SHA256_V1 \
        FE2O3_AUTHORITY_CARGO_SHA256_V1 \
        FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1 \
        FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1 \
        FE2O3_AUTHORITY_RUSTC_PATH_V1 \
        FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1 \
        FE2O3_AUTHORITY_RUSTC_SHA256_V1 \
        FE2O3_BACKEND \
        FE2O3_PRODUCTION_BUILD_CONFIG_V1 \
        FE2O3_PRODUCTION_BUILD_CONFIG_V2 \
        FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT
    do
        if [[ -v $name ]]; then
            environment+=("$name=${!name}")
        fi
    done

    local command=(
        "$timeout_bin" --foreground --signal=TERM --kill-after=10 "$timeout_seconds"
        "$cargo_fe2o3" authority release "$subcommand" --locked
        --manifest-path "$example_dir/Cargo.toml"
        --target-dir "$scratch/target"
        --bin fe2o3-tiled-gemm-general-v1
    )
    if [[ $subcommand == run ]]; then
        command+=(-- --qualification)
    fi
    env "${environment[@]}" "${command[@]}"
    if [[ $subcommand == run ]]; then
        fe2o3_tutorial_hardware_finish "$processor"
    else
        printf 'COMPILE PASS: tiled_gemm_general_v1 reached %s:xnack-\n' "$processor"
    fi

    rm -rf -- "$FE2O3_GENERAL_GEMM_RUN_SCRATCH"
    unset FE2O3_GENERAL_GEMM_RUN_SCRATCH
    trap - EXIT HUP INT TERM
}
