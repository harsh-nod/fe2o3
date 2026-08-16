#!/usr/bin/env bash
set -euo pipefail

readonly PINNED_LLVM_VERSION='22.1.8'
readonly PINNED_LLVM_BUILD_ID='upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1'
readonly PINNED_LLVM_SOURCE_COMMIT='ca7933e47d3a3451d81e72ac174dcb5aa28b59d1'
readonly PINNED_LLVM_SOURCE_TREE='1e4fdb95266974a0cbca9ec4c6f740488322f238'
readonly PINNED_LLVM_CLOSURE_SHA256='8c1a9a78969b70a75b6eec6557b087ff3de586179a94ef41235574e8b4a24ed0'
readonly PINNED_LLVM_CLOSURE_LENGTH='331292'
readonly PINNED_LLVM_BUILD_ID_SHA256='5075b97ac8f762d9dd7364a1dcf21c8c310bcc4ca7153d3dd362d9bd9a952174'
readonly PINNED_LLVM_BUILD_ID_LENGTH='65'
readonly PINNED_LLD_ELF_SHA256='24ad3df8b0d1819494b4ba9f4cfc882eee0d84dea06a6d84d6778258d3caf597'
readonly PINNED_LLD_COMMON_SHA256='24bd6ce41c90e05d6a15840627dab6d89e2deebf8b44db0d1df7b0749b2cf2e5'

readonly CMAKE='/usr/bin/cmake'
readonly CMAKE_SHA256='1c5227af4edd22d8d689def545e18ee458260c0fd579eba2187967f38817e638'
readonly CMAKE_LENGTH='11796472'
readonly NINJA='/usr/bin/ninja'
readonly NINJA_SHA256='5965527e09fe2b3787772aa4f711d6a36b393e7f2fcaa744a7a96c5a4ddf59cb'
readonly NINJA_LENGTH='248040'
readonly CXX='/usr/bin/x86_64-linux-gnu-g++-13'
readonly CXX_SHA256='1353e9bdd29a7295c7226bf6c63abccce056d8cac31f112e5cdbecc3f28c2769'
readonly CXX_LENGTH='1027128'

usage() {
  printf 'usage: %s SOURCE_ROOT BUILD_DIR ARTIFACT_DIR LLVM_SOURCE_DIR LLVM_PACKAGE_ROOT LLVM_BUILD_ID_FILE LLVM_CLOSURE_MANIFEST\n' "$0" >&2
  exit 64
}

die() {
  printf 'fe2o3-static-host-lld-build: %s\n' "$*" >&2
  exit 1
}

sha256_file() {
  /usr/bin/sha256sum -- "$1" | /usr/bin/cut -d ' ' -f 1
}

file_length() {
  /usr/bin/stat -Lc '%s' -- "$1"
}

verify_file() {
  local path=$1 expected_sha256=$2 expected_length=$3 label=$4
  [[ -f "$path" && ! -L "$path" ]] || die "$label is not a regular non-symlink file"
  [[ $(file_length "$path") == "$expected_length" ]] ||
    die "$label length differs from its pin"
  [[ $(sha256_file "$path") == "$expected_sha256" ]] ||
    die "$label digest differs from its pin"
}

canonical_directory() {
  local path=$1 label=$2 canonical
  [[ "$path" == /* && -d "$path" && ! -L "$path" ]] ||
    die "$label must be an absolute non-symlink directory"
  canonical=$(/usr/bin/readlink -f -- "$path")
  [[ "$canonical" == "$path" ]] || die "$label is not canonical"
  printf '%s\n' "$canonical"
}

canonical_file() {
  local path=$1 label=$2 canonical
  [[ "$path" == /* && -f "$path" && ! -L "$path" ]] ||
    die "$label must be an absolute regular non-symlink file"
  canonical=$(/usr/bin/readlink -f -- "$path")
  [[ "$canonical" == "$path" ]] || die "$label is not canonical"
  printf '%s\n' "$canonical"
}

prepare_fresh_directory() {
  local path=$1 label=$2 parent base canonical_parent
  [[ "$path" == /* && "$path" != */ && ! -e "$path" && ! -L "$path" ]] ||
    die "$label must be an absolute nonexistent path"
  parent=$(/usr/bin/dirname -- "$path")
  base=$(/usr/bin/basename -- "$path")
  [[ "$base" != . && "$base" != .. ]] || die "$label has an invalid basename"
  canonical_parent=$(/usr/bin/readlink -f -- "$parent")
  [[ -d "$canonical_parent" && ! -L "$parent" &&
    "$canonical_parent/$base" == "$path" ]] ||
    die "$label parent or path is not canonical"
  /usr/bin/mkdir --mode=700 -- "$path"
  [[ -d "$path" && ! -L "$path" ]] || die "$label was replaced during creation"
  /usr/bin/stat -Lc '%d:%i' -- "$path"
}

directory_identity_matches() {
  local path=$1 expected=$2
  [[ -d "$path" && ! -L "$path" ]] &&
    [[ $(/usr/bin/stat -Lc '%d:%i' -- "$path") == "$expected" ]]
}

generate_tree_manifest() {
  local root=$1 output=$2
  {
    printf 'fe2o3-llvm-package-closure-v1\n'
    printf 'root=%s\n' "$root"
    while IFS= read -r -d '' path; do
      local relative target
      relative=${path#"$root"/}
      [[ "$relative" != *$'\n'* && "$relative" != *$'\r'* &&
        "$relative" != *$'\t'* ]] || die 'LLVM package has a noncanonical path'
      if [[ -L "$path" ]]; then
        target=$(/usr/bin/readlink -- "$path")
        [[ "$target" != *$'\n'* && "$target" != *$'\r'* &&
          "$target" != *$'\t'* ]] || die 'LLVM package has a noncanonical symlink'
        printf 'L\t%s\t%s\n' "$relative" "$target"
      else
        printf 'F\t%s\t%s\t%s\n' "$relative" \
          "$(file_length "$path")" "$(sha256_file "$path")"
      fi
    done < <(
      /usr/bin/find "$root" -mindepth 1 \( -type f -o -type l \) -print0 |
        /usr/bin/sort -z
    )
  } >"$output"
}

verify_llvm_closure() {
  local root=$1 expected=$2 observed=$3
  generate_tree_manifest "$root" "$observed"
  /usr/bin/cmp -s -- "$observed" "$expected" ||
    die 'LLVM package closure differs from the reviewed manifest'
}

generate_source_manifest() {
  local source_directory=$1 output=$2
  {
    printf 'fe2o3-host-lld-source-v1\n'
    while IFS= read -r -d '' path; do
      local relative
      relative=${path#"$source_directory"/}
      [[ -f "$path" && ! -L "$path" && "$relative" != *$'\n'* &&
        "$relative" != *$'\r'* && "$relative" != *$'\t'* ]] ||
        die 'host LLD source contains a noncanonical entry'
      printf 'F\t%s\t%s\t%s\n' "$relative" "$(file_length "$path")" \
        "$(sha256_file "$path")"
    done < <(/usr/bin/find "$source_directory" -mindepth 1 -type f -print0 |
      /usr/bin/sort -z)
  } >"$output"
}

generate_runtime_manifest() {
  local output=$1
  local name path
  {
    printf 'fe2o3-host-lld-static-runtime-v1\n'
    for name in crt1.o crti.o crtbeginT.o crtend.o crtn.o libstdc++.a \
        libgcc.a libgcc_eh.a libc.a libpthread.a libm.a libdl.a librt.a; do
      path=$(env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
        "$CXX" -print-file-name="$name")
      path=$(/usr/bin/readlink -f -- "$path")
      [[ "$path" == /* && -f "$path" && ! -L "$path" ]] ||
        die "required static runtime input is unavailable: $name ($path)"
      printf 'F\t%s\t%s\t%s\t%s\n' "$name" "$path" \
        "$(file_length "$path")" "$(sha256_file "$path")"
    done
  } >"$output"
}

inspect_static_elf() {
  local executable=$1 elf_report=$2 dynamic_report=$3
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    /usr/bin/readelf -hW -lW -SW --dyn-syms -- "$executable" >"$elf_report"
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    /usr/bin/readelf -dW -- "$executable" >"$dynamic_report"

  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "$elf_report" ||
    die 'host LLD is not ELF64'
  /usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "$elf_report" ||
    die 'host LLD is not ET_EXEC'
  /usr/bin/grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' \
    "$elf_report" || die 'host LLD has the wrong ELF machine'
  ! /usr/bin/grep -Eq '^[[:space:]]*INTERP[[:space:]]' "$elf_report" ||
    die 'host LLD has a PT_INTERP loader'
  ! /usr/bin/grep -Eq '^[[:space:]]*DYNAMIC[[:space:]]' "$elf_report" ||
    die 'host LLD has a PT_DYNAMIC segment'
  ! /usr/bin/awk '$1 == "LOAD" && $0 ~ /W/ && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'host LLD has a writable executable LOAD segment'
  ! /usr/bin/awk '$1 == "GNU_STACK" && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'host LLD has an executable stack'
  /usr/bin/grep -Fq 'There is no dynamic section in this file.' \
    "$dynamic_report" || die 'host LLD has a dynamic section'
  ! /usr/bin/grep -Eq '\.(dynamic|dynsym|dynstr)[[:space:]]' "$elf_report" ||
    die 'host LLD has a dynamic ELF section'
  ! /usr/bin/grep -Eq '\((NEEDED|RPATH|RUNPATH)\)' "$dynamic_report" ||
    die 'host LLD has dynamic dependencies or search paths'
  ! env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin /usr/bin/strings -a \
    "$executable" | /usr/bin/grep -Eqi 'amd_comgr|libamd_comgr|comgr\.h' ||
    die 'host LLD contains a COMGR reference'
}

[[ $# -eq 7 ]] || usage

source_root=$(canonical_directory "$1" 'source root')
build_dir=$2
artifact_dir=$3
llvm_source=$(canonical_directory "$4" 'LLVM source')
llvm_root=$(canonical_directory "$5" 'LLVM package root')
llvm_build_id_file=$(canonical_file "$6" 'LLVM build-ID file')
llvm_closure_manifest=$(canonical_file "$7" 'LLVM closure manifest')
readonly source_root build_dir artifact_dir llvm_source llvm_root
readonly llvm_build_id_file llvm_closure_manifest
readonly tool_source="$source_root/tools/fe2o3-host-lld"

[[ -d "$tool_source" && ! -L "$tool_source" ]] ||
  die 'source root does not contain tools/fe2o3-host-lld'
verify_file "$CMAKE" "$CMAKE_SHA256" "$CMAKE_LENGTH" 'CMake'
verify_file "$NINJA" "$NINJA_SHA256" "$NINJA_LENGTH" 'Ninja'
verify_file "$CXX" "$CXX_SHA256" "$CXX_LENGTH" 'C++ compiler'
verify_file "$llvm_build_id_file" "$PINNED_LLVM_BUILD_ID_SHA256" \
  "$PINNED_LLVM_BUILD_ID_LENGTH" 'LLVM build-ID file'
[[ $(/usr/bin/tr -d '\n' <"$llvm_build_id_file") == "$PINNED_LLVM_BUILD_ID" ]] ||
  die 'LLVM build-ID file content differs from its pin'
verify_file "$llvm_closure_manifest" "$PINNED_LLVM_CLOSURE_SHA256" \
  "$PINNED_LLVM_CLOSURE_LENGTH" 'LLVM closure manifest'

observed_llvm_commit=$(/usr/bin/git -C "$llvm_source" rev-parse HEAD)
readonly observed_llvm_commit
[[ "$observed_llvm_commit" == "$PINNED_LLVM_SOURCE_COMMIT" ]] ||
  die 'LLVM source commit differs from its pin'
observed_llvm_tree=$(/usr/bin/git -C "$llvm_source" rev-parse 'HEAD^{tree}')
readonly observed_llvm_tree
[[ "$observed_llvm_tree" == "$PINNED_LLVM_SOURCE_TREE" ]] ||
  die 'LLVM source tree differs from its pin'
[[ -z $(/usr/bin/git -C "$llvm_source" status --short --untracked-files=no) ]] ||
  die 'LLVM source has tracked modifications'

build_identity=$(prepare_fresh_directory "$build_dir" 'build directory')
readonly build_identity
artifact_identity=$(prepare_fresh_directory "$artifact_dir" 'artifact directory')
readonly artifact_identity
readonly llvm_before="$build_dir/llvm-closure-before.txt"
readonly llvm_after="$build_dir/llvm-closure-after.txt"
readonly source_before="$build_dir/source-before.txt"
readonly source_after="$build_dir/source-after.txt"
readonly runtime_manifest="$artifact_dir/fe2o3-host-lld.static-runtime-manifest.txt"

verify_llvm_closure "$llvm_root" "$llvm_closure_manifest" "$llvm_before"
generate_source_manifest "$tool_source" "$source_before"
generate_runtime_manifest "$runtime_manifest"
verify_file "$llvm_root/lib/liblldELF.a" "$PINNED_LLD_ELF_SHA256" '6796738' \
  'pinned liblldELF.a'
verify_file "$llvm_root/lib/liblldCommon.a" "$PINNED_LLD_COMMON_SHA256" '377796' \
  'pinned liblldCommon.a'

env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin "$CMAKE" -S "$tool_source" -B "$build_dir" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_MAKE_PROGRAM="$NINJA" \
  -DCMAKE_CXX_COMPILER="$CXX" \
  -DLLVM_DIR="$llvm_root/lib/cmake/llvm" \
  -DLLD_DIR="$llvm_root/lib/cmake/lld" \
  -DFE2O3_LLVM_PACKAGE_ROOT="$llvm_root" \
  -DFE2O3_LLVM_BUILD_ID_FILE="$llvm_build_id_file" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="$PINNED_LLVM_BUILD_ID" \
  -DFE2O3_EXPECTED_LLVM_SOURCE_COMMIT="$PINNED_LLVM_SOURCE_COMMIT" \
  -DFE2O3_EXPECTED_LLVM_SOURCE_TREE="$PINNED_LLVM_SOURCE_TREE"
env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin "$CMAKE" --build "$build_dir" --target fe2o3-host-lld \
  --parallel 16

directory_identity_matches "$build_dir" "$build_identity" ||
  die 'build directory identity changed during compilation'
directory_identity_matches "$artifact_dir" "$artifact_identity" ||
  die 'artifact directory identity changed during compilation'

readonly built_tool="$build_dir/fe2o3-host-lld"
readonly artifact_tool="$artifact_dir/fe2o3-host-lld"
[[ -f "$built_tool" && ! -L "$built_tool" ]] || die 'build did not produce host LLD'
/usr/bin/install --mode=0555 -- "$built_tool" "$artifact_tool"
[[ -f "$artifact_tool" && ! -L "$artifact_tool" ]] ||
  die 'artifact host LLD was replaced during installation'

readonly readelf_output="$artifact_dir/fe2o3-host-lld.readelf.txt"
readonly dynamic_output="$artifact_dir/fe2o3-host-lld.dynamic.txt"
readonly identity_output="$artifact_dir/fe2o3-host-lld.identity.txt"
readonly identity_stderr="$artifact_dir/fe2o3-host-lld.identity.stderr"
inspect_static_elf "$artifact_tool" "$readelf_output" "$dynamic_output"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 "$artifact_tool" \
  --fe2o3-identity-v1 >"$identity_output" 2>"$identity_stderr"
[[ ! -s "$identity_stderr" ]] ||
  die 'identity command wrote stderr'

generate_tree_manifest "$llvm_root" "$llvm_after"
/usr/bin/cmp -s -- "$llvm_after" "$llvm_closure_manifest" ||
  die 'LLVM package closure changed during compilation'
generate_source_manifest "$tool_source" "$source_after"
/usr/bin/cmp -s -- "$source_before" "$source_after" ||
  die 'host LLD source changed during compilation'
verify_file "$CMAKE" "$CMAKE_SHA256" "$CMAKE_LENGTH" 'CMake after build'
verify_file "$NINJA" "$NINJA_SHA256" "$NINJA_LENGTH" 'Ninja after build'
verify_file "$CXX" "$CXX_SHA256" "$CXX_LENGTH" 'C++ compiler after build'
verify_file "$llvm_root/lib/liblldELF.a" "$PINNED_LLD_ELF_SHA256" '6796738' \
  'pinned liblldELF.a after build'
verify_file "$llvm_root/lib/liblldCommon.a" "$PINNED_LLD_COMMON_SHA256" '377796' \
  'pinned liblldCommon.a after build'

/usr/bin/install --mode=0444 -- "$source_before" \
  "$artifact_dir/fe2o3-host-lld.source-manifest.txt"
readonly source_manifest="$artifact_dir/fe2o3-host-lld.source-manifest.txt"
readonly artifact_manifest="$artifact_dir/fe2o3-host-lld.artifact-manifest.txt"
runtime_entries=$(/usr/bin/awk -F '\t' '$1 == "F" { count += 1 } END { print count + 0 }' \
  "$runtime_manifest")
readonly runtime_entries
{
  printf 'FORMAT=fe2o3-host-lld-artifact-v1\n'
  printf 'STATUS=measured-no-authority\n'
  printf 'TOOL_BASENAME=fe2o3-host-lld\n'
  printf 'TOOL_SHA256=%s\n' "$(sha256_file "$artifact_tool")"
  printf 'TOOL_LENGTH=%s\n' "$(file_length "$artifact_tool")"
  printf 'TOOL_MODE=%s\n' "$(/usr/bin/stat -Lc '%a' -- "$artifact_tool")"
  printf 'ELF_CLASS=ELF64\n'
  printf 'ELF_MACHINE=Advanced_Micro_Devices_X86-64\n'
  printf 'ELF_TYPE=EXEC\n'
  printf 'ELF_DYNAMIC_SECTION=absent\n'
  printf 'ELF_PT_INTERP=absent\n'
  printf 'ELF_DT_NEEDED=absent\n'
  printf 'ELF_RPATH_RUNPATH=absent\n'
  printf 'ELF_WX_LOAD=absent\n'
  printf 'ELF_EXECUTABLE_STACK=absent\n'
  printf 'SUPPORTED_FLAVOR=gnu-elf\n'
  printf 'SUPPORTED_PROTOCOL=fe2o3-host-lld-elf-v1\n'
  printf 'OUTPUT_STAGING_PROTOCOL=retained-private-directory-v1\n'
  printf 'OUTPUT_BASENAME=fe2o3-host-output\n'
  printf 'LLVM_VERSION=%s\n' "$PINNED_LLVM_VERSION"
  printf 'LLVM_BUILD_ID=%s\n' "$PINNED_LLVM_BUILD_ID"
  printf 'LLVM_SOURCE_COMMIT=%s\n' "$PINNED_LLVM_SOURCE_COMMIT"
  printf 'LLVM_SOURCE_TREE=%s\n' "$PINNED_LLVM_SOURCE_TREE"
  printf 'LLVM_PACKAGE_CLOSURE_SHA256=%s\n' "$PINNED_LLVM_CLOSURE_SHA256"
  printf 'LLD_ELF_SHA256=%s\n' "$PINNED_LLD_ELF_SHA256"
  printf 'LLD_COMMON_SHA256=%s\n' "$PINNED_LLD_COMMON_SHA256"
  printf 'CXX_SHA256=%s\n' "$CXX_SHA256"
  printf 'SOURCE_MANIFEST_SHA256=%s\n' "$(sha256_file "$source_manifest")"
  printf 'SOURCE_MANIFEST_LENGTH=%s\n' "$(file_length "$source_manifest")"
  printf 'STATIC_RUNTIME_MANIFEST_SHA256=%s\n' "$(sha256_file "$runtime_manifest")"
  printf 'STATIC_RUNTIME_MANIFEST_LENGTH=%s\n' "$(file_length "$runtime_manifest")"
  printf 'STATIC_RUNTIME_ENTRIES=%s\n' "$runtime_entries"
  printf 'IDENTITY_REPORT_SHA256=%s\n' "$(sha256_file "$identity_output")"
  printf 'IDENTITY_REPORT_LENGTH=%s\n' "$(file_length "$identity_output")"
  printf 'AUTHORITY=none\n'
  printf 'BROKER_IDENTITY=not_constructed\n'
  printf 'ARTIFACT_HANDOFF=not_constructed\n'
  printf 'GPU_LINKER=unchanged\n'
  printf 'COMGR=absent\n'
} >"$artifact_manifest"
/usr/bin/chmod 0444 "$artifact_manifest" "$identity_output" "$readelf_output" \
  "$dynamic_output" "$runtime_manifest" "$source_manifest" "$identity_stderr"

directory_identity_matches "$artifact_dir" "$artifact_identity" ||
  die 'artifact directory identity changed before publication'
verify_file "$artifact_tool" "$(sha256_file "$artifact_tool")" \
  "$(file_length "$artifact_tool")" 'final host LLD artifact'
printf 'fe2o3 static host LLD build passed: %s\n' "$artifact_manifest"
