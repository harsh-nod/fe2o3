#!/usr/bin/env bash

# Sourced by the gfx950 attention build scripts. The caller must set
# SCRIPT_DIR, CLANG, LD_LLD, and SHA256SUM before invoking this function.

validate_gfx950_ocml_closure() {
    local manifest=${FE2O3_GFX950_OCML_MANIFEST:-$SCRIPT_DIR/gfx950-ocml-rocm-7.2.1.manifest}
    local expected_key_count=15
    local key actual file
    local -a libraries=(
        ocml.bc
        ockl.bc
        oclc_daz_opt_off.bc
        oclc_unsafe_math_off.bc
        oclc_finite_only_off.bc
        oclc_correctly_rounded_sqrt_on.bc
        oclc_wavefrontsize64_on.bc
        oclc_isa_version_950.bc
        oclc_abi_version_600.bc
    )

    if [[ ! -f $manifest || -L $manifest ]]; then
        printf 'gfx950 OCML manifest is not a regular non-symlink file: %s\n' "$manifest" >&2
        return 1
    fi
    if [[ $(wc -l < "$manifest") -ne $expected_key_count ]] ||
        ! awk -F= '
            NF != 2 || $1 !~ /^[A-Za-z0-9_.-]+$/ || $2 == "" { exit 1 }
            seen[$1]++ > 0 { exit 1 }
        ' "$manifest"; then
        printf 'gfx950 OCML manifest is noncanonical\n' >&2
        return 1
    fi

    manifest_value() {
        local requested=$1
        awk -F= -v requested="$requested" '$1 == requested { print $2 }' "$manifest"
    }

    if [[ $(manifest_value schema) != fe2o3-gfx950-ocml-closure-v1 ]] ||
        [[ $(manifest_value rocm_version) != 7.2.1 ]] ||
        [[ $(manifest_value llvm_version) != 22.0.0git ]]; then
        printf 'gfx950 OCML manifest version is not the reviewed v1 closure\n' >&2
        return 1
    fi

    GFX950_OCML_DEVICE_LIBRARY_DIR=$(manifest_value canonical_device_library_dir)
    if [[ -z $GFX950_OCML_DEVICE_LIBRARY_DIR ]] ||
        [[ $(readlink -f -- "$GFX950_OCML_DEVICE_LIBRARY_DIR") != "$GFX950_OCML_DEVICE_LIBRARY_DIR" ]]; then
        printf 'gfx950 OCML device-library directory is absent or noncanonical\n' >&2
        return 1
    fi

    GFX950_OCML_CLANG_ARGS=()
    for file in "${libraries[@]}"; do
        expected=$(manifest_value "$file")
        if [[ ! $expected =~ ^[0-9a-f]{64}$ ]] ||
            [[ ! -f $GFX950_OCML_DEVICE_LIBRARY_DIR/$file ]] ||
            [[ -L $GFX950_OCML_DEVICE_LIBRARY_DIR/$file ]]; then
            printf 'gfx950 OCML input is absent or malformed: %s\n' "$file" >&2
            return 1
        fi
        actual=$("$SHA256SUM" -- "$GFX950_OCML_DEVICE_LIBRARY_DIR/$file" | awk '{ print $1 }')
        if [[ $actual != "$expected" ]]; then
            printf 'gfx950 OCML input digest mismatch: %s\n' "$file" >&2
            return 1
        fi
        GFX950_OCML_CLANG_ARGS+=(
            -Xclang -mlink-builtin-bitcode
            -Xclang "$GFX950_OCML_DEVICE_LIBRARY_DIR/$file"
        )
    done

    for key in clang-22 lld; do
        if [[ $key == clang-22 ]]; then
            file=$(readlink -f -- "$CLANG")
        else
            file=$(readlink -f -- "$LD_LLD")
        fi
        expected=$(manifest_value "$key")
        actual=$("$SHA256SUM" -- "$file" | awk '{ print $1 }')
        if [[ ! $expected =~ ^[0-9a-f]{64}$ || $actual != "$expected" ]]; then
            printf 'gfx950 OCML tool digest mismatch: %s (%s)\n' "$key" "$file" >&2
            return 1
        fi
    done

    readonly GFX950_OCML_DEVICE_LIBRARY_DIR
    readonly -a GFX950_OCML_CLANG_ARGS
}
