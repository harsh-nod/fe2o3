#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s SOURCE_ROOT WORK_ROOT LLVM_SOURCE_DIR LLVM_PACKAGE_ROOT LLVM_BUILD_ID_FILE LLVM_CLOSURE_MANIFEST\n' "$0" >&2
  exit 64
}

die() {
  printf 'fe2o3-static-host-lld-test: %s\n' "$*" >&2
  exit 1
}

sha256_file() {
  /usr/bin/sha256sum -- "$1" | /usr/bin/cut -d ' ' -f 1
}

assert_static_elf() {
  local executable=$1 prefix=$2
  local readelf_report="$work_root/$prefix.readelf"
  local llvm_report="$work_root/$prefix.llvm-readobj"
  local file_report="$work_root/$prefix.file"

  /usr/bin/readelf -hW -lW -dW -SW --dyn-syms -- "$executable" \
    >"$readelf_report"
  "$llvm_root/bin/llvm-readobj" --file-headers --program-headers \
    --dynamic-table --needed-libs "$executable" >"$llvm_report"
  /usr/bin/file -- "$executable" >"$file_report"

  /usr/bin/grep -Fq 'statically linked' "$file_report" ||
    die "$prefix is not reported as statically linked"
  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "$readelf_report" ||
    die "$prefix is not ELF64"
  /usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "$readelf_report" ||
    die "$prefix is not ET_EXEC"
  ! /usr/bin/grep -Eq '^[[:space:]]*(INTERP|DYNAMIC)[[:space:]]' \
    "$readelf_report" || die "$prefix has a loader or dynamic segment"
  ! /usr/bin/grep -Eq '\((NEEDED|RPATH|RUNPATH)\)' "$readelf_report" ||
    die "$prefix has a dynamic dependency or search path"
  ! /usr/bin/awk '$1 == "LOAD" && $0 ~ /W/ && $0 ~ /E/ { bad = 1 }
    END { exit bad ? 0 : 1 }' "$readelf_report" ||
    die "$prefix has a writable executable LOAD segment"
  ! /usr/bin/awk '$1 == "GNU_STACK" && $0 ~ /E/ { bad = 1 }
    END { exit bad ? 0 : 1 }' "$readelf_report" ||
    die "$prefix has an executable stack"
  ! /usr/bin/strings -a "$executable" | /usr/bin/grep -Eqi \
    '(^|[^[:alnum:]_])(amd_)?comgr([^[:alnum:]_]|$)' ||
    die "$prefix contains a COMGR reference"
}

[[ $# -eq 6 ]] || usage
source_root=$(/usr/bin/readlink -f -- "$1")
work_root=$2
llvm_source=$(/usr/bin/readlink -f -- "$3")
llvm_root=$(/usr/bin/readlink -f -- "$4")
llvm_build_id_file=$(/usr/bin/readlink -f -- "$5")
llvm_closure_manifest=$(/usr/bin/readlink -f -- "$6")
readonly source_root work_root llvm_source llvm_root llvm_build_id_file
readonly llvm_closure_manifest
readonly build_script="$source_root/scripts/fe2o3-static-host-lld-build.sh"
readonly secure_suite="$source_root/tools/fe2o3-host-lld/tests/secure_protocol.sh"

[[ "$1" == "$source_root" && -d "$source_root" && ! -L "$1" ]] ||
  die 'source root must be a canonical non-symlink directory'
[[ -x "$build_script" && -x "$secure_suite" ]] ||
  die 'source root omits an executable build or secure-v2 test driver'
[[ "$work_root" == /* && ! -e "$work_root" && ! -L "$work_root" ]] ||
  die 'work root must be an absolute nonexistent path'
work_parent=$(/usr/bin/dirname -- "$work_root")
readonly work_parent
[[ -d "$work_parent" && ! -L "$work_parent" ]] ||
  die 'work root parent must be a non-symlink directory'
/usr/bin/mkdir --mode=700 -- "$work_root"
work_identity=$(/usr/bin/stat -Lc '%d:%i' -- "$work_root")
readonly work_identity

"$build_script" "$source_root" "$work_root/build-a" \
  "$work_root/artifact-a" "$llvm_source" "$llvm_root" \
  "$llvm_build_id_file" "$llvm_closure_manifest"
"$build_script" "$source_root" "$work_root/build-b" \
  "$work_root/artifact-b" "$llvm_source" "$llvm_root" \
  "$llvm_build_id_file" "$llvm_closure_manifest"

for artifact in fe2o3-host-lld fe2o3-host-lld.artifact-manifest.txt \
    fe2o3-host-lld.identity.txt fe2o3-host-lld.source-manifest.txt \
    fe2o3-host-lld.static-runtime-manifest.txt fe2o3-host-lld.readelf.txt \
    fe2o3-host-lld.dynamic.txt; do
  /usr/bin/cmp -s -- "$work_root/artifact-a/$artifact" \
    "$work_root/artifact-b/$artifact" ||
    die "two fresh repeated builds differ for $artifact"
done

tool_a="$work_root/artifact-a/fe2o3-host-lld"
tool_b="$work_root/artifact-b/fe2o3-host-lld"
manifest="$work_root/artifact-a/fe2o3-host-lld.artifact-manifest.txt"
readonly tool_a tool_b manifest
tool_sha256=$(sha256_file "$tool_a")
tool_length=$(/usr/bin/stat -Lc '%s' -- "$tool_a")
readonly tool_sha256 tool_length
[[ $(/usr/bin/sed -n 's/^TOOL_SHA256=//p' "$manifest") == "$tool_sha256" ]] ||
  die 'artifact manifest tool digest is not exact'
[[ $(/usr/bin/sed -n 's/^TOOL_LENGTH=//p' "$manifest") == "$tool_length" ]] ||
  die 'artifact manifest tool length is not exact'
[[ $(/usr/bin/sed -n 's/^TOOL_MODE=//p' "$manifest") == 555 ]] ||
  die 'artifact manifest mode is not exact'
[[ $(/usr/bin/sed -n 's/^AUTHORITY=//p' "$manifest") == none ]] ||
  die 'artifact manifest makes an authority claim'
[[ $(/usr/bin/sed -n 's/^GPU_LINKER=//p' "$manifest") == unchanged ]] ||
  die 'artifact manifest changes the GPU linker boundary'

assert_static_elf "$tool_a" build-a
assert_static_elf "$tool_b" build-b

identity="$work_root/artifact-a/fe2o3-host-lld.identity.txt"
readonly identity
for field in protocol=fe2o3-host-lld-elf-v2 \
    output_staging=tool-owned-sealed-memfd-v1 \
    result_copy=receiver-owned-memfd-v1 \
    max_argument_count=4096 max_argument_bytes=4096 \
    max_total_argument_bytes=1048576 max_input_count=2048 \
    max_input_bytes=268435456 max_total_input_bytes=2147483648 \
    max_output_bytes=536870912 max_address_space_bytes=4294967296 \
    max_archive_members=262144 max_cpu_seconds=60 \
    dependent_libraries=forbidden \
    signal_state=linux-x86_64-kernel-1-64-main-v2; do
  /usr/bin/grep -Fxq "$field" "$identity" ||
    die "tool identity omitted $field"
done

"$secure_suite" "$source_root/tools/fe2o3-host-lld" "$tool_a" \
  "$work_root/secure-a"
"$secure_suite" "$source_root/tools/fe2o3-host-lld" "$tool_b" \
  "$work_root/secure-b"

/usr/bin/cmp -s -- "$work_root/secure-a/out-minimal-a" \
  "$work_root/secure-b/out-minimal-a" ||
  die 'fresh repeated secure-v2 links are not byte-identical'
/usr/bin/cmp -s -- "$work_root/secure-a/out-rust" \
  "$work_root/secure-b/out-rust" ||
  die 'fresh repeated Rust rlib extraction outputs are not byte-identical'

! /usr/bin/grep -Eq \
  '(^|[^[:alnum:]_])(system|popen|posix_spawn|fork|execv)[[:space:]]*\(' \
  "$source_root/tools/fe2o3-host-lld/src/main.cpp" ||
  die 'host LLD source contains a subprocess invocation'
[[ $(/usr/bin/stat -Lc '%d:%i' -- "$work_root") == "$work_identity" ]] ||
  die 'test work root identity changed'

printf 'static host LLD release tests passed\n'
printf 'tool_sha256=%s\n' "$tool_sha256"
printf 'tool_length=%s\n' "$tool_length"
printf 'artifact_manifest_sha256=%s\n' "$(sha256_file "$manifest")"
printf 'secure_a_trace=%s\n' "$work_root/secure-a"
printf 'secure_b_trace=%s\n' "$work_root/secure-b"
printf 'work_root=%s\n' "$work_root"
