#!/usr/bin/env bash

# Shared qualification runner for ordinary Rust kernels lowered to gfx942.

fe2o3_run_gfx942() {
    : "${FE2O3_EXAMPLE_DIR:?missing FE2O3_EXAMPLE_DIR}"
    : "${FE2O3_EXAMPLE_CRATE:?missing FE2O3_EXAMPLE_CRATE}"
    : "${FE2O3_EXAMPLE_STEM:?missing FE2O3_EXAMPLE_STEM}"
    : "${FE2O3_EXAMPLE_HOST_BIN:?missing FE2O3_EXAMPLE_HOST_BIN}"
    : "${FE2O3_EXAMPLE_HSACO_ENV:?missing FE2O3_EXAMPLE_HSACO_ENV}"
    : "${FE2O3_EXAMPLE_LINK_DEVICE_LIBS:?missing FE2O3_EXAMPLE_LINK_DEVICE_LIBS}"

    local repo_root toolchain root_target output_dir rocm_dir sysroot extractor
    local llvm_ir linked_ir object hsaco amd_target binding_path binding compiler_input
    repo_root=$(cd -- "$FE2O3_EXAMPLE_DIR/../.." && pwd)
    toolchain=${FE2O3_RUST_TOOLCHAIN:-nightly-2026-04-03}
    root_target=${FE2O3_ROOT_TARGET_DIR:-$repo_root/target}
    output_dir=${FE2O3_OUTPUT_DIR:-$FE2O3_EXAMPLE_DIR/target/fe2o3-gfx942}
    rocm_dir=${ROCM_PATH:-/opt/rocm}
    FE2O3_RUN_WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-gfx942.XXXXXX")
    chmod 700 "$FE2O3_RUN_WORK_DIR"
    trap 'rm -rf -- "${FE2O3_RUN_WORK_DIR:?}"' EXIT

    sysroot=$(rustup run "$toolchain" rustc --print sysroot)
    extractor="$root_target/debug/fe2o3-rustc-extract"
    llvm_ir="$FE2O3_RUN_WORK_DIR/$FE2O3_EXAMPLE_STEM.ll"
    linked_ir="$FE2O3_RUN_WORK_DIR/$FE2O3_EXAMPLE_STEM.bc"
    object="$FE2O3_RUN_WORK_DIR/$FE2O3_EXAMPLE_STEM.o"
    hsaco="$output_dir/$FE2O3_EXAMPLE_STEM.hsaco"
    amd_target="$FE2O3_RUN_WORK_DIR/amd-target"
    binding_path="$FE2O3_RUN_WORK_DIR/crate-binding-v1"
    if [[ -e "$binding_path" || -L "$binding_path" ]]; then
        printf 'crate-binding handoff path was not attempt-fresh\n' >&2
        return 1
    fi

    mkdir -p -- "$output_dir"
    CARGO_TARGET_DIR="$root_target" rustup run "$toolchain" cargo build \
        --locked --manifest-path "$repo_root/Cargo.toml" \
        -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract

    (
        cd -- "$FE2O3_EXAMPLE_DIR"
        FE2O3_EXTRACT_CRATE_V1="$FE2O3_EXAMPLE_CRATE" \
        FE2O3_EXTRACT_CRATE_BINDING_PATH_V1="$binding_path" \
        FE2O3_EXTRACT_GFX942_LLVM_PATH_V1="$llvm_ir" \
        RUSTC_WORKSPACE_WRAPPER="$extractor" \
        CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS='-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32' \
        LD_LIBRARY_PATH="$root_target/debug/deps:$sysroot/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        rustup run "$toolchain" cargo check --release --locked -Zbuild-std=core \
            --target amdgcn-amd-amdhsa --target-dir "$amd_target" --lib
    )

    if [[ ! -f "$binding_path" || -L "$binding_path" ]]; then
        printf 'compiler did not publish a regular crate-binding handoff\n' >&2
        return 1
    fi
    if [[ $(stat -c '%a:%h:%F' "$binding_path") != '600:1:regular file' ]]; then
        printf 'compiler published an insecure crate-binding handoff\n' >&2
        return 1
    fi
    if [[ $(wc -c < "$binding_path") -ne 65 ]]; then
        printf 'compiler published a malformed crate-binding handoff length\n' >&2
        return 1
    fi
    IFS= read -r binding < "$binding_path"
    if [[ ! "$binding" =~ ^[0-9a-f]{64}$ ]]; then
        printf 'compiler published a noncanonical crate-binding handoff\n' >&2
        return 1
    fi

    compiler_input=$llvm_ir
    if [[ "$FE2O3_EXAMPLE_LINK_DEVICE_LIBS" == 1 ]]; then
        "$rocm_dir/llvm/bin/llvm-link" \
            "$llvm_ir" \
            "$rocm_dir/amdgcn/bitcode/ocml.bc" \
            "$rocm_dir/amdgcn/bitcode/oclc_isa_version_942.bc" \
            "$rocm_dir/amdgcn/bitcode/oclc_unsafe_math_off.bc" \
            "$rocm_dir/amdgcn/bitcode/oclc_finite_only_off.bc" \
            -o "$linked_ir"
        compiler_input=$linked_ir
    elif [[ "$FE2O3_EXAMPLE_LINK_DEVICE_LIBS" != 0 ]]; then
        printf 'FE2O3_EXAMPLE_LINK_DEVICE_LIBS must be 0 or 1\n' >&2
        return 1
    fi

    "$rocm_dir/llvm/bin/clang" -O3 -nogpulib -x ir \
        --target=amdgcn-amd-amdhsa -mcpu=gfx942 -mno-xnack \
        -c "$compiler_input" -o "$object"
    "$rocm_dir/llvm/bin/ld.lld" -shared "$object" -o "$hsaco"

    (
        cd -- "$FE2O3_EXAMPLE_DIR"
        env -u FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2 \
            "$FE2O3_EXAMPLE_HSACO_ENV=$hsaco" \
            "FE2O3_CRATE_BINDING_ID_V1=$binding" \
            "CARGO_TARGET_DIR=$root_target" \
            rustup run "$toolchain" cargo run --release --locked \
                --bin "$FE2O3_EXAMPLE_HOST_BIN"
    )

    printf 'HSACO: %s\n' "$hsaco"
}
