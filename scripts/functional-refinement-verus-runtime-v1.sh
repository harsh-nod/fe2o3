#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIRECTORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIRECTORY
REPOSITORY_ROOT="$(cd -- "$SCRIPT_DIRECTORY/.." && pwd -P)"
readonly REPOSITORY_ROOT
readonly PINS_DIRECTORY="$REPOSITORY_ROOT/crates/fe2o3-verifier/verus/pins"
readonly TARGET_PINS="$PINS_DIRECTORY/rust_target_1_97_1.sha256"
readonly MANIFEST="$PINS_DIRECTORY/FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest"
readonly INSTALLED_MANIFEST_NAME=FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest
readonly RUNTIME_LABEL="functional-refinement Verus runtime V1"
readonly SUCCESS_PREFIX=FE2O3_FUNCTIONAL_REFINEMENT_RUNTIME_V1
readonly TARGET_PREFIX=toolchain/lib/rustlib/x86_64-unknown-linux-gnu/lib
readonly TARGET_SOURCE_PREFIX=lib/rustlib/x86_64-unknown-linux-gnu/lib
readonly QUALIFICATION_PACKAGES="$SCRIPT_DIRECTORY/compiler-execution-qualification-base-packages-v1.lock"

die() {
    printf '%s: %s\n' "$RUNTIME_LABEL" "$*" >&2
    exit 1
}

usage() {
    cat >&2 <<'EOF'
usage:
  functional-refinement-verus-runtime-v1.sh audit-source VERUS_DIST RUST_TOOLCHAIN RUSTUP
  functional-refinement-verus-runtime-v1.sh audit-qualification-source VERUS_DIST RUST_TOOLCHAIN INPUTS
  functional-refinement-verus-runtime-v1.sh provision VERUS_DIST RUST_TOOLCHAIN RUSTUP DESTINATION
  functional-refinement-verus-runtime-v1.sh audit-installed RUNTIME_ROOT

`provision` must run as root and accepts only a new destination beneath
/opt/fe2o3/verus-runtime-v2/. The Verus launcher and rustup are audited as
excluded provenance; neither is copied into the executable closure.
These commands grant no proof authority. The typed Rust entry point separately
admits and retains the installed closure, revalidates it around every bounded
proof process, and returns only non-authoritative refinement evidence.
`audit-qualification-source` reads INPUTS/{libc.deb,zlib.deb,rustup-init}
without extracting, installing, or executing them. It does not approve the
installed interpreter or replace the post-overlay source/installed audits.
EOF
    exit 2
}

require_absolute_directory() {
    local path=$1
    local label=$2
    [[ "$path" == /* && "$path" != *$'\n'* ]] || die "$label must be a normalized absolute path"
    [[ -d "$path" && ! -L "$path" ]] || die "$label is not a no-follow directory: $path"
}

header_value() {
    local key=$1
    local value
    value="$(awk -F '|' -v key="$key" '$1 == key { if (seen++) exit 3; print substr($0, length($1) + 2) }' "$MANIFEST")" \
        || die "manifest has duplicate $key records"
    [[ -n "$value" ]] || die "manifest is missing $key"
    printf '%s\n' "$value"
}

sha256_file() {
    local digest
    digest="$(sha256sum -- "$1" | awk '{print $1}')" || die "cannot hash file: $1"
    [[ "$digest" =~ ^[0-9a-f]{64}$ ]] || die "malformed file digest: $1"
    printf '%s\n' "$digest"
}

report_success() {
    local digest
    digest="$(sha256_file "$MANIFEST")" || die "cannot hash manifest"
    if [[ $# == 2 ]]; then
        printf '%s_%s_OK manifest_sha256=%s root=%s\n' "$SUCCESS_PREFIX" "$1" "$digest" "$2"
    else
        printf '%s_%s_OK manifest_sha256=%s\n' "$SUCCESS_PREFIX" "$1" "$digest"
    fi
}

require_regular_source() {
    local path=$1
    [[ -f "$path" && ! -L "$path" ]] || die "source is not a no-follow regular file: $path"
    [[ "$(stat -Lc %h -- "$path")" == 1 ]] || die "source has multiple hard links: $path"
}

system_source() {
    case "$1" in
        system-lib/libc.so.6) printf '%s\n' /usr/lib/x86_64-linux-gnu/libc.so.6 ;;
        system-lib/libdl.so.2) printf '%s\n' /usr/lib/x86_64-linux-gnu/libdl.so.2 ;;
        system-lib/libgcc_s.so.1) printf '%s\n' /usr/lib/x86_64-linux-gnu/libgcc_s.so.1 ;;
        system-lib/libm.so.6) printf '%s\n' /usr/lib/x86_64-linux-gnu/libm.so.6 ;;
        system-lib/libpthread.so.0) printf '%s\n' /usr/lib/x86_64-linux-gnu/libpthread.so.0 ;;
        system-lib/librt.so.1) printf '%s\n' /usr/lib/x86_64-linux-gnu/librt.so.1 ;;
        system-lib/libstdc++.so.6) printf '%s\n' /usr/lib/x86_64-linux-gnu/libstdc++.so.6.0.33 ;;
        system-lib/libz.so.1) printf '%s\n' /usr/lib/x86_64-linux-gnu/libz.so.1.3 ;;
        *) die "manifest names an unsupported system DSO: $1" ;;
    esac
}

source_for_file() {
    local relative=$1
    local distribution=$2
    local toolchain=$3
    case "$relative" in
        dist/*) printf '%s/%s\n' "$distribution" "${relative#dist/}" ;;
        toolchain/*) printf '%s/%s\n' "$toolchain" "${relative#toolchain/}" ;;
        system-lib/*) system_source "$relative" ;;
        *) die "manifest file is outside the closed layout: $relative" ;;
    esac
}

validate_pin_contract() {
    [[ -f "$MANIFEST" && -f "$TARGET_PINS" ]] || die "runtime V2 pins are missing"
    [[ "$(tail -c 1 "$MANIFEST" | od -An -tuC | tr -d ' ')" == 10 ]] \
        || die "manifest must end in exactly one newline"
    [[ "$(tail -c 1 "$TARGET_PINS" | od -An -tuC | tr -d ' ')" == 10 ]] \
        || die "target pins must end in exactly one newline"

    local pin_record pin_count pin_bytes pin_digest actual
    pin_record="$(header_value rust-target-pins)"
    IFS='|' read -r pin_count pin_bytes pin_digest <<<"$pin_record"
    [[ "$pin_count" =~ ^[0-9]+$ && "$pin_bytes" =~ ^[0-9]+$ && "$pin_digest" =~ ^[0-9a-f]{64}$ ]] \
        || die "rust-target-pins record is malformed"
    [[ "$(wc -l < "$TARGET_PINS" | tr -d ' ')" == "$pin_count" ]] || die "target pin count differs"
    [[ "$(wc -c < "$TARGET_PINS" | tr -d ' ')" == "$pin_bytes" ]] || die "target pin byte count differs"
    actual="$(sha256_file "$TARGET_PINS")" || die "cannot hash target pins"
    [[ "$actual" == "$pin_digest" ]] || die "target pin digest differs"
    awk 'length($1) != 64 || $1 !~ /^[0-9a-f]+$/ || $2 == "" || NF != 2 { exit 1 } { print $2 }' "$TARGET_PINS" \
        | LC_ALL=C sort -c -u || die "target pins are malformed or not strictly sorted"
    awk -F '|' '$1 == "directory" { print $3 }' "$MANIFEST" | LC_ALL=C sort -c -u \
        || die "manifest directories are not strictly sorted"
    awk -F '|' '$1 == "file" { print $5 }' "$MANIFEST" | LC_ALL=C sort -c -u \
        || die "manifest files are not strictly sorted"
}

verify_file() {
    local path=$1
    local expected_mode=$2
    local expected_size=$3
    local expected_digest=$4
    local require_root=${5:-false}
    local actual
    require_regular_source "$path"
    [[ "$(stat -Lc %s -- "$path")" == "$expected_size" ]] || die "size differs: $path"
    actual="$(sha256_file "$path")" || die "cannot hash source: $path"
    [[ "$actual" == "$expected_digest" ]] || die "SHA-256 differs: $path"
    if [[ "$require_root" == true ]]; then
        [[ "$(stat -Lc %a -- "$path")" == "${expected_mode#0}" ]] || die "mode differs: $path"
        [[ "$(stat -Lc '%u:%g' -- "$path")" == 0:0 ]] || die "owner differs: $path"
    fi
}

verify_interpreter() {
    local record requested canonical size digest kind link target
    record="$(header_value interpreter)"
    IFS='|' read -r requested canonical size digest <<<"$record"
    [[ "$requested" == /lib64/ld-linux-x86-64.so.2 ]] || die "PT_INTERP path differs"
    [[ "$canonical" == /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 ]] \
        || die "canonical interpreter path differs"
    verify_file "$canonical" 0755 "$size" "$digest" true
    while IFS='|' read -r kind link target; do
        [[ "$kind" == interpreter-link ]] || continue
        [[ -L "$link" ]] || die "interpreter link is not a symbolic link: $link"
        [[ "$(readlink -- "$link")" == "$target" ]] || die "interpreter link target differs: $link"
        [[ "$(stat -c '%u:%g' -- "$link")" == 0:0 ]] || die "interpreter link owner differs: $link"
    done < "$MANIFEST"
    [[ "$(readlink -f -- "$requested")" == "$canonical" ]] || die "PT_INTERP chain resolves elsewhere"
}

audit_toolchain_source() {
    local distribution=$1
    local toolchain=$2
    local rustup=$3
    require_absolute_directory "$distribution" VERUS_DIST
    require_absolute_directory "$toolchain" RUST_TOOLCHAIN
    [[ "$rustup" == /* ]] || die "RUSTUP must be absolute"
    require_regular_source "$rustup"
    validate_pin_contract

    local launcher_record launcher_size launcher_digest
    launcher_record="$(header_value launcher-excluded)"
    IFS='|' read -r launcher_size launcher_digest <<<"$launcher_record"
    verify_file "$distribution/verus" 0555 "$launcher_size" "$launcher_digest" false
    local rustup_record rustup_size rustup_digest
    rustup_record="$(header_value rustup-excluded)"
    IFS='|' read -r rustup_size rustup_digest <<<"$rustup_record"
    verify_file "$rustup" 0555 "$rustup_size" "$rustup_digest" false

    local kind mode size digest relative actual
    # Direct record iteration cannot lose an enumeration subprocess's exit status.
    while IFS='|' read -r kind mode size digest relative; do
        [[ "$kind" == file ]] || continue
        [[ "$relative" != system-lib/* ]] || continue
        local source
        source="$(source_for_file "$relative" "$distribution" "$toolchain")"
        verify_file "$source" "$mode" "$size" "$digest" false
    done < "$MANIFEST"
    while IFS=' ' read -r digest name; do
        local source="$toolchain/$TARGET_SOURCE_PREFIX/$name"
        require_regular_source "$source"
        actual="$(sha256_file "$source")" || die "cannot hash target: $source"
        [[ "$actual" == "$digest" ]] || die "target SHA-256 differs: $source"
    done < "$TARGET_PINS"
}

audit_source() {
    audit_toolchain_source "$1" "$2" "$3"
    local kind mode size digest relative
    while IFS='|' read -r kind mode size digest relative; do
        [[ "$kind" == file ]] || continue
        [[ "$relative" == system-lib/* ]] || continue
        verify_file "$(system_source "$relative")" "$mode" "$size" "$digest" false
    done < "$MANIFEST"
    verify_interpreter
    report_success SOURCE
}

qualification_package_digest() {
    local record
    record="$(awk -F '\t' -v name="$1" '
        $1 == "package" && $2 == name {
            if (NF != 5 || $4 != "amd64" || seen++) exit 3;
            print $5
        }
        END { if (!seen) exit 4 }
    ' "$QUALIFICATION_PACKAGES")" || die "qualification package pin differs: $1"
    [[ "$record" =~ ^[0-9a-f]{64}$ ]] || die "malformed qualification package digest: $1"
    printf '%s\n' "$record"
}

verify_package_member() {
    local package=$1 member=$2 expected_size=$3 expected_digest=$4 size digest
    # Complete pipelines avoid extraction and propagate decoder/tar failures.
    # Package SHA-256 is checked before any archive is parsed.
    size="$(dpkg-deb --fsys-tarfile "$package" | tar -xOf - -- "$member" | wc -c)" \
        || die "cannot measure package member: $member"
    [[ "$size" == "$expected_size" ]] || die "package member size differs: $member"
    digest="$(dpkg-deb --fsys-tarfile "$package" | tar -xOf - -- "$member" | sha256sum | awk '{print $1}')" \
        || die "cannot hash package member: $member"
    [[ "$digest" == "$expected_digest" ]] || die "package member SHA-256 differs: $member"
}

audit_qualification_source() {
    local distribution=$1 toolchain=$2 inputs=$3 name package expected actual
    require_absolute_directory "$inputs" INPUTS
    for name in libc6 zlib1g; do
        case "$name" in
            libc6) package="$inputs/libc.deb" ;;
            zlib1g) package="$inputs/zlib.deb" ;;
        esac
        require_regular_source "$package"
        expected="$(qualification_package_digest "$name")" || die "cannot read qualification package pin: $name"
        actual="$(sha256_file "$package")" || die "cannot hash qualification package: $name"
        [[ "$actual" == "$expected" ]] || die "qualification package SHA-256 differs: $name"
    done
    audit_toolchain_source "$distribution" "$toolchain" "$inputs/rustup-init"

    local kind mode size digest relative member
    while IFS='|' read -r kind mode size digest relative; do
        [[ "$kind" == file ]] || continue
        case "$relative" in
            system-lib/libc.so.6|system-lib/libdl.so.2|system-lib/libm.so.6|system-lib/libpthread.so.0|system-lib/librt.so.1)
                package="$inputs/libc.deb"
                member="./usr/lib/x86_64-linux-gnu/${relative#system-lib/}"
                ;;
            system-lib/libz.so.1)
                package="$inputs/zlib.deb"
                member=./usr/lib/x86_64-linux-gnu/libz.so.1.3
                ;;
            system-lib/libgcc_s.so.1|system-lib/libstdc++.so.6)
                verify_file "$(system_source "$relative")" "$mode" "$size" "$digest" false
                continue
                ;;
            system-lib/*) die "unsupported qualification system DSO: $relative" ;;
            *) continue ;;
        esac
        verify_package_member "$package" "$member" "$size" "$digest"
    done < "$MANIFEST"

    local interpreter requested canonical
    interpreter="$(header_value interpreter)"
    IFS='|' read -r requested canonical size digest <<<"$interpreter"
    [[ "$requested" == /lib64/ld-linux-x86-64.so.2 \
        && "$canonical" == /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 ]] \
        || die "qualification interpreter layout differs"
    verify_package_member "$inputs/libc.deb" \
        ./usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 "$size" "$digest"
    # /usr/lib64 is rebuilt by the harness; /lib64 is inherited unchanged.
    [[ -L /lib64 && "$(readlink -- /lib64)" == usr/lib64 \
        && "$(stat -c '%u:%g' -- /lib64)" == 0:0 ]] || die "host /lib64 link differs"
    report_success QUALIFICATION_INPUTS
}

expected_inventory() {
    printf '%s|f\n' "$INSTALLED_MANIFEST_NAME"
    awk -F '|' '$1 == "directory" { print $3 "|d" } $1 == "file" { print $5 "|f" }' "$MANIFEST" \
        || die "cannot enumerate manifest inventory"
    while IFS=' ' read -r _ name; do
        printf '%s/%s|f\n' "$TARGET_PREFIX" "$name"
    done < "$TARGET_PINS"
}

audit_installed() {
    local root=$1
    require_absolute_directory "$root" RUNTIME_ROOT
    validate_pin_contract
    [[ "$(stat -Lc '%u:%g' -- "$root")" == 0:0 ]] || die "runtime root is not root-owned"
    [[ "$(stat -Lc %a -- "$root")" == 555 ]] || die "runtime root mode is not 0555"
    [[ -f "$root/$INSTALLED_MANIFEST_NAME" && ! -L "$root/$INSTALLED_MANIFEST_NAME" ]] \
        || die "installed manifest is not a no-follow regular file"
    cmp -s -- "$MANIFEST" "$root/$INSTALLED_MANIFEST_NAME" || die "installed manifest bytes differ"

    local temporary
    temporary="$(mktemp -d)"
    expected_inventory | LC_ALL=C sort > "$temporary/expected"
    find -P "$root" -mindepth 1 -printf '%P|%y\n' | LC_ALL=C sort > "$temporary/actual"
    cmp -s -- "$temporary/expected" "$temporary/actual" || {
        diff -u -- "$temporary/expected" "$temporary/actual" >&2 || true
        rm -rf -- "$temporary"
        die "installed closure inventory differs"
    }
    rm -rf -- "$temporary"

    local kind mode relative size digest actual
    while IFS='|' read -r kind mode relative; do
        [[ "$kind" == directory ]] || continue
        local path="$root/$relative"
        [[ -d "$path" && ! -L "$path" ]] || die "installed directory differs: $relative"
        [[ "$(stat -Lc %a -- "$path")" == "${mode#0}" ]] || die "installed directory mode differs: $relative"
        [[ "$(stat -Lc '%u:%g' -- "$path")" == 0:0 ]] || die "installed directory owner differs: $relative"
    done < "$MANIFEST"
    while IFS='|' read -r kind mode size digest relative; do
        [[ "$kind" == file ]] || continue
        verify_file "$root/$relative" "$mode" "$size" "$digest" true
    done < "$MANIFEST"
    while IFS=' ' read -r digest name; do
        local path="$root/$TARGET_PREFIX/$name"
        require_regular_source "$path"
        [[ "$(stat -Lc %a -- "$path")" == 444 ]] || die "target mode differs: $name"
        [[ "$(stat -Lc '%u:%g' -- "$path")" == 0:0 ]] || die "target owner differs: $name"
        actual="$(sha256_file "$path")" || die "cannot hash installed target: $name"
        [[ "$actual" == "$digest" ]] || die "target SHA-256 differs: $name"
    done < "$TARGET_PINS"
    actual="$(sha256_file "$MANIFEST")" || die "cannot hash manifest"
    verify_file "$root/$INSTALLED_MANIFEST_NAME" 0444 "$(wc -c < "$MANIFEST" | tr -d ' ')" "$actual" true
    verify_interpreter
    report_success INSTALLED "$root"
}

provision() {
    local distribution=$1
    local toolchain=$2
    local rustup=$3
    local destination=$4
    [[ "$EUID" == 0 ]] || die "provision must run as root"
    [[ "$destination" =~ ^/opt/fe2o3/verus-runtime-v2/[A-Za-z0-9][A-Za-z0-9._-]*$ ]] \
        || die "destination must be one canonical child of /opt/fe2o3/verus-runtime-v2"
    [[ ! -e "$destination" && ! -L "$destination" ]] || die "destination already exists"
    audit_source "$distribution" "$toolchain" "$rustup"

    umask 077
    install -d -o root -g root -m 0755 -- "$destination"
    local kind mode relative
    while IFS='|' read -r kind mode relative; do
        [[ "$kind" == directory ]] || continue
        install -d -o root -g root -m 0755 -- "$destination/$relative"
    done < "$MANIFEST"
    while IFS='|' read -r kind mode _ _ relative; do
        [[ "$kind" == file ]] || continue
        local source
        source="$(source_for_file "$relative" "$distribution" "$toolchain")"
        install -o root -g root -m "${mode#0}" -- "$source" "$destination/$relative"
    done < "$MANIFEST"
    while IFS=' ' read -r _ name; do
        install -o root -g root -m 0444 -- "$toolchain/$TARGET_SOURCE_PREFIX/$name" "$destination/$TARGET_PREFIX/$name"
    done < "$TARGET_PINS"
    install -o root -g root -m 0444 -- "$MANIFEST" "$destination/$INSTALLED_MANIFEST_NAME"
    while IFS='|' read -r kind mode relative; do
        [[ "$kind" == directory ]] || continue
        chmod "${mode#0}" -- "$destination/$relative"
    done < "$MANIFEST"
    chmod 0555 -- "$destination"
    audit_installed "$destination"
}

main() {
    case "${1:-}" in
        audit-qualification-source)
            [[ $# == 4 ]] || usage
            audit_qualification_source "$2" "$3" "$4"
            ;;
        audit-source)
            [[ $# == 4 ]] || usage
            audit_source "$2" "$3" "$4"
            ;;
        provision)
            [[ $# == 5 ]] || usage
            provision "$2" "$3" "$4" "$5"
            ;;
        audit-installed)
            [[ $# == 2 ]] || usage
            audit_installed "$2"
            ;;
        *) usage ;;
    esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
