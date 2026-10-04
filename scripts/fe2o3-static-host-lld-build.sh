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
readonly PINNED_IDENTITY_FORMAT='fe2o3-host-lld-identity-v1'
readonly PINNED_LINK_PROTOCOL='fe2o3-host-lld-elf-v2'
readonly PINNED_INPUT_PROTOCOL='fe2o3-input-v1'
readonly PINNED_RESULT_PROTOCOL='fe2o3-host-lld-result-v1'
readonly PINNED_RESULT_SOCKET_FD='91'
readonly PINNED_OUTPUT_STAGING='tool-owned-sealed-memfd-v1'
readonly PINNED_GUARD_SOURCE_SHA256='2491f56187cfdeb742ca646778f687277adaa7f54d8f85352e601fcaafb664a4'
readonly PINNED_GUARD_SOURCE_LENGTH='73769'
readonly PINNED_GUARD_TEST_SHA256='46dbc02887abe1299a7367fd440c6d058631814a54ffaf22315eee741b179840'
readonly PINNED_GUARD_TEST_LENGTH='49136'
readonly PINNED_BOOTSTRAP_HELPER_SHA256='071122a9ad655bc5f6a2f2b58539e6f73ea3c1ff0163d6c245d84a274a57226d'
readonly PINNED_BOOTSTRAP_HELPER_LENGTH='9557'
readonly PINNED_TRACE_CHECK_SOURCE_SHA256='caa96417da7eeb485591ddb95ff5a367ca76991df9bfd675f7b613de04388b77'
readonly PINNED_TRACE_CHECK_SOURCE_LENGTH='68692'
readonly PINNED_TMP_REDIRECT_SOURCE_SHA256='5445cd7978698e794afc08be1eb916c3ab321c0041f682832820f474ef634df4'
readonly PINNED_TMP_REDIRECT_SOURCE_LENGTH='4672'
readonly PINNED_BUILD_INPUTS_SHA256='61d47f1dbace3fcc9e1f756f11f6319bce74edf438153bb63c4202b1a465dce0'
readonly PINNED_BUILD_INPUTS_LENGTH='13148'
readonly PINNED_ROOTS_SHA256='39f369a8cfca14e9e6fd2a14a66c1d308e9c89e492de04fd7ab94573753f16d8'
readonly PINNED_ROOTS_LENGTH='1998'
readonly PINNED_RUNTIME_SHA256='3f70263ff19198bf5a79fc9352157752e374923746cbfb05210907ee4b5517ef'
readonly PINNED_RUNTIME_LENGTH='1806'
readonly PINNED_CTEST_POLICY_SHA256='911f9279a6c974de3492a0aad176871a59c7590770072530c8cf2d0b314b70b8'
readonly PINNED_CTEST_POLICY_LENGTH='4963'

readonly CMAKE='/usr/bin/cmake'
readonly NINJA='/usr/bin/ninja'
readonly CXX='/usr/bin/x86_64-linux-gnu-g++-13'
readonly READELF='/usr/bin/x86_64-linux-gnu-readelf'
readonly STRINGS='/usr/bin/x86_64-linux-gnu-strings'
readonly AWK='/usr/bin/gawk'
readonly STRACE='/usr/bin/strace'
readonly RAW_TRACE_GLOBAL_FILE_BOUND='65536'
readonly RAW_TRACE_GLOBAL_BYTE_BOUND='268435456'

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
  local expected_mode=${5:-}
  [[ -f "$path" && ! -L "$path" ]] || die "$label is not a regular non-symlink file"
  [[ $(file_length "$path") == "$expected_length" ]] ||
    die "$label length differs from its pin"
  [[ $(sha256_file "$path") == "$expected_sha256" ]] ||
    die "$label digest differs from its pin"
  [[ -z "$expected_mode" || $(/usr/bin/stat -Lc '%a' -- "$path") == "$expected_mode" ]] ||
    die "$label mode differs from its pin"
}

verify_pin_file() {
  local path=$1 expected_sha256=$2 expected_length=$3 label=$4
  verify_file "$path" "$expected_sha256" "$expected_length" "$label" 644
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

verify_reviewed_source_manifest() {
  local manifest=$1 phase=$2
  local observed
  observed="$(sha256_file "$manifest"):$(file_length "$manifest")"
  [[ "$observed" == "$TOOL_SOURCE_MANIFEST_SHA256:$TOOL_SOURCE_MANIFEST_LENGTH" ]] ||
    die "host LLD source differs from its reviewed pin during $phase ($observed)"
}

load_tool_source_pin() {
  local pin=$1 key value extra index=0
  local -a expected=(
    FORMAT STATUS ORIGIN_COMMIT ORIGIN_TREE SOURCE_MANIFEST_SHA256
    SOURCE_MANIFEST_LENGTH GUARD_ROOT_MANIFEST_SHA256 GUARD_ROOT_MANIFEST_LENGTH
  )
  while IFS='=' read -r key value extra; do
    [[ $index -lt ${#expected[@]} && "$key" == "${expected[index]}" &&
      -n "$value" && -z "$extra" ]] || die 'tool-source pin is noncanonical'
    case "$key" in
      FORMAT)
        [[ "$value" == fe2o3-static-host-lld-tool-source-pin-v1 ]] ||
          die 'tool-source pin format is unsupported'
        ;;
      STATUS)
        [[ "$value" == development-only-not-accepted ||
          "$value" == reviewed-accepted ]] || die 'tool-source pin status is invalid'
        TOOL_SOURCE_PIN_STATUS=$value
        ;;
      ORIGIN_COMMIT)
        [[ "$value" =~ ^[0-9a-f]{40}$ ]] || die 'tool-source commit is invalid'
        TOOL_SOURCE_ORIGIN_COMMIT=$value
        ;;
      ORIGIN_TREE)
        [[ "$value" =~ ^[0-9a-f]{40}$ ]] || die 'tool-source tree is invalid'
        TOOL_SOURCE_ORIGIN_TREE=$value
        ;;
      SOURCE_MANIFEST_SHA256)
        [[ "$value" =~ ^[0-9a-f]{64}$ ]] || die 'tool source digest is invalid'
        TOOL_SOURCE_MANIFEST_SHA256=$value
        ;;
      SOURCE_MANIFEST_LENGTH)
        [[ "$value" =~ ^[0-9]+$ ]] || die 'tool source length is invalid'
        TOOL_SOURCE_MANIFEST_LENGTH=$value
        ;;
      GUARD_ROOT_MANIFEST_SHA256)
        [[ "$value" =~ ^[0-9a-f]{64}$ ]] || die 'tool root digest is invalid'
        TOOL_SOURCE_ROOT_SHA256=$value
        ;;
      GUARD_ROOT_MANIFEST_LENGTH)
        [[ "$value" =~ ^[0-9]+$ ]] || die 'tool root length is invalid'
        TOOL_SOURCE_ROOT_LENGTH=$value
        ;;
    esac
    ((index += 1))
  done <"$pin"
  [[ $index -eq ${#expected[@]} ]] || die 'tool-source pin is incomplete'
  readonly TOOL_SOURCE_PIN_STATUS TOOL_SOURCE_ORIGIN_COMMIT TOOL_SOURCE_ORIGIN_TREE
  readonly TOOL_SOURCE_MANIFEST_SHA256 TOOL_SOURCE_MANIFEST_LENGTH
  readonly TOOL_SOURCE_ROOT_SHA256 TOOL_SOURCE_ROOT_LENGTH
}

write_expected_identity_report() {
  local output=$1
  {
    printf 'format=%s\n' "$PINNED_IDENTITY_FORMAT"
    printf 'authority=none\n'
    printf 'flavor=gnu-elf\n'
    printf 'protocol=%s\n' "$PINNED_LINK_PROTOCOL"
    printf 'input_protocol=%s\n' "$PINNED_INPUT_PROTOCOL"
    printf 'result_protocol=%s\n' "$PINNED_RESULT_PROTOCOL"
    printf 'result_socket_fd=%s\n' "$PINNED_RESULT_SOCKET_FD"
    printf 'output_staging=%s\n' "$PINNED_OUTPUT_STAGING"
    printf 'result_copy=receiver-owned-memfd-v1\n'
    printf 'max_argument_count=4096\n'
    printf 'max_argument_bytes=4096\n'
    printf 'max_total_argument_bytes=1048576\n'
    printf 'max_input_count=2048\n'
    printf 'max_input_bytes=268435456\n'
    printf 'max_total_input_bytes=2147483648\n'
    printf 'max_output_bytes=536870912\n'
    printf 'max_address_space_bytes=4294967296\n'
    printf 'max_archive_members=262144\n'
    printf 'max_cpu_seconds=60\n'
    printf 'dependent_libraries=forbidden\n'
    printf 'signal_state=linux-x86_64-kernel-1-64-main-v2\n'
    printf 'llvm_version=%s\n' "$PINNED_LLVM_VERSION"
    printf 'llvm_build_identity=%s\n' "$PINNED_LLVM_BUILD_ID"
    printf 'llvm_source_commit=%s\n' "$PINNED_LLVM_SOURCE_COMMIT"
    printf 'llvm_source_tree=%s\n' "$PINNED_LLVM_SOURCE_TREE"
    printf 'elf_class=ELF64\n'
    printf 'elf_machine=Advanced Micro Devices X86-64\n'
  } >"$output"
}

identity_report_matches() {
  local report=$1 expected_report=$2
  [[ -f "$report" && ! -L "$report" ]] &&
    /usr/bin/cmp -s -- "$report" "$expected_report"
}

identity_value() {
  local report=$1 key=$2
  # shellcheck disable=SC2016
  "$AWK" -F = -v key="$key" '$1 == key { print substr($0, length($1) + 2) }' \
    "$report"
}

append_retained_guard_file() {
  local label=$1 path=$2 fields digest length mode
  fields=$(fe2o3_bootstrap_file_fields "$path") || die "$FE2O3_BOOTSTRAP_ERROR"
  IFS=$'\t' read -r digest length mode <<<"$fields"
  guard_file_args+=(--file "$label" "$path" "$digest" "$length" "$mode")
}

write_trace_allowlist() {
  local output=$1 index
  # shellcheck disable=SC2016
  {
    printf 'FORMAT=fe2o3-static-host-lld-trace-allowlist-v1\n'
    for index in "${!FE2O3_BOOTSTRAP_FILE_PATHS[@]}"; do
      printf 'F\t%s\t%s\n' "${FE2O3_BOOTSTRAP_FILE_LABELS[index]}" \
        "${FE2O3_BOOTSTRAP_FILE_PATHS[index]}"
    done
    for index in "${!admission_root_paths[@]}"; do
      printf 'R\t%s\t%s\n' "${admission_root_labels[index]}" \
        "${admission_root_paths[index]}"
    done
    for index in "${!absence_root_paths[@]}"; do
      printf 'K\tabsence-root-metadata-%s\t%s\n' \
        "${absence_root_labels[index]}" "${absence_root_paths[index]}"
      printf 'N\t%s\t%s\n' "${absence_root_labels[index]}" \
        "${absence_root_paths[index]}"
    done
    printf 'P\tgcc-parent-search\t/usr/lib/gcc/x86_64-linux-gnu\n'
    printf 'P\tgcc-libexec-parent-search\t/usr/libexec/gcc/x86_64-linux-gnu\n'
    printf 'P\tsystem-lib-version-search\t/usr/lib/x86_64-linux-gnu/13\n'
    printf 'P\tcross-prefix-search\t/usr/x86_64-linux-gnu\n'
    printf 'K\tdev-null\t/dev/null\n'
    printf 'K\tdev-urandom\t/dev/urandom\n'
    printf 'K\tabsent-ld-so-preload\t/etc/ld.so.preload\n'
    printf 'K\tabsent-arch-release\t/etc/arch-release\n'
    printf 'K\tsystem-usr-metadata\t/usr\n'
    printf 'K\tsystem-lib-metadata\t/usr/lib\n'
    printf 'K\tsystem-target-lib-metadata\t/usr/lib/x86_64-linux-gnu\n'
    printf 'K\tetc-metadata\t/etc\n'
    printf 'K\tgcc-lib-metadata\t/usr/lib/gcc\n'
    printf 'K\tgcc-target-metadata\t/usr/lib/gcc/x86_64-linux-gnu\n'
    printf 'K\tlibexec-metadata\t/usr/libexec\n'
    printf 'K\tgcc-libexec-metadata\t/usr/libexec/gcc\n'
    printf 'K\tgcc-libexec-target-metadata\t/usr/libexec/gcc/x86_64-linux-gnu\n'
    printf 'K\tlocal-prefix-metadata\t/usr/local\n'
    printf 'K\tshare-prefix-metadata\t/usr/share\n'
    printf 'K\tllvm-system-metadata\t/usr/lib/llvm-18\n'
    printf 'K\tllvm-system-lib-metadata\t/usr/lib/llvm-18/lib\n'
    printf 'K\tclang-system-metadata\t/usr/lib/llvm-18/lib/clang\n'
    printf 'K\tclang-version-metadata\t/usr/lib/llvm-18/lib/clang/18\n'
    printf 'K\tsystem-home-metadata\t/home\n'
    printf 'K\tuser-home-metadata\t/home/harsh\n'
    for index in "${!llvm_parent_paths[@]}"; do
      printf 'K\t%s-metadata\t%s\n' "${llvm_parent_labels[index]}" \
        "${llvm_parent_paths[index]}"
    done
    printf 'K\tsource-parent-metadata\t%s\n' "${source_root%/*}"
    printf 'K\tsource-root-metadata\t%s\n' "$source_root"
    printf 'K\ttool-source-parent-metadata\t%s\n' "${tool_source%/*}"
    printf 'K\tllvm-source-root-metadata\t%s\n' "$llvm_source"
    printf 'K\tllvm-source-llvm-parent-metadata\t%s/llvm\n' "$llvm_source"
    printf 'K\tllvm-source-llvm-cmake-parent-metadata\t%s/llvm/cmake\n' \
      "$llvm_source"
    printf 'K\tllvm-source-lld-parent-metadata\t%s/lld\n' "$llvm_source"
    printf 'K\twork-parent-metadata\t%s\n' "${build_dir%/*}"
    printf 'K\tproc-cpuinfo\t/proc/cpuinfo\n'
    printf 'K\tproc-version-signature-denied\t/proc/version_signature\n'
    printf 'K\tproc-self-cgroup-denied\t/proc/self/cgroup\n'
    printf 'K\tproc-self-mountinfo-denied\t/proc/self/mountinfo\n'
    printf 'K\tproc-version-signature\t/proc/version_signature\n'
    printf 'K\tsys-cpu-online\t/sys/devices/system/cpu/online\n'
    printf 'K\tsys-cpu-possible\t/sys/devices/system/cpu/possible\n'
    printf 'O\tBUILD\t%s\n' "$build_dir"
    printf 'O\tARTIFACT\t%s\n' "$artifact_dir"
  } | "$AWK" -F $'\t' -v OFS=$'\t' '
    function has_label(value, wanted, count, items, position) {
      count = split(value, items, "+")
      for (position = 1; position <= count; ++position)
        if (items[position] == wanted)
          return 1
      return 0
    }
    $1 == "K" {
      if (!($3 in labels))
        labels[$3] = $2
      else if (!has_label(labels[$3], $2))
        labels[$3] = labels[$3] "+" $2
      next
    }
    { print }
    END {
      count = asorti(labels, paths)
      for (position = 1; position <= count; ++position)
        print "K", labels[paths[position]], paths[position]
    }
  ' >"$output"
}

load_build_input_pins() {
  local pin=$1 append_guard_args=$2 phase=$3
  local kind path mode length digest extra count=0 label canonical
  local -A seen=()

  while IFS=$'\t' read -r kind path mode length digest extra; do
    case "$kind" in
      'FORMAT=fe2o3-static-host-lld-build-input-pin-v1'|'TARGET=x86_64-linux-gnu-ubuntu-24.04')
        [[ -z "$path$mode$length$digest$extra" ]] ||
          die 'build-input pin header has extra fields'
        continue
        ;;
      F) ;;
      *) die "build-input pin has an unknown row: $kind" ;;
    esac
    [[ -z "$extra" && "$path" == /* && "$mode" =~ ^[0-7]{3,4}$ &&
      "$length" =~ ^[0-9]+$ && "$digest" =~ ^[0-9a-f]{64}$ ]] ||
      die 'build-input pin has a noncanonical file row'
    canonical=$(/usr/bin/readlink -f -- "$path")
    [[ "$canonical" == "$path" ]] ||
      die "build-input pin path is not canonical: $path"
    [[ -z ${seen["$path"]+present} ]] ||
      die "build-input pin repeats a path: $path"
    seen["$path"]=1
    verify_file "$path" "$digest" "$length" "build input during $phase: $path" \
      "$mode"
    if [[ "$append_guard_args" == yes ]]; then
      printf -v label 'build-input-%03d' "$count"
      fe2o3_bootstrap_retain_file "$label" "$path" "$digest" "$length" "$mode" ||
        die "$FE2O3_BOOTSTRAP_ERROR"
      guard_file_args+=(--file "$label" "$path" "$digest" "$length" "$mode")
    fi
    ((count += 1))
  done <"$pin"
  [[ $count -ge 80 ]] || die 'build-input pin is unexpectedly incomplete'
}

load_runtime_pins() {
  local pin=$1 append_guard_args=$2 phase=$3
  local kind name path mode length digest extra count=0 label resolved canonical
  local -A seen_names=() seen_paths=()

  while IFS=$'\t' read -r kind name path mode length digest extra; do
    case "$kind" in
      'FORMAT=fe2o3-static-host-lld-runtime-pin-v1'|'TARGET=x86_64-linux-gnu-ubuntu-24.04')
        [[ -z "$name$path$mode$length$digest$extra" ]] ||
          die 'runtime pin header has extra fields'
        continue
        ;;
      F) ;;
      *) die "runtime pin has an unknown row: $kind" ;;
    esac
    [[ -z "$extra" && "$name" =~ ^[A-Za-z0-9._+-]+$ && "$path" == /* &&
      "$mode" =~ ^[0-7]{3,4}$ && "$length" =~ ^[0-9]+$ &&
      "$digest" =~ ^[0-9a-f]{64}$ ]] ||
      die 'runtime pin has a noncanonical file row'
    [[ -z ${seen_names["$name"]+present} && -z ${seen_paths["$path"]+present} ]] ||
      die "runtime pin repeats a name or path: $name"
    seen_names["$name"]=1
    seen_paths["$path"]=1
    resolved=$(env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
      "$CXX" -print-file-name="$name")
    canonical=$(/usr/bin/readlink -f -- "$resolved")
    [[ "$canonical" == "$path" ]] ||
      die "runtime resolver differs from pin during $phase: $name"
    verify_file "$path" "$digest" "$length" "runtime during $phase: $name" \
      "$mode"
    if [[ "$append_guard_args" == yes ]]; then
      printf -v label 'runtime-input-%03d' "$count"
      fe2o3_bootstrap_retain_file "$label" "$path" "$digest" "$length" "$mode" ||
        die "$FE2O3_BOOTSTRAP_ERROR"
      guard_file_args+=(--file "$label" "$path" "$digest" "$length" "$mode")
      guard_runtime_args+=(--runtime "$name" "$path")
    fi
    ((count += 1))
  done <"$pin"
  [[ $count -eq 14 ]] || die 'runtime pin must contain exactly 14 inputs'
}

load_root_pins() {
  local pin=$1
  local kind label selector length digest extra path count=0
  local -A seen=()

  while IFS=$'\t' read -r kind label selector length digest extra; do
    case "$kind" in
      'FORMAT=fe2o3-static-host-lld-root-pin-v1'|'TARGET=x86_64-linux-gnu-ubuntu-24.04')
        [[ -z "$label$selector$length$digest$extra" ]] ||
          die 'root pin header has extra fields'
        continue
        ;;
      R|N) ;;
      *) die "root pin has an unknown row: $kind" ;;
    esac
    [[ -z "$extra" && "$label" =~ ^[A-Za-z0-9._-]+$ &&
      "$length" =~ ^[0-9]+$ && "$digest" =~ ^[0-9a-f]{64}$ ]] ||
      die 'root pin has a noncanonical row'
    case "$selector" in
      LLVM_PACKAGE_ROOT) path=$llvm_root ;;
      LLVM_SOURCE_LLVM_INCLUDE) path=$llvm_source/llvm/include ;;
      LLVM_SOURCE_LLD_INCLUDE) path=$llvm_source/lld/include ;;
      LLVM_SOURCE_LLVM_CMAKE_MODULES) path=$llvm_source/llvm/cmake/modules ;;
      TOOL_SOURCE) path=$tool_source ;;
      /*) path=$selector ;;
      *) die "root pin has an unknown selector: $selector" ;;
    esac
    path=$(canonical_directory "$path" "guarded root $label")
    [[ -z ${seen["$label"]+present} ]] || die "root pin repeats label: $label"
    seen["$label"]=1
    if [[ "$kind" == R ]]; then
      guard_root_args+=(--root "$label" "$path" "$digest" "$length")
      admission_root_labels+=("$label")
      admission_root_paths+=("$path")
    else
      guard_root_args+=(--absence-root "$label" "$path" "$digest" "$length")
      absence_root_labels+=("$label")
      absence_root_paths+=("$path")
    fi
    ((count += 1))
  done <"$pin"
  [[ $count -eq 17 ]] || die 'root pin must contain exactly 17 shared roots'
}

inspect_static_elf() {
  local executable=$1 elf_report=$2 dynamic_report=$3
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    "$READELF" -hW -lW -SW --dyn-syms -- "$executable" >"$elf_report"
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    "$READELF" -dW -- "$executable" >"$dynamic_report"

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
  # shellcheck disable=SC2016
  ! "$AWK" '$1 == "LOAD" && $0 ~ /W/ && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'host LLD has a writable executable LOAD segment'
  # shellcheck disable=SC2016
  ! "$AWK" '$1 == "GNU_STACK" && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'host LLD has an executable stack'
  /usr/bin/grep -Fq 'There is no dynamic section in this file.' \
    "$dynamic_report" || die 'host LLD has a dynamic section'
  ! /usr/bin/grep -Eq '\.(dynamic|dynsym|dynstr)[[:space:]]' "$elf_report" ||
    die 'host LLD has a dynamic ELF section'
  ! /usr/bin/grep -Eq '\((NEEDED|RPATH|RUNPATH)\)' "$dynamic_report" ||
    die 'host LLD has dynamic dependencies or search paths'
  ! env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin "$STRINGS" -a \
    "$executable" | /usr/bin/grep -Eqi 'amd_comgr|libamd_comgr|comgr\.h' ||
    die 'host LLD contains a COMGR reference'
}

inspect_tmp_redirect_elf() {
  local library=$1 elf_report=$2 dynamic_report=$3 symbol
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    "$READELF" -hW -lW -SW --dyn-syms -- "$library" >"$elf_report"
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
    "$READELF" -dW -- "$library" >"$dynamic_report"

  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "$elf_report" ||
    die 'temporary-path redirect is not ELF64'
  /usr/bin/grep -Eq 'Type:[[:space:]]+DYN' "$elf_report" ||
    die 'temporary-path redirect is not ET_DYN'
  /usr/bin/grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' \
    "$elf_report" || die 'temporary-path redirect has the wrong ELF machine'
  ! /usr/bin/grep -Eq '^[[:space:]]*INTERP[[:space:]]' "$elf_report" ||
    die 'temporary-path redirect has a PT_INTERP loader'
  ! /usr/bin/grep -Eq '\((NEEDED|RPATH|RUNPATH)\)' "$dynamic_report" ||
    die 'temporary-path redirect has an ambient dynamic dependency'
  # shellcheck disable=SC2016
  ! "$AWK" '$1 == "LOAD" && $0 ~ /W/ && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'temporary-path redirect has a writable executable LOAD segment'
  # shellcheck disable=SC2016
  ! "$AWK" '$1 == "GNU_STACK" && $0 ~ /E/ { found = 1 }
    END { exit found ? 0 : 1 }' "$elf_report" ||
    die 'temporary-path redirect has an executable stack'
  for symbol in __realpath_chk canonicalize_file_name fstatat lstat readlink \
    readlinkat realpath stat; do
    /usr/bin/grep -Eq "[[:space:]]$symbol$" "$elf_report" ||
      die "temporary-path redirect omits exported symbol $symbol"
  done
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
readonly script_dir="$source_root/scripts"
readonly build_script="$script_dir/fe2o3-static-host-lld-build.sh"
readonly guard_source="$script_dir/fe2o3-static-host-lld-build-guard.cpp"
readonly guard_test="$script_dir/fe2o3-static-host-lld-build-guard-test.sh"
readonly ctest_policy="$script_dir/fe2o3-static-host-lld-ctest-policy.sh"
readonly bootstrap_helper="$script_dir/fe2o3-static-host-lld-build-bootstrap.sh"
readonly trace_check_source="$script_dir/fe2o3-static-host-lld-build-trace-check.cpp"
readonly tmp_redirect_source="$script_dir/fe2o3-static-host-lld-tmp-redirect.cpp"
readonly tool_source_pin="$script_dir/fe2o3-static-host-lld-tool-source.pin"
readonly build_inputs_pin="$script_dir/fe2o3-static-host-lld-build-inputs.pin"
readonly roots_pin="$script_dir/fe2o3-static-host-lld-roots.pin"
readonly runtime_pin="$script_dir/fe2o3-static-host-lld-runtime.pin"
readonly llvm_package_parent="${llvm_root%/*}"
readonly llvm_source_parent="${llvm_source%/*}"
declare -a llvm_parent_labels=()
declare -a llvm_parent_paths=()
declare -a guard_llvm_parent_args=()

[[ -d "$tool_source" && ! -L "$tool_source" ]] ||
  die 'source root does not contain tools/fe2o3-host-lld'
[[ $(/usr/bin/readlink -f -- "$0") == "$build_script" ]] ||
  die 'build script must be invoked from the supplied canonical source root'
verify_file "$guard_source" "$PINNED_GUARD_SOURCE_SHA256" \
  "$PINNED_GUARD_SOURCE_LENGTH" 'build guard source' 644
verify_file "$guard_test" "$PINNED_GUARD_TEST_SHA256" \
  "$PINNED_GUARD_TEST_LENGTH" 'build guard test' 755
verify_file "$ctest_policy" "$PINNED_CTEST_POLICY_SHA256" \
  "$PINNED_CTEST_POLICY_LENGTH" 'CTest policy helper' 755
verify_file "$bootstrap_helper" "$PINNED_BOOTSTRAP_HELPER_SHA256" \
  "$PINNED_BOOTSTRAP_HELPER_LENGTH" 'bootstrap measurement helper' 644
verify_file "$trace_check_source" "$PINNED_TRACE_CHECK_SOURCE_SHA256" \
  "$PINNED_TRACE_CHECK_SOURCE_LENGTH" 'trace admission checker source' 644
verify_file "$tmp_redirect_source" "$PINNED_TMP_REDIRECT_SOURCE_SHA256" \
  "$PINNED_TMP_REDIRECT_SOURCE_LENGTH" 'temporary-path redirect source' 644
verify_pin_file "$build_inputs_pin" "$PINNED_BUILD_INPUTS_SHA256" \
  "$PINNED_BUILD_INPUTS_LENGTH" 'build-input pin'
verify_pin_file "$roots_pin" "$PINNED_ROOTS_SHA256" \
  "$PINNED_ROOTS_LENGTH" 'root pin'
verify_pin_file "$runtime_pin" "$PINNED_RUNTIME_SHA256" \
  "$PINNED_RUNTIME_LENGTH" 'runtime pin'
verify_file "$llvm_build_id_file" "$PINNED_LLVM_BUILD_ID_SHA256" \
  "$PINNED_LLVM_BUILD_ID_LENGTH" 'LLVM build-ID file' 664
[[ $(/usr/bin/tr -d '\n' <"$llvm_build_id_file") == "$PINNED_LLVM_BUILD_ID" ]] ||
  die 'LLVM build-ID file content differs from its pin'
verify_file "$llvm_closure_manifest" "$PINNED_LLVM_CLOSURE_SHA256" \
  "$PINNED_LLVM_CLOSURE_LENGTH" 'LLVM closure manifest' 664

exec {bootstrap_helper_fd}<"$bootstrap_helper"
# shellcheck disable=SC1090
source "/proc/self/fd/$bootstrap_helper_fd"
fe2o3_bootstrap_retain_directory scripts-directory "$script_dir" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory filesystem-root / ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory system-etc /etc ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory system-usr /usr ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory gcc-parent /usr/lib/gcc/x86_64-linux-gnu ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory gcc-libexec-parent \
  /usr/libexec/gcc/x86_64-linux-gnu || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory system-lib-parent \
  /usr/lib/x86_64-linux-gnu || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory source-root "$source_root" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory llvm-package-parent "$llvm_package_parent" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory llvm-source-root "$llvm_source" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory llvm-source-llvm-parent \
  "$llvm_source/llvm" || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory llvm-source-llvm-cmake-parent \
  "$llvm_source/llvm/cmake" || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_directory llvm-source-lld-parent "$llvm_source/lld" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
llvm_parent_labels+=(llvm-package-parent)
llvm_parent_paths+=("$llvm_package_parent")
guard_llvm_parent_args+=(--directory llvm-package-parent "$llvm_package_parent")
if [[ "$llvm_source_parent" != "$llvm_package_parent" ]]; then
  fe2o3_bootstrap_retain_directory llvm-source-parent "$llvm_source_parent" ||
    die "$FE2O3_BOOTSTRAP_ERROR"
  llvm_parent_labels+=(llvm-source-parent)
  llvm_parent_paths+=("$llvm_source_parent")
  guard_llvm_parent_args+=(--directory llvm-source-parent "$llvm_source_parent")
fi
fe2o3_bootstrap_retain_file build-script "$build_script" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file guard-source "$guard_source" \
  "$PINNED_GUARD_SOURCE_SHA256" "$PINNED_GUARD_SOURCE_LENGTH" 644 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file guard-test "$guard_test" "$PINNED_GUARD_TEST_SHA256" \
  "$PINNED_GUARD_TEST_LENGTH" 755 || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file ctest-policy "$ctest_policy" \
  "$PINNED_CTEST_POLICY_SHA256" "$PINNED_CTEST_POLICY_LENGTH" 755 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file bootstrap-helper "$bootstrap_helper" \
  "$PINNED_BOOTSTRAP_HELPER_SHA256" "$PINNED_BOOTSTRAP_HELPER_LENGTH" 644 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file trace-check-source "$trace_check_source" \
  "$PINNED_TRACE_CHECK_SOURCE_SHA256" "$PINNED_TRACE_CHECK_SOURCE_LENGTH" 644 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file tmp-redirect-source "$tmp_redirect_source" \
  "$PINNED_TMP_REDIRECT_SOURCE_SHA256" "$PINNED_TMP_REDIRECT_SOURCE_LENGTH" 644 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file tool-source-pin "$tool_source_pin" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
tool_source_pin_fd=$(fe2o3_bootstrap_file_descriptor_path "$tool_source_pin") ||
  die "$FE2O3_BOOTSTRAP_ERROR"
readonly tool_source_pin_fd
load_tool_source_pin "$tool_source_pin_fd"
fe2o3_bootstrap_retain_file build-input-pin "$build_inputs_pin" \
  "$PINNED_BUILD_INPUTS_SHA256" "$PINNED_BUILD_INPUTS_LENGTH" 644 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file root-pin "$roots_pin" "$PINNED_ROOTS_SHA256" \
  "$PINNED_ROOTS_LENGTH" 644 || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file runtime-pin "$runtime_pin" "$PINNED_RUNTIME_SHA256" \
  "$PINNED_RUNTIME_LENGTH" 644 || die "$FE2O3_BOOTSTRAP_ERROR"

declare -a guard_root_args=()
declare -a guard_file_args=()
declare -a guard_runtime_args=()
declare -a admission_root_labels=()
declare -a admission_root_paths=()
declare -a absence_root_labels=()
declare -a absence_root_paths=()
append_retained_guard_file build-script "$build_script"
append_retained_guard_file guard-source "$guard_source"
append_retained_guard_file guard-test "$guard_test"
append_retained_guard_file ctest-policy "$ctest_policy"
append_retained_guard_file bootstrap-helper "$bootstrap_helper"
append_retained_guard_file trace-check-source "$trace_check_source"
append_retained_guard_file tool-source-pin "$tool_source_pin"
append_retained_guard_file build-input-pin "$build_inputs_pin"
append_retained_guard_file root-pin "$roots_pin"
append_retained_guard_file runtime-pin "$runtime_pin"
load_build_input_pins "$build_inputs_pin" yes baseline
load_runtime_pins "$runtime_pin" yes baseline
load_root_pins "$roots_pin"
guard_root_args+=(
  --root tool-source "$tool_source" "$TOOL_SOURCE_ROOT_SHA256"
  "$TOOL_SOURCE_ROOT_LENGTH"
)
admission_root_labels+=(tool-source)
admission_root_paths+=("$tool_source")
fe2o3_bootstrap_retain_file llvm-build-id "$llvm_build_id_file" \
  "$PINNED_LLVM_BUILD_ID_SHA256" "$PINNED_LLVM_BUILD_ID_LENGTH" 664 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file llvm-closure-manifest "$llvm_closure_manifest" \
  "$PINNED_LLVM_CLOSURE_SHA256" "$PINNED_LLVM_CLOSURE_LENGTH" 664 ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file lld-elf "$llvm_root/lib/liblldELF.a" \
  "$PINNED_LLD_ELF_SHA256" 6796738 664 || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file lld-common "$llvm_root/lib/liblldCommon.a" \
  "$PINNED_LLD_COMMON_SHA256" 377796 664 || die "$FE2O3_BOOTSTRAP_ERROR"
guard_file_args+=(
  --file llvm-build-id "$llvm_build_id_file" "$PINNED_LLVM_BUILD_ID_SHA256"
  "$PINNED_LLVM_BUILD_ID_LENGTH" 664
  --file llvm-closure-manifest "$llvm_closure_manifest"
  "$PINNED_LLVM_CLOSURE_SHA256" "$PINNED_LLVM_CLOSURE_LENGTH" 664
  --file lld-elf "$llvm_root/lib/liblldELF.a" "$PINNED_LLD_ELF_SHA256"
  6796738 664
  --file lld-common "$llvm_root/lib/liblldCommon.a"
  "$PINNED_LLD_COMMON_SHA256" 377796 664
)

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
fe2o3_bootstrap_retain_directory work-parent "${build_dir%/*}" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
readonly llvm_before="$build_dir/llvm-closure-before.txt"
readonly llvm_after="$build_dir/llvm-closure-after.txt"
readonly source_before="$build_dir/source-before.txt"
readonly source_after="$build_dir/source-after.txt"
readonly runtime_manifest="$artifact_dir/fe2o3-host-lld.static-runtime-manifest.txt"
readonly staged_guard_source="$build_dir/fe2o3-static-host-lld-build-guard.cpp"
readonly staged_trace_check_source="$build_dir/fe2o3-static-host-lld-build-trace-check.cpp"
readonly staged_tmp_redirect_source="$build_dir/fe2o3-static-host-lld-tmp-redirect.cpp"
readonly guard_executable="$build_dir/fe2o3-static-host-lld-build-guard"
readonly trace_check_executable="$build_dir/fe2o3-static-host-lld-build-trace-check"
readonly tmp_redirect_library="$build_dir/fe2o3-static-host-lld-tmp-redirect.so"
readonly bootstrap_trace_allowlist="$build_dir/fe2o3-static-host-lld-bootstrap-trace.allowlist"
readonly trace_allowlist="$build_dir/fe2o3-static-host-lld-trace.allowlist"
readonly bootstrap_manifest="$build_dir/fe2o3-static-host-lld.bootstrap-measurement.txt"
readonly guard_compile_stdout="$build_dir/fe2o3-build-guard-compile.stdout"
readonly guard_compile_stderr="$build_dir/fe2o3-build-guard-compile.stderr"
readonly trace_check_compile_stdout="$build_dir/fe2o3-trace-check-compile.stdout"
readonly trace_check_compile_stderr="$build_dir/fe2o3-trace-check-compile.stderr"
readonly tmp_redirect_compile_stdout="$build_dir/fe2o3-tmp-redirect-compile.stdout"
readonly tmp_redirect_compile_stderr="$build_dir/fe2o3-tmp-redirect-compile.stderr"
readonly trace_check_readelf="$build_dir/fe2o3-trace-check.readelf.txt"
readonly trace_check_dynamic="$build_dir/fe2o3-trace-check.dynamic.txt"
readonly tmp_redirect_readelf="$build_dir/fe2o3-tmp-redirect.readelf.txt"
readonly tmp_redirect_dynamic="$build_dir/fe2o3-tmp-redirect.dynamic.txt"
readonly guard_readelf="$build_dir/fe2o3-build-guard.readelf.txt"
readonly guard_dynamic="$build_dir/fe2o3-build-guard.dynamic.txt"
readonly guard_status="$build_dir/fe2o3-build-guard.status.txt"
readonly ctest_policy_status="$build_dir/fe2o3-host-lld.ctest-policy-status.txt"
readonly trace_check_bootstrap_prefix="$build_dir/trace-check-bootstrap.raw"
readonly guard_bootstrap_prefix="$build_dir/guard-bootstrap.raw"
readonly tmp_redirect_bootstrap_prefix="$build_dir/tmp-redirect-bootstrap.raw"
readonly configure_trace_prefix="$build_dir/configure.raw"
readonly object_trace_prefix="$build_dir/object.raw"
readonly link_trace_prefix="$build_dir/link.raw"
readonly trace_check_bootstrap_checked="$build_dir/trace-check-bootstrap.checked-raw.txt"
readonly tmp_redirect_bootstrap_checked="$build_dir/tmp-redirect-bootstrap.checked-raw.txt"
readonly guard_bootstrap_checked="$build_dir/guard-bootstrap.checked-raw.txt"
readonly configure_trace_checked="$build_dir/configure.checked-raw.txt"
readonly object_trace_checked="$build_dir/object.checked-raw.txt"
readonly link_trace_checked="$build_dir/link.checked-raw.txt"
readonly retained_trace_check_bootstrap_prefix="$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.raw"
readonly retained_tmp_redirect_bootstrap_prefix="$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.raw"
readonly retained_guard_bootstrap_prefix="$artifact_dir/fe2o3-host-lld.guard-bootstrap.raw"
readonly retained_configure_trace_prefix="$artifact_dir/fe2o3-host-lld.configure.raw"
readonly retained_object_trace_prefix="$artifact_dir/fe2o3-host-lld.object.raw"
readonly retained_link_trace_prefix="$artifact_dir/fe2o3-host-lld.link.raw"
readonly raw_trace_retention_ledger="$artifact_dir/fe2o3-host-lld.raw-retention-ledger.txt"

/usr/bin/mkdir --mode=700 -- "$build_dir/tmp"
export LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 TMPDIR="$build_dir/tmp"
cd "$build_dir"

verify_llvm_closure "$llvm_root" "$llvm_closure_manifest" "$llvm_before"
generate_source_manifest "$tool_source" "$source_before"
verify_reviewed_source_manifest "$source_before" baseline
/usr/bin/install --mode=0444 -- "$runtime_pin" "$runtime_manifest"
verify_file "$llvm_root/lib/liblldELF.a" "$PINNED_LLD_ELF_SHA256" '6796738' \
  'pinned liblldELF.a' 664
verify_file "$llvm_root/lib/liblldCommon.a" "$PINNED_LLD_COMMON_SHA256" '377796' \
  'pinned liblldCommon.a' 664

write_trace_allowlist "$bootstrap_trace_allowlist"
/usr/bin/chmod 0444 "$bootstrap_trace_allowlist"
fe2o3_bootstrap_retain_file bootstrap-trace-allowlist \
  "$bootstrap_trace_allowlist" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
append_retained_guard_file bootstrap-trace-allowlist \
  "$bootstrap_trace_allowlist"
fe2o3_bootstrap_write_manifest "$bootstrap_manifest"
/usr/bin/chmod 0444 "$bootstrap_manifest"
fe2o3_bootstrap_verify_all before-bootstrap-compilation ||
  die "$FE2O3_BOOTSTRAP_ERROR"

exec {guard_source_fd}<"$guard_source"
guard_source_identity=$(/usr/bin/stat -Lc '%d:%i:%s:%a' \
  "/proc/self/fd/$guard_source_fd")
readonly guard_source_identity
/usr/bin/install --mode=0444 -- "/proc/self/fd/$guard_source_fd" \
  "$staged_guard_source"
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_source_fd") == "$guard_source_identity" ]] ||
  die 'build guard source descriptor changed while staging'
verify_file "$staged_guard_source" "$PINNED_GUARD_SOURCE_SHA256" \
  "$PINNED_GUARD_SOURCE_LENGTH" 'staged build guard source' 444
exec {guard_source_fd}<&-

exec {trace_source_fd}<"$trace_check_source"
trace_source_identity=$(/usr/bin/stat -Lc '%d:%i:%s:%a' \
  "/proc/self/fd/$trace_source_fd")
readonly trace_source_identity
/usr/bin/install --mode=0444 -- "/proc/self/fd/$trace_source_fd" \
  "$staged_trace_check_source"
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$trace_source_fd") == "$trace_source_identity" ]] ||
  die 'trace checker source descriptor changed while staging'
verify_file "$staged_trace_check_source" "$PINNED_TRACE_CHECK_SOURCE_SHA256" \
  "$PINNED_TRACE_CHECK_SOURCE_LENGTH" 'staged trace checker source' 444
exec {trace_source_fd}<&-

exec {tmp_redirect_source_fd}<"$tmp_redirect_source"
tmp_redirect_source_identity=$(/usr/bin/stat -Lc '%d:%i:%s:%a' \
  "/proc/self/fd/$tmp_redirect_source_fd")
readonly tmp_redirect_source_identity
/usr/bin/install --mode=0444 -- "/proc/self/fd/$tmp_redirect_source_fd" \
  "$staged_tmp_redirect_source"
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$tmp_redirect_source_fd") == "$tmp_redirect_source_identity" ]] ||
  die 'temporary-path redirect source descriptor changed while staging'
verify_file "$staged_tmp_redirect_source" "$PINNED_TMP_REDIRECT_SOURCE_SHA256" \
  "$PINNED_TMP_REDIRECT_SOURCE_LENGTH" \
  'staged temporary-path redirect source' 444
exec {tmp_redirect_source_fd}<&-

trace_options=(
  -ff -qq -yy -v -s 65535 -e 'trace=%file,%process,mmap'
)
# shellcheck disable=SC2054
static_compile_flags=(
  -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -Wconversion
  -Wsign-conversion -fno-rtti -fno-ident -save-temps=obj
  -fno-use-linker-plugin
  -static -static-libgcc
  -static-libstdc++ -no-pie -Wl,--build-id=none -Wl,--no-dynamic-linker
  -Wl,-z,noexecstack -Wl,-z,separate-code
)
readonly trace_options static_compile_flags

env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin TMPDIR="$TMPDIR" "$STRACE" "${trace_options[@]}" \
  -o "$trace_check_bootstrap_prefix" -- "$CXX" "${static_compile_flags[@]}" \
  -ffile-prefix-map="$staged_trace_check_source=fe2o3-trace-check-source" \
  -o "$trace_check_executable" "$staged_trace_check_source" \
  >"$trace_check_compile_stdout" 2>"$trace_check_compile_stderr"
/usr/bin/chmod 0500 "$trace_check_executable"
readonly trace_check_bootstrap_canonical="$build_dir/trace-check-bootstrap.canonical.txt"
readonly trace_check_bootstrap_inputs="$build_dir/trace-check-bootstrap.inputs.txt"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --check "$trace_check_bootstrap_prefix" \
  "$trace_check_bootstrap_canonical" "$trace_check_bootstrap_inputs" \
  "$bootstrap_trace_allowlist" "$build_dir" "$trace_check_bootstrap_checked"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --retain "$trace_check_bootstrap_checked" \
  "$trace_check_bootstrap_prefix" "$retained_trace_check_bootstrap_prefix" \
  "$trace_check_bootstrap_canonical" "$trace_check_bootstrap_inputs" \
  "$raw_trace_retention_ledger" trace-check-bootstrap \
  "$RAW_TRACE_GLOBAL_FILE_BOUND" "$RAW_TRACE_GLOBAL_BYTE_BOUND"
inspect_static_elf "$trace_check_executable" "$trace_check_readelf" \
  "$trace_check_dynamic"
fe2o3_bootstrap_verify_all after-trace-checker-compilation ||
  die "$FE2O3_BOOTSTRAP_ERROR"

# shellcheck disable=SC2054
shared_redirect_flags=(
  -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -Wconversion
  -Wsign-conversion -fPIC -fvisibility=hidden -fno-ident -fno-stack-protector
  -save-temps=obj -fno-use-linker-plugin
  -shared -nostdlib -nodefaultlibs -Wl,--build-id=none -Wl,-z,noexecstack
  -Wl,-z,separate-code
)
readonly shared_redirect_flags
env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin TMPDIR="$TMPDIR" "$STRACE" "${trace_options[@]}" \
  -o "$tmp_redirect_bootstrap_prefix" -- "$CXX" \
  "${shared_redirect_flags[@]}" \
  -ffile-prefix-map="$staged_tmp_redirect_source=fe2o3-tmp-redirect-source" \
  -o "$tmp_redirect_library" "$staged_tmp_redirect_source" \
  >"$tmp_redirect_compile_stdout" 2>"$tmp_redirect_compile_stderr"
/usr/bin/chmod 0444 "$tmp_redirect_library"
readonly tmp_redirect_bootstrap_canonical="$build_dir/tmp-redirect-bootstrap.canonical.txt"
readonly tmp_redirect_bootstrap_inputs="$build_dir/tmp-redirect-bootstrap.inputs.txt"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --check "$tmp_redirect_bootstrap_prefix" \
  "$tmp_redirect_bootstrap_canonical" "$tmp_redirect_bootstrap_inputs" \
  "$bootstrap_trace_allowlist" "$build_dir" "$tmp_redirect_bootstrap_checked"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --retain "$tmp_redirect_bootstrap_checked" \
  "$tmp_redirect_bootstrap_prefix" "$retained_tmp_redirect_bootstrap_prefix" \
  "$tmp_redirect_bootstrap_canonical" "$tmp_redirect_bootstrap_inputs" \
  "$raw_trace_retention_ledger" tmp-redirect-bootstrap \
  "$RAW_TRACE_GLOBAL_FILE_BOUND" "$RAW_TRACE_GLOBAL_BYTE_BOUND"
inspect_tmp_redirect_elf "$tmp_redirect_library" "$tmp_redirect_readelf" \
  "$tmp_redirect_dynamic"
fe2o3_bootstrap_verify_all after-tmp-redirect-compilation ||
  die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file tmp-redirect "$tmp_redirect_library" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
append_retained_guard_file tmp-redirect "$tmp_redirect_library"

env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin TMPDIR="$TMPDIR" "$STRACE" "${trace_options[@]}" \
  -o "$guard_bootstrap_prefix" -- "$CXX" "${static_compile_flags[@]}" \
  -ffile-prefix-map="$staged_guard_source=fe2o3-build-guard-source" \
  -o "$guard_executable" "$staged_guard_source" \
  >"$guard_compile_stdout" 2>"$guard_compile_stderr"
/usr/bin/chmod 0500 "$guard_executable"
readonly guard_bootstrap_canonical="$build_dir/guard-bootstrap.canonical.txt"
readonly guard_bootstrap_inputs="$build_dir/guard-bootstrap.inputs.txt"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --check "$guard_bootstrap_prefix" \
  "$guard_bootstrap_canonical" "$guard_bootstrap_inputs" \
  "$bootstrap_trace_allowlist" "$build_dir" "$guard_bootstrap_checked"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  "$trace_check_executable" --retain "$guard_bootstrap_checked" \
  "$guard_bootstrap_prefix" "$retained_guard_bootstrap_prefix" \
  "$guard_bootstrap_canonical" "$guard_bootstrap_inputs" \
  "$raw_trace_retention_ledger" guard-bootstrap \
  "$RAW_TRACE_GLOBAL_FILE_BOUND" "$RAW_TRACE_GLOBAL_BYTE_BOUND"
verify_file "$guard_source" "$PINNED_GUARD_SOURCE_SHA256" \
  "$PINNED_GUARD_SOURCE_LENGTH" 'build guard source after compilation' 644
verify_file "$staged_guard_source" "$PINNED_GUARD_SOURCE_SHA256" \
  "$PINNED_GUARD_SOURCE_LENGTH" 'staged guard source after compilation' 444
verify_pin_file "$build_inputs_pin" "$PINNED_BUILD_INPUTS_SHA256" \
  "$PINNED_BUILD_INPUTS_LENGTH" 'build-input pin after guard compilation'
verify_pin_file "$runtime_pin" "$PINNED_RUNTIME_SHA256" \
  "$PINNED_RUNTIME_LENGTH" 'runtime pin after guard compilation'
load_build_input_pins "$build_inputs_pin" no guard-compilation
load_runtime_pins "$runtime_pin" no guard-compilation
inspect_static_elf "$guard_executable" "$guard_readelf" "$guard_dynamic"
fe2o3_bootstrap_verify_all after-guard-compilation || die "$FE2O3_BOOTSTRAP_ERROR"
fe2o3_bootstrap_retain_file trace-checker "$trace_check_executable" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
append_retained_guard_file trace-checker "$trace_check_executable"
fe2o3_bootstrap_retain_file build-guard "$guard_executable" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
append_retained_guard_file build-guard "$guard_executable"
write_trace_allowlist "$trace_allowlist"
/usr/bin/chmod 0444 "$trace_allowlist"
fe2o3_bootstrap_retain_file trace-allowlist "$trace_allowlist" ||
  die "$FE2O3_BOOTSTRAP_ERROR"
append_retained_guard_file trace-allowlist "$trace_allowlist"

configure_inner=(
  "$CMAKE" --debug-trycompile -S "$tool_source" -B "$build_dir" -G Ninja
  -DBUILD_TESTING:BOOL=ON
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_MAKE_PROGRAM="$NINJA"
  -DCMAKE_CXX_COMPILER="$CXX"
  -DCMAKE_CXX_FLAGS=-save-temps=obj
  -DCMAKE_EXE_LINKER_FLAGS=-fno-use-linker-plugin
  -DLLVM_DIR="$llvm_root/lib/cmake/llvm"
  -DLLD_DIR="$llvm_root/lib/cmake/lld"
  -DFE2O3_LLVM_PACKAGE_ROOT="$llvm_root"
  -DFE2O3_LLVM_BUILD_ID_FILE="$llvm_build_id_file"
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="$PINNED_LLVM_BUILD_ID"
  -DFE2O3_EXPECTED_LLVM_SOURCE_COMMIT="$PINNED_LLVM_SOURCE_COMMIT"
  -DFE2O3_EXPECTED_LLVM_SOURCE_TREE="$PINNED_LLVM_SOURCE_TREE"
)
object_inner=(
  "$CMAKE" --build "$build_dir" --target
  CMakeFiles/fe2o3-host-lld.dir/src/main.cpp.o --parallel 16
)
link_inner=(
  "$CMAKE" --build "$build_dir" --target fe2o3-host-lld --parallel 16
)
readonly configure_trace_canonical="$build_dir/configure.canonical.txt"
readonly configure_trace_inputs="$build_dir/configure.inputs.txt"
readonly object_trace_canonical="$build_dir/object.canonical.txt"
readonly object_trace_inputs="$build_dir/object.inputs.txt"
readonly link_trace_canonical="$build_dir/link.canonical.txt"
readonly link_trace_inputs="$build_dir/link.inputs.txt"
configure_command=(
  "$STRACE" "${trace_options[@]}" -o "$configure_trace_prefix" --
  "${configure_inner[@]}"
)
configure_check_command=(
  "$trace_check_executable" --check "$configure_trace_prefix"
  "$configure_trace_canonical" "$configure_trace_inputs" "$trace_allowlist"
  "$build_dir" "$configure_trace_checked"
)
configure_retain_command=(
  "$trace_check_executable" --retain "$configure_trace_checked"
  "$configure_trace_prefix" "$retained_configure_trace_prefix"
  "$configure_trace_canonical" "$configure_trace_inputs"
  "$raw_trace_retention_ledger" configure "$RAW_TRACE_GLOBAL_FILE_BOUND"
  "$RAW_TRACE_GLOBAL_BYTE_BOUND"
)
ctest_policy_command=(
  /usr/bin/dash "$ctest_policy" "$tool_source" "$build_dir"
  "$ctest_policy_status"
)
object_command=(
  "$STRACE" "${trace_options[@]}" -o "$object_trace_prefix" --
  "${object_inner[@]}"
)
object_check_command=(
  "$trace_check_executable" --check "$object_trace_prefix"
  "$object_trace_canonical" "$object_trace_inputs" "$trace_allowlist"
  "$build_dir" "$object_trace_checked"
)
object_retain_command=(
  "$trace_check_executable" --retain "$object_trace_checked"
  "$object_trace_prefix" "$retained_object_trace_prefix"
  "$object_trace_canonical" "$object_trace_inputs"
  "$raw_trace_retention_ledger" object "$RAW_TRACE_GLOBAL_FILE_BOUND"
  "$RAW_TRACE_GLOBAL_BYTE_BOUND"
)
link_command=(
  "$STRACE" "${trace_options[@]}" -o "$link_trace_prefix" --
  "${link_inner[@]}"
)
link_check_command=(
  "$trace_check_executable" --check "$link_trace_prefix"
  "$link_trace_canonical" "$link_trace_inputs" "$trace_allowlist" "$build_dir"
  "$link_trace_checked"
)
link_retain_command=(
  "$trace_check_executable" --retain "$link_trace_checked"
  "$link_trace_prefix" "$retained_link_trace_prefix"
  "$link_trace_canonical" "$link_trace_inputs"
  "$raw_trace_retention_ledger" link "$RAW_TRACE_GLOBAL_FILE_BOUND"
  "$RAW_TRACE_GLOBAL_BYTE_BOUND"
)
readonly configure_inner object_inner link_inner
readonly configure_command configure_check_command configure_retain_command
readonly ctest_policy_command
readonly object_command object_check_command object_retain_command
readonly link_command link_check_command link_retain_command

exec {guard_fd}<"$guard_executable"
guard_identity=$(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_fd")
readonly guard_identity
env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin TMPDIR="$TMPDIR" TMP="$TMPDIR" TEMP="$TMPDIR" \
  "/proc/self/fd/$guard_fd" \
  --status "$guard_status" --max-entries 200000 --max-depth 128 \
  --max-manifest-bytes 67108864 \
  "${guard_root_args[@]}" --directory scripts-directory "$script_dir" \
  --directory filesystem-root / \
  --directory system-etc /etc \
  --directory system-usr /usr \
  --directory gcc-parent /usr/lib/gcc/x86_64-linux-gnu \
  --directory gcc-libexec-parent /usr/libexec/gcc/x86_64-linux-gnu \
  --directory system-lib-parent /usr/lib/x86_64-linux-gnu \
  --ancestor system-home /home \
  --ancestor user-home /home/harsh \
  --ancestor source-parent "${source_root%/*}" \
  --directory source-root "$source_root" \
  "${guard_llvm_parent_args[@]}" \
  --directory llvm-source-root "$llvm_source" \
  --directory llvm-source-llvm-parent "$llvm_source/llvm" \
  --directory llvm-source-llvm-cmake-parent "$llvm_source/llvm/cmake" \
  --directory llvm-source-lld-parent "$llvm_source/lld" \
  --directory work-parent "${build_dir%/*}" \
  "${guard_file_args[@]}" \
  --landlock-writable-root BUILD "$build_dir" \
  --landlock-writable-root ARTIFACT "$artifact_dir" \
  --landlock-writable-root TMP "$TMPDIR" \
  --tmp-redirect "$tmp_redirect_library" "$TMPDIR" \
  --landlock-read-write-file dev-null /dev/null \
  --landlock-read-only dev-urandom /dev/urandom \
  --landlock-read-only proc-cpuinfo /proc/cpuinfo \
  --landlock-read-only proc-version-signature /proc/version_signature \
  --landlock-read-only sys-cpu-online /sys/devices/system/cpu/online \
  --landlock-read-only sys-cpu-possible /sys/devices/system/cpu/possible \
  --resolver "$CXX" \
  "${guard_runtime_args[@]}" \
  --command "${#configure_command[@]}" "${configure_command[@]}" \
  --command "${#configure_check_command[@]}" "${configure_check_command[@]}" \
  --command "${#configure_retain_command[@]}" "${configure_retain_command[@]}" \
  --command "${#ctest_policy_command[@]}" "${ctest_policy_command[@]}" \
  --command "${#object_command[@]}" "${object_command[@]}" \
  --command "${#object_check_command[@]}" "${object_check_command[@]}" \
  --command "${#object_retain_command[@]}" "${object_retain_command[@]}" \
  --command "${#link_command[@]}" "${link_command[@]}" \
  --command "${#link_check_command[@]}" "${link_check_command[@]}" \
  --command "${#link_retain_command[@]}" "${link_retain_command[@]}"
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_fd") == "$guard_identity" ]] ||
  die 'build guard executable descriptor changed'
exec {guard_fd}<&-
exec {guard_status_fd}<"$guard_status"
guard_status_identity=$(/usr/bin/stat -Lc '%d:%i:%s:%a' \
  "/proc/self/fd/$guard_status_fd")
guard_status_sha256=$(sha256_file "/proc/self/fd/$guard_status_fd")
guard_status_length=$(file_length "/proc/self/fd/$guard_status_fd")
readonly guard_status_identity guard_status_sha256 guard_status_length
/usr/bin/grep -Fxq 'STATUS=passed' "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not retain a passing status'
/usr/bin/grep -Fxq \
  'SCOPE=measured-build-closure-integrity-with-landlock-filesystem-enforcement-and-observational-input-admission' \
  "/proc/self/fd/$guard_status_fd" || die 'build guard status has the wrong scope'
/usr/bin/grep -Fxq 'LANDLOCK_FILESYSTEM_ENFORCEMENT=passed' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not report Landlock filesystem enforcement'
/usr/bin/grep -Fxq 'LANDLOCK_ABI=4' "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not enforce the reviewed Landlock ABI'
/usr/bin/grep -Fxq 'LANDLOCK_HANDLED_FS_RIGHTS=0x7fff' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not handle every ABI-4 filesystem right'
/usr/bin/grep -Fxq \
  'LANDLOCK_MAKE_SYM=handled-and-denied-in-writable-roots' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not deny symlink creation in writable roots'
/usr/bin/grep -Fxq \
  'INHERITED_AMBIENT_DESCRIPTORS=closed-before-child-exec' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not close ambient child descriptors'
/usr/bin/grep -Fxq \
  'NETWORK_IPC_ISOLATION=provided-by-seccomp-deny-policy-v1' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not report its seccomp network IPC policy'
/usr/bin/grep -Fxq \
  'SECCOMP_X32_TAGGED_SYSCALLS=denied-with-EPERM-before-table-v1' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard did not report x32-tagged syscall rejection'
/usr/bin/grep -Fxq \
  'PROCESS_CREATION=allowed-required-subprocesses-inherit-policy' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard overstated process isolation'
/usr/bin/grep -Fxq \
  'AMBIENT_TMP_ACCESS=landlock-open-denied-with-partial-libc-metadata-redirect' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard overstated temporary-path redirection'
/usr/bin/grep -Fxq 'DIRECT_TMP_SYSCALL_REDIRECTION=not_provided' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard omitted direct temporary syscall limits'
/usr/bin/grep -Fxq \
  'STATUS_TERMINAL=fe2o3-static-host-lld-build-guard-status-v1-end' \
  "/proc/self/fd/$guard_status_fd" ||
  die 'build guard status omitted its terminal marker'
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_status_fd") == \
  "$guard_status_identity" &&
  $(sha256_file "/proc/self/fd/$guard_status_fd") == "$guard_status_sha256" ]] ||
  die 'build guard status descriptor changed during receipt'

fe2o3_bootstrap_verify_all after-guarded-build || die "$FE2O3_BOOTSTRAP_ERROR"

directory_identity_matches "$build_dir" "$build_identity" ||
  die 'build directory identity changed during compilation'
directory_identity_matches "$artifact_dir" "$artifact_identity" ||
  die 'artifact directory identity changed during compilation'

readonly expected_ctest_policy_status=$'FORMAT=fe2o3-static-host-lld-ctest-policy-v1\nSTATUS=passed\nBUILD_TESTING=enabled-exactly-once\nTEST_REGISTRATION=canonical-exactly-once\nOPTIONAL_CTEST_CACHE_KEYS=absent\nOPTIONAL_CTEST_DISCOVERY=absent\nPROTOCOL_CTEST_EXECUTION=not-executed-by-policy-check\nTERMINAL=fe2o3-static-host-lld-ctest-policy-v1-end'
[[ -f "$ctest_policy_status" && ! -L "$ctest_policy_status" &&
  $(/usr/bin/stat -Lc '%a' -- "$ctest_policy_status") == 600 &&
  $(<"$ctest_policy_status") == "$expected_ctest_policy_status" ]] ||
  die 'guarded configure did not retain a canonical CTest policy status'
verify_file "$ctest_policy" "$PINNED_CTEST_POLICY_SHA256" \
  "$PINNED_CTEST_POLICY_LENGTH" 'CTest policy helper after guarded build' 755

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
readonly expected_identity_output="$build_dir/fe2o3-host-lld.identity.expected.txt"
readonly stale_protocol_identity="$build_dir/fe2o3-host-lld.identity.stale-protocol.txt"
readonly stale_staging_identity="$build_dir/fe2o3-host-lld.identity.stale-staging.txt"
inspect_static_elf "$artifact_tool" "$readelf_output" "$dynamic_output"
env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 "$artifact_tool" \
  --fe2o3-identity-v1 >"$identity_output" 2>"$identity_stderr"
[[ ! -s "$identity_stderr" ]] ||
  die 'identity command wrote stderr'
write_expected_identity_report "$expected_identity_output"
identity_report_matches "$identity_output" "$expected_identity_output" ||
  die 'host LLD identity differs from the exact reviewed v2 contract'
/usr/bin/sed "s/^protocol=$PINNED_LINK_PROTOCOL$/protocol=fe2o3-host-lld-elf-v1/" \
  "$expected_identity_output" >"$stale_protocol_identity"
! identity_report_matches "$stale_protocol_identity" "$expected_identity_output" ||
  die 'identity verifier accepted the stale v1 link protocol'
/usr/bin/sed \
  "s/^output_staging=$PINNED_OUTPUT_STAGING$/output_staging=retained-private-directory-v1/" \
  "$expected_identity_output" >"$stale_staging_identity"
! identity_report_matches "$stale_staging_identity" "$expected_identity_output" ||
  die 'identity verifier accepted the stale retained-directory staging protocol'
identity_format=$(identity_value "$identity_output" format)
identity_authority=$(identity_value "$identity_output" authority)
identity_flavor=$(identity_value "$identity_output" flavor)
identity_protocol=$(identity_value "$identity_output" protocol)
identity_input_protocol=$(identity_value "$identity_output" input_protocol)
identity_result_protocol=$(identity_value "$identity_output" result_protocol)
identity_result_socket_fd=$(identity_value "$identity_output" result_socket_fd)
identity_output_staging=$(identity_value "$identity_output" output_staging)
readonly identity_format identity_authority identity_flavor identity_protocol
readonly identity_input_protocol identity_result_protocol identity_result_socket_fd
readonly identity_output_staging
artifact_tool_sha256=$(sha256_file "$artifact_tool")
artifact_tool_length=$(file_length "$artifact_tool")
readonly artifact_tool_sha256 artifact_tool_length

generate_tree_manifest "$llvm_root" "$llvm_after"
/usr/bin/cmp -s -- "$llvm_after" "$llvm_closure_manifest" ||
  die 'LLVM package closure changed during compilation'
generate_source_manifest "$tool_source" "$source_after"
/usr/bin/cmp -s -- "$source_before" "$source_after" ||
  die 'host LLD source changed during compilation'
verify_reviewed_source_manifest "$source_after" 'post-build revalidation'
verify_file "$guard_source" "$PINNED_GUARD_SOURCE_SHA256" \
  "$PINNED_GUARD_SOURCE_LENGTH" 'build guard source after build' 644
verify_file "$guard_test" "$PINNED_GUARD_TEST_SHA256" \
  "$PINNED_GUARD_TEST_LENGTH" 'build guard test after build' 755
verify_file "$trace_check_source" "$PINNED_TRACE_CHECK_SOURCE_SHA256" \
  "$PINNED_TRACE_CHECK_SOURCE_LENGTH" 'trace checker source after build' 644
verify_file "$tmp_redirect_source" "$PINNED_TMP_REDIRECT_SOURCE_SHA256" \
  "$PINNED_TMP_REDIRECT_SOURCE_LENGTH" \
  'temporary-path redirect source after build' 644
verify_pin_file "$build_inputs_pin" "$PINNED_BUILD_INPUTS_SHA256" \
  "$PINNED_BUILD_INPUTS_LENGTH" 'build-input pin after build'
verify_pin_file "$roots_pin" "$PINNED_ROOTS_SHA256" \
  "$PINNED_ROOTS_LENGTH" 'root pin after build'
verify_pin_file "$runtime_pin" "$PINNED_RUNTIME_SHA256" \
  "$PINNED_RUNTIME_LENGTH" 'runtime pin after build'
load_build_input_pins "$build_inputs_pin" no post-build
load_runtime_pins "$runtime_pin" no post-build
verify_file "$llvm_build_id_file" "$PINNED_LLVM_BUILD_ID_SHA256" \
  "$PINNED_LLVM_BUILD_ID_LENGTH" 'LLVM build-ID after build' 664
verify_file "$llvm_closure_manifest" "$PINNED_LLVM_CLOSURE_SHA256" \
  "$PINNED_LLVM_CLOSURE_LENGTH" 'LLVM closure manifest after build' 664
verify_file "$llvm_root/lib/liblldELF.a" "$PINNED_LLD_ELF_SHA256" '6796738' \
  'pinned liblldELF.a after build' 664
verify_file "$llvm_root/lib/liblldCommon.a" "$PINNED_LLD_COMMON_SHA256" '377796' \
  'pinned liblldCommon.a after build' 664
[[ $(/usr/bin/git -C "$llvm_source" rev-parse HEAD) == "$PINNED_LLVM_SOURCE_COMMIT" ]] ||
  die 'LLVM source commit changed during build'
[[ $(/usr/bin/git -C "$llvm_source" rev-parse 'HEAD^{tree}') == "$PINNED_LLVM_SOURCE_TREE" ]] ||
  die 'LLVM source tree changed during build'
[[ -z $(/usr/bin/git -C "$llvm_source" status --short --untracked-files=no) ]] ||
  die 'LLVM source acquired tracked modifications during build'

readonly guard_test_stdout="$build_dir/fe2o3-build-guard-test.stdout"
readonly guard_test_stderr="$build_dir/fe2o3-build-guard-test.stderr"
env -i HOME=/nonexistent LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
  PATH=/usr/bin:/bin CXX="$CXX" "$guard_test" \
  >"$guard_test_stdout" 2>"$guard_test_stderr"
[[ $(<"$guard_test_stdout") == 'fe2o3 static host LLD build guard tests passed' ]] ||
  die 'build guard self-test output is not canonical'
[[ ! -s "$guard_test_stderr" ]] || die 'build guard self-test wrote stderr'
fe2o3_bootstrap_verify_all after-guard-self-test || die "$FE2O3_BOOTSTRAP_ERROR"

/usr/bin/install --mode=0444 -- "$source_before" \
  "$artifact_dir/fe2o3-host-lld.source-manifest.txt"
readonly source_manifest="$artifact_dir/fe2o3-host-lld.source-manifest.txt"
readonly artifact_manifest="$artifact_dir/fe2o3-host-lld.artifact-manifest.txt"
readonly retained_guard_status="$artifact_dir/fe2o3-host-lld.build-guard-status.txt"
readonly retained_build_inputs="$artifact_dir/fe2o3-host-lld.build-inputs.pin"
readonly retained_roots="$artifact_dir/fe2o3-host-lld.roots.pin"
readonly retained_guard_readelf="$artifact_dir/fe2o3-host-lld.build-guard.readelf.txt"
readonly retained_guard_dynamic="$artifact_dir/fe2o3-host-lld.build-guard.dynamic.txt"
readonly retained_guard_test="$artifact_dir/fe2o3-host-lld.build-guard-test.txt"
readonly retained_ctest_policy="$artifact_dir/fe2o3-host-lld.ctest-policy.sh"
readonly retained_ctest_policy_status="$artifact_dir/fe2o3-host-lld.ctest-policy-status.txt"
readonly retained_guard_elf="$artifact_dir/fe2o3-host-lld.build-guard"
readonly retained_trace_elf="$artifact_dir/fe2o3-host-lld.trace-check"
readonly retained_build_script="$artifact_dir/fe2o3-host-lld.build-script.sh"
readonly retained_bootstrap_helper="$artifact_dir/fe2o3-host-lld.build-bootstrap.sh"
readonly retained_bootstrap_manifest="$artifact_dir/fe2o3-host-lld.bootstrap-measurement.txt"
readonly retained_guard_source="$artifact_dir/fe2o3-host-lld.build-guard-source.cpp"
readonly retained_trace_source="$artifact_dir/fe2o3-host-lld.trace-check-source.cpp"
readonly retained_tmp_redirect_source="$artifact_dir/fe2o3-host-lld.tmp-redirect-source.cpp"
readonly retained_tmp_redirect_library="$artifact_dir/fe2o3-host-lld.tmp-redirect.so"
readonly retained_bootstrap_trace_allowlist="$artifact_dir/fe2o3-host-lld.bootstrap-trace-allowlist.txt"
readonly retained_trace_allowlist="$artifact_dir/fe2o3-host-lld.trace-allowlist.txt"
readonly retained_trace_readelf="$artifact_dir/fe2o3-host-lld.trace-check.readelf.txt"
readonly retained_trace_dynamic="$artifact_dir/fe2o3-host-lld.trace-check.dynamic.txt"
readonly retained_tmp_redirect_readelf="$artifact_dir/fe2o3-host-lld.tmp-redirect.readelf.txt"
readonly retained_tmp_redirect_dynamic="$artifact_dir/fe2o3-host-lld.tmp-redirect.dynamic.txt"
readonly retained_tool_source_pin="$artifact_dir/fe2o3-host-lld.tool-source.pin"
/usr/bin/install --mode=0444 -- "/proc/self/fd/$guard_status_fd" \
  "$retained_guard_status"
verify_file "$retained_guard_status" "$guard_status_sha256" \
  "$guard_status_length" 'receiver-retained build guard status' 444
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_status_fd") == \
  "$guard_status_identity" &&
  $(sha256_file "/proc/self/fd/$guard_status_fd") == "$guard_status_sha256" ]] ||
  die 'build guard status descriptor changed during receiver retention'
/usr/bin/install --mode=0444 -- "$build_inputs_pin" "$retained_build_inputs"
/usr/bin/install --mode=0444 -- "$roots_pin" "$retained_roots"
/usr/bin/install --mode=0444 -- "$guard_readelf" "$retained_guard_readelf"
/usr/bin/install --mode=0444 -- "$guard_dynamic" "$retained_guard_dynamic"
/usr/bin/install --mode=0444 -- "$guard_test_stdout" "$retained_guard_test"
/usr/bin/install --mode=0444 -- "$ctest_policy" "$retained_ctest_policy"
/usr/bin/install --mode=0444 -- "$ctest_policy_status" \
  "$retained_ctest_policy_status"
/usr/bin/install --mode=0555 -- "$guard_executable" "$retained_guard_elf"
/usr/bin/install --mode=0555 -- "$trace_check_executable" "$retained_trace_elf"
/usr/bin/install --mode=0444 -- "$build_script" "$retained_build_script"
/usr/bin/install --mode=0444 -- "$bootstrap_helper" "$retained_bootstrap_helper"
/usr/bin/install --mode=0444 -- "$bootstrap_manifest" \
  "$retained_bootstrap_manifest"
/usr/bin/install --mode=0444 -- "$guard_source" "$retained_guard_source"
/usr/bin/install --mode=0444 -- "$trace_check_source" "$retained_trace_source"
/usr/bin/install --mode=0444 -- "$tmp_redirect_source" \
  "$retained_tmp_redirect_source"
/usr/bin/install --mode=0444 -- "$tmp_redirect_library" \
  "$retained_tmp_redirect_library"
/usr/bin/install --mode=0444 -- "$bootstrap_trace_allowlist" \
  "$retained_bootstrap_trace_allowlist"
/usr/bin/install --mode=0444 -- "$trace_allowlist" "$retained_trace_allowlist"
/usr/bin/install --mode=0444 -- "$trace_check_readelf" \
  "$retained_trace_readelf"
/usr/bin/install --mode=0444 -- "$trace_check_dynamic" \
  "$retained_trace_dynamic"
/usr/bin/install --mode=0444 -- "$tmp_redirect_readelf" \
  "$retained_tmp_redirect_readelf"
/usr/bin/install --mode=0444 -- "$tmp_redirect_dynamic" \
  "$retained_tmp_redirect_dynamic"
/usr/bin/install --mode=0444 -- "$tool_source_pin" "$retained_tool_source_pin"

readonly retained_raw_trace_index="$artifact_dir/fe2o3-host-lld.raw-traces.txt"
readonly expected_raw_trace_retention_ledger="$build_dir/fe2o3-host-lld.expected-raw-retention-ledger.txt"
{
  printf 'FORMAT=fe2o3-static-host-lld-retained-raw-traces-v2\n'
  printf 'STATUS=measured-observational-replay-evidence\n'
  printf 'GLOBAL_FILE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_FILE_BOUND"
  printf 'GLOBAL_BYTE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_BYTE_BOUND"
  printf 'PER_PID_TRACE_BYTE_BOUND=67108864\n'
} >"$retained_raw_trace_index"
{
  printf 'FORMAT=fe2o3-static-host-lld-global-retention-ledger-v1\n'
  printf 'STATUS=global-precopy-budget-accounting\n'
  printf 'GLOBAL_FILE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_FILE_BOUND"
  printf 'GLOBAL_BYTE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_BYTE_BOUND"
} >"$expected_raw_trace_retention_ledger"
declare -A retained_raw_artifacts=()
retained_raw_global_files=0
retained_raw_global_bytes=0

retain_trace_evidence() {
  local phase=$1 canonical=$2 inputs=$3 checked=$4 retained_prefix=$5
  local allowlist=$6 replay_canonical replay_inputs replay_checked
  local kind pid device inode length digest path extra name
  local count=0 expected_count='' expected_bytes='' observed_bytes=0 sentinel=0
  local -a phase_index_rows=()
  local -A seen_pids=()
  replay_canonical="$artifact_dir/fe2o3-host-lld.$phase.replay.canonical.txt"
  replay_inputs="$artifact_dir/fe2o3-host-lld.$phase.replay.inputs.txt"
  replay_checked="$artifact_dir/fe2o3-host-lld.$phase.checked-raw.txt"
  env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
    "$trace_check_executable" --check "$retained_prefix" \
    "$replay_canonical" "$replay_inputs" "$allowlist" "$build_dir" \
    "$replay_checked"
  /usr/bin/cmp --silent -- "$canonical" "$replay_canonical" ||
    die "retained raw trace replay changed canonical evidence during $phase"
  /usr/bin/cmp --silent -- "$inputs" "$replay_inputs" ||
    die "retained raw trace replay changed admitted inputs during $phase"
  /usr/bin/install --mode=0444 -- "$canonical" \
    "$artifact_dir/fe2o3-host-lld.$phase.canonical.txt"
  /usr/bin/install --mode=0444 -- "$inputs" \
    "$artifact_dir/fe2o3-host-lld.$phase.inputs.txt"
  /usr/bin/grep -Fxq \
    'TERMINAL=fe2o3-static-host-lld-checked-raw-traces-v1-end' \
    "$checked" || die "initial checked raw record is incomplete during $phase"
  while IFS=$'\t' read -r kind pid device inode length digest path extra; do
    case "$kind" in
      FORMAT=fe2o3-static-host-lld-checked-raw-traces-v1 | STATUS=descriptor-bound-checked-bytes | PREFIX=* | PER_FILE_BYTE_BOUND=67108864 | AGGREGATE_BYTE_BOUND=268435456 | CANONICAL_SHA256=* | INPUTS_SHA256=* | TERMINAL=fe2o3-static-host-lld-checked-raw-traces-v1-end)
        [[ -z "$pid$device$inode$length$digest$path$extra" ]] ||
          die "checked raw record header has extra fields during $phase"
        ;;
      F)
        [[ "$pid" =~ ^[0-9]+$ && "$device" =~ ^[0-9]+$ &&
          "$inode" =~ ^[0-9]+$ &&
          "$length" =~ ^(0|[1-9][0-9]{0,7})$ &&
          length -le 67108864 &&
          "$digest" =~ ^[0-9a-f]{64}$ && -z "$extra" &&
          -z ${seen_pids["$pid"]+present} ]] ||
          die "checked raw record has a noncanonical row during $phase"
        seen_pids["$pid"]=1
        name="fe2o3-host-lld.$phase.raw.$pid"
        [[ "$path" == "$artifact_dir/$name" ]] ||
          die "checked raw record names the wrong retained path during $phase"
        verify_file "$path" "$digest" "$length" \
          "retained raw trace during $phase" 444
        retained_raw_artifacts["$name"]=444
        phase_index_rows+=("$(printf 'F\t%s\t%s\t%s\t%s\t%s' \
          "$phase" "$pid" "$name" "$length" "$digest")")
        ((count += 1))
        ((observed_bytes += length))
        ;;
      FILES=*)
        [[ $sentinel -eq 0 && -z "$pid$device$inode$length$digest$path$extra" ]] ||
          die "checked raw record repeats its count during $phase"
        expected_count=${kind#FILES=}
        [[ "$expected_count" =~ ^(0|[1-9][0-9]{0,5})$ &&
          expected_count -le RAW_TRACE_GLOBAL_FILE_BOUND ]] ||
          die "checked raw record count is invalid during $phase"
        sentinel=1
        ;;
      TOTAL_BYTES=*)
        [[ $sentinel -eq 1 && -z "$pid$device$inode$length$digest$path$extra" ]] ||
          die "checked raw record has a misplaced byte count during $phase"
        expected_bytes=${kind#TOTAL_BYTES=}
        [[ "$expected_bytes" =~ ^(0|[1-9][0-9]{0,8})$ &&
          expected_bytes -le RAW_TRACE_GLOBAL_BYTE_BOUND ]] ||
          die "checked raw record byte count is invalid during $phase"
        ;;
      *) die "checked raw record has an unknown row during $phase" ;;
    esac
  done <"$replay_checked"
  [[ $sentinel -eq 1 && $count -eq expected_count &&
    $observed_bytes -eq expected_bytes ]] ||
    die "retained raw trace count or bytes differ during $phase"
  ((count <= RAW_TRACE_GLOBAL_FILE_BOUND - retained_raw_global_files)) ||
    die "retained raw trace global file bound exceeded during $phase"
  ((observed_bytes <= RAW_TRACE_GLOBAL_BYTE_BOUND - retained_raw_global_bytes)) ||
    die "retained raw trace global byte bound exceeded during $phase"
  printf 'P\t%s\t%s\t%s\n' "$phase" "$count" "$observed_bytes" \
    >>"$retained_raw_trace_index"
  printf '%s\n' "${phase_index_rows[@]}" >>"$retained_raw_trace_index"
  printf 'P\t%s\t%s\t%s\n' "$phase" "$count" "$observed_bytes" \
    >>"$expected_raw_trace_retention_ledger"
  ((retained_raw_global_files += count))
  ((retained_raw_global_bytes += observed_bytes))
}

retain_trace_evidence trace-check-bootstrap "$trace_check_bootstrap_canonical" \
  "$trace_check_bootstrap_inputs" "$trace_check_bootstrap_checked" \
  "$retained_trace_check_bootstrap_prefix" "$bootstrap_trace_allowlist"
retain_trace_evidence tmp-redirect-bootstrap "$tmp_redirect_bootstrap_canonical" \
  "$tmp_redirect_bootstrap_inputs" "$tmp_redirect_bootstrap_checked" \
  "$retained_tmp_redirect_bootstrap_prefix" "$bootstrap_trace_allowlist"
retain_trace_evidence guard-bootstrap "$guard_bootstrap_canonical" \
  "$guard_bootstrap_inputs" "$guard_bootstrap_checked" \
  "$retained_guard_bootstrap_prefix" "$bootstrap_trace_allowlist"
retain_trace_evidence configure "$configure_trace_canonical" \
  "$configure_trace_inputs" "$configure_trace_checked" \
  "$retained_configure_trace_prefix" "$trace_allowlist"
retain_trace_evidence object "$object_trace_canonical" \
  "$object_trace_inputs" "$object_trace_checked" \
  "$retained_object_trace_prefix" "$trace_allowlist"
retain_trace_evidence link "$link_trace_canonical" "$link_trace_inputs" \
  "$link_trace_checked" "$retained_link_trace_prefix" "$trace_allowlist"
{
  printf 'FILES=%s\n' "$retained_raw_global_files"
  printf 'TOTAL_BYTES=%s\n' "$retained_raw_global_bytes"
  printf 'TERMINAL=fe2o3-static-host-lld-retained-raw-traces-v2-end\n'
} >>"$retained_raw_trace_index"
{
  printf 'FILES=%s\n' "$retained_raw_global_files"
  printf 'TOTAL_BYTES=%s\n' "$retained_raw_global_bytes"
  printf 'TERMINAL=fe2o3-static-host-lld-global-retention-ledger-v1-end\n'
} >>"$expected_raw_trace_retention_ledger"
/usr/bin/cmp --silent -- "$expected_raw_trace_retention_ledger" \
  "$raw_trace_retention_ledger" ||
  die 'global raw trace retention ledger differs from replayed phase totals'
/usr/bin/grep -Fxq "FILES=$retained_raw_global_files" \
  "$retained_raw_trace_index" || die 'raw trace index omits global file total'
/usr/bin/grep -Fxq "TOTAL_BYTES=$retained_raw_global_bytes" \
  "$retained_raw_trace_index" || die 'raw trace index omits global byte total'
[[ $("$AWK" 'END { print }' "$retained_raw_trace_index") == \
  'TERMINAL=fe2o3-static-host-lld-retained-raw-traces-v2-end' ]] ||
  die 'raw trace index omits its canonical terminal record'
/usr/bin/chmod 0444 "$retained_raw_trace_index" "$raw_trace_retention_ledger"
retained_raw_artifacts[fe2o3-host-lld.raw-retention-ledger.txt]=444
readonly retained_raw_global_files retained_raw_global_bytes
fe2o3_bootstrap_verify_all before-final-evidence || die "$FE2O3_BOOTSTRAP_ERROR"
# shellcheck disable=SC2016
runtime_entries=$("$AWK" -F '\t' '$1 == "F" { count += 1 } END { print count + 0 }' \
  "$runtime_manifest")
readonly runtime_entries

emit_evidence_file() {
  local key=$1 path=$2
  printf '%s_SHA256=%s\n' "$key" "$(sha256_file "$path")"
  printf '%s_LENGTH=%s\n' "$key" "$(file_length "$path")"
}

{
  printf 'FORMAT=fe2o3-host-lld-artifact-v1\n'
  printf 'STATUS=measured-no-authority\n'
  printf 'TOOL_BASENAME=fe2o3-host-lld\n'
  printf 'TOOL_SHA256=%s\n' "$artifact_tool_sha256"
  printf 'TOOL_LENGTH=%s\n' "$artifact_tool_length"
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
  printf 'IDENTITY_FORMAT=%s\n' "$identity_format"
  printf 'SUPPORTED_FLAVOR=%s\n' "$identity_flavor"
  printf 'SUPPORTED_PROTOCOL=%s\n' "$identity_protocol"
  printf 'INPUT_PROTOCOL=%s\n' "$identity_input_protocol"
  printf 'RESULT_PROTOCOL=%s\n' "$identity_result_protocol"
  printf 'RESULT_SOCKET_FD=%s\n' "$identity_result_socket_fd"
  printf 'OUTPUT_STAGING_PROTOCOL=%s\n' "$identity_output_staging"
  printf 'LLVM_VERSION=%s\n' "$PINNED_LLVM_VERSION"
  printf 'LLVM_BUILD_ID=%s\n' "$PINNED_LLVM_BUILD_ID"
  printf 'LLVM_SOURCE_COMMIT=%s\n' "$PINNED_LLVM_SOURCE_COMMIT"
  printf 'LLVM_SOURCE_TREE=%s\n' "$PINNED_LLVM_SOURCE_TREE"
  printf 'LLVM_PACKAGE_CLOSURE_SHA256=%s\n' "$PINNED_LLVM_CLOSURE_SHA256"
  printf 'LLD_ELF_SHA256=%s\n' "$PINNED_LLD_ELF_SHA256"
  printf 'LLD_COMMON_SHA256=%s\n' "$PINNED_LLD_COMMON_SHA256"
  printf 'CXX_SHA256=%s\n' "$(sha256_file "$CXX")"
  printf 'SOURCE_MANIFEST_SHA256=%s\n' "$(sha256_file "$source_manifest")"
  printf 'SOURCE_MANIFEST_LENGTH=%s\n' "$(file_length "$source_manifest")"
  printf 'STATIC_RUNTIME_MANIFEST_SHA256=%s\n' "$(sha256_file "$runtime_manifest")"
  printf 'STATIC_RUNTIME_MANIFEST_LENGTH=%s\n' "$(file_length "$runtime_manifest")"
  printf 'STATIC_RUNTIME_ENTRIES=%s\n' "$runtime_entries"
  printf 'BUILD_GUARD_STATUS_SHA256=%s\n' \
    "$(sha256_file "$retained_guard_status")"
  printf 'BUILD_GUARD_STATUS_LENGTH=%s\n' \
    "$(file_length "$retained_guard_status")"
  printf 'BUILD_GUARD_SELF_TEST=passed\n'
  printf 'BUILD_CLOSURE_SCOPE=measured-build-closure-integrity-with-landlock-filesystem-enforcement-and-observational-input-admission\n'
  printf 'LANDLOCK_FILESYSTEM_ENFORCEMENT=passed\n'
  printf 'LANDLOCK_ABI=4\n'
  printf 'LANDLOCK_HANDLED_FS_RIGHTS=0x7fff\n'
  printf 'LANDLOCK_MAKE_SYM=handled-and-denied-in-writable-roots\n'
  printf 'TRACE_ADMISSION=observational-gap-detector\n'
  printf 'INHERITED_AMBIENT_DESCRIPTORS=closed-before-child-exec\n'
  printf 'NETWORK_IPC_ISOLATION=provided-by-seccomp-deny-policy-v1\n'
  printf 'SECCOMP_X32_TAGGED_SYSCALLS=denied-with-EPERM-before-table-v1\n'
  printf 'NETWORK_NAMESPACE_ISOLATION=not_provided\n'
  printf 'PROCESS_ISOLATION=not_provided\n'
  printf 'PROCESS_CREATION=allowed-required-subprocesses-inherit-policy\n'
  printf 'AMBIENT_TMP_ACCESS=landlock-open-denied-with-partial-libc-metadata-redirect\n'
  printf 'TMP_METADATA_REDIRECT=partial-reviewed-libc-symbol-interposition\n'
  printf 'DIRECT_TMP_SYSCALL_REDIRECTION=not_provided\n'
  printf 'GLOBAL_TMP_FILE_OPEN=landlock-denied\n'
  printf 'GLOBAL_TMP_METADATA_SYSCALLS=observational-only\n'
  printf 'RAW_TRACE_RETENTION=complete-bounded-replayable\n'
  printf 'RAW_TRACE_GLOBAL_FILE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_FILE_BOUND"
  printf 'RAW_TRACE_GLOBAL_BYTE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_BYTE_BOUND"
  printf 'RAW_TRACE_FILES=%s\n' "$retained_raw_global_files"
  printf 'RAW_TRACE_TOTAL_BYTES=%s\n' "$retained_raw_global_bytes"
  printf 'RAW_TRACE_INDEX_SHA256=%s\n' \
    "$(sha256_file "$retained_raw_trace_index")"
  printf 'RAW_TRACE_INDEX_LENGTH=%s\n' \
    "$(file_length "$retained_raw_trace_index")"
  printf 'RAW_TRACE_RETENTION_LEDGER_SHA256=%s\n' \
    "$(sha256_file "$raw_trace_retention_ledger")"
  printf 'RAW_TRACE_RETENTION_LEDGER_LENGTH=%s\n' \
    "$(file_length "$raw_trace_retention_ledger")"
  printf 'TMP_REDIRECT_SHA256=%s\n' \
    "$(sha256_file "$retained_tmp_redirect_library")"
  printf 'TMP_REDIRECT_LENGTH=%s\n' \
    "$(file_length "$retained_tmp_redirect_library")"
  printf 'TOOL_SOURCE_PIN_STATUS=%s\n' "$TOOL_SOURCE_PIN_STATUS"
  printf 'PROTECTED_PUBLICATION=absent\n'
  printf 'IDENTITY_REPORT_SHA256=%s\n' "$(sha256_file "$identity_output")"
  printf 'IDENTITY_REPORT_LENGTH=%s\n' "$(file_length "$identity_output")"
  printf 'IDENTITY_CONTRACT_SELF_TEST=passed\n'
  printf 'AUTHORITY=%s\n' "$identity_authority"
  printf 'BROKER_IDENTITY=not_constructed\n'
  printf 'ARTIFACT_HANDOFF=not_constructed\n'
  printf 'GPU_LINKER=unchanged\n'
  printf 'COMGR=absent\n'
} >"$artifact_manifest"
readonly build_evidence_manifest="$artifact_dir/fe2o3-host-lld.build-evidence-manifest.txt"
{
  printf 'FORMAT=fe2o3-host-lld-build-evidence-v1\n'
  printf 'STATUS=measured-no-authority\n'
  printf 'SCOPE=measured-build-closure-integrity-with-landlock-filesystem-enforcement-and-observational-input-admission\n'
  printf 'PROTECTED_PUBLICATION=absent\n'
  printf 'BUILD_DIRECTORY_PATH=%s\n' "$build_dir"
  printf 'BUILD_DIRECTORY_IDENTITY=%s\n' "$build_identity"
  printf 'ARTIFACT_DIRECTORY_PATH=%s\n' "$artifact_dir"
  printf 'ARTIFACT_DIRECTORY_IDENTITY=%s\n' "$artifact_identity"
  printf 'SOURCE_ROOT_PATH=%s\n' "$source_root"
  printf 'SCRIPTS_DIRECTORY_IDENTITY=%s\n' \
    "$(/usr/bin/stat -Lc '%d:%i' -- "$script_dir")"
  printf 'TOOL_SOURCE_IDENTITY=%s\n' \
    "$(/usr/bin/stat -Lc '%d:%i' -- "$tool_source")"
  printf 'TOOL_SOURCE_MANIFEST_SHA256=%s\n' "$(sha256_file "$source_manifest")"
  printf 'TOOL_SOURCE_MANIFEST_LENGTH=%s\n' "$(file_length "$source_manifest")"
  printf 'TOOL_SOURCE_PIN_STATUS=%s\n' "$TOOL_SOURCE_PIN_STATUS"
  printf 'TOOL_SOURCE_ORIGIN_COMMIT=%s\n' "$TOOL_SOURCE_ORIGIN_COMMIT"
  printf 'TOOL_SOURCE_ORIGIN_TREE=%s\n' "$TOOL_SOURCE_ORIGIN_TREE"
  printf 'TOOL_SOURCE_PINNED_MANIFEST_SHA256=%s\n' \
    "$TOOL_SOURCE_MANIFEST_SHA256"
  printf 'TOOL_SOURCE_PINNED_MANIFEST_LENGTH=%s\n' \
    "$TOOL_SOURCE_MANIFEST_LENGTH"
  printf 'TOOL_SOURCE_PINNED_ROOT_SHA256=%s\n' "$TOOL_SOURCE_ROOT_SHA256"
  printf 'TOOL_SOURCE_PINNED_ROOT_LENGTH=%s\n' "$TOOL_SOURCE_ROOT_LENGTH"
  emit_evidence_file BUILD_SCRIPT "$retained_build_script"
  emit_evidence_file BOOTSTRAP_HELPER "$retained_bootstrap_helper"
  emit_evidence_file CTEST_POLICY_HELPER "$retained_ctest_policy"
  emit_evidence_file CTEST_POLICY_STATUS "$retained_ctest_policy_status"
  emit_evidence_file BOOTSTRAP_MANIFEST "$retained_bootstrap_manifest"
  emit_evidence_file TOOL_SOURCE_PIN "$retained_tool_source_pin"
  printf 'ROOT_PIN_SHA256=%s\n' "$(sha256_file "$retained_roots")"
  printf 'ROOT_PIN_LENGTH=%s\n' "$(file_length "$retained_roots")"
  printf 'BUILD_INPUT_PIN_SHA256=%s\n' "$(sha256_file "$retained_build_inputs")"
  printf 'BUILD_INPUT_PIN_LENGTH=%s\n' "$(file_length "$retained_build_inputs")"
  printf 'RUNTIME_PIN_SHA256=%s\n' "$(sha256_file "$runtime_manifest")"
  printf 'RUNTIME_PIN_LENGTH=%s\n' "$(file_length "$runtime_manifest")"
  printf 'LLVM_CLOSURE_BEFORE_SHA256=%s\n' "$(sha256_file "$llvm_before")"
  printf 'LLVM_CLOSURE_AFTER_SHA256=%s\n' "$(sha256_file "$llvm_after")"
  printf 'LLVM_BUILD_ID_SHA256=%s\n' "$(sha256_file "$llvm_build_id_file")"
  printf 'GUARD_SOURCE_SHA256=%s\n' "$(sha256_file "$staged_guard_source")"
  printf 'GUARD_SOURCE_LENGTH=%s\n' "$(file_length "$staged_guard_source")"
  printf 'GUARD_ELF_SHA256=%s\n' "$(sha256_file "$retained_guard_elf")"
  printf 'GUARD_ELF_LENGTH=%s\n' "$(file_length "$retained_guard_elf")"
  printf 'GUARD_ELF_MODE=%s\n' \
    "$(/usr/bin/stat -Lc '%a' -- "$retained_guard_elf")"
  printf 'GUARD_READELF_SHA256=%s\n' "$(sha256_file "$retained_guard_readelf")"
  printf 'GUARD_DYNAMIC_SHA256=%s\n' "$(sha256_file "$retained_guard_dynamic")"
  printf 'GUARD_COMPILE_STDOUT_SHA256=%s\n' "$(sha256_file "$guard_compile_stdout")"
  printf 'GUARD_COMPILE_STDERR_SHA256=%s\n' "$(sha256_file "$guard_compile_stderr")"
  printf 'GUARD_STATUS_SHA256=%s\n' "$(sha256_file "$retained_guard_status")"
  printf 'GUARD_STATUS=passed\n'
  printf 'GUARD_MUTATION_JOURNAL=empty\n'
  printf 'GUARD_OVERFLOW=absent\n'
  printf 'GUARD_SELF_TEST_SHA256=%s\n' "$(sha256_file "$retained_guard_test")"
  printf 'GUARD_SELF_TEST=passed\n'
  printf 'CTEST_POLICY=passed-inside-guarded-build-closure\n'
  printf 'LANDLOCK_FILESYSTEM_ENFORCEMENT=passed\n'
  printf 'LANDLOCK_ABI=4\n'
  printf 'LANDLOCK_HANDLED_FS_RIGHTS=0x7fff\n'
  printf 'LANDLOCK_MAKE_SYM=handled-and-denied-in-writable-roots\n'
  printf 'NETWORK_IPC_ISOLATION=provided-by-seccomp-deny-policy-v1\n'
  printf 'SECCOMP_X32_TAGGED_SYSCALLS=denied-with-EPERM-before-table-v1\n'
  printf 'NETWORK_NAMESPACE_ISOLATION=not_provided\n'
  printf 'PROCESS_ISOLATION=not_provided\n'
  printf 'PROCESS_CREATION=allowed-required-subprocesses-inherit-policy\n'
  printf 'INHERITED_AMBIENT_DESCRIPTORS=closed-before-child-exec\n'
  printf 'AMBIENT_TMP_ACCESS=landlock-open-denied-with-partial-libc-metadata-redirect\n'
  printf 'TMP_METADATA_REDIRECT=partial-reviewed-libc-symbol-interposition\n'
  printf 'DIRECT_TMP_SYSCALL_REDIRECTION=not_provided\n'
  printf 'GLOBAL_TMP_FILE_OPEN=landlock-denied\n'
  printf 'GLOBAL_TMP_METADATA_SYSCALLS=observational-only\n'
  printf 'RAW_TRACE_RETENTION=complete-bounded-replayable\n'
  printf 'RAW_TRACE_GLOBAL_FILE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_FILE_BOUND"
  printf 'RAW_TRACE_GLOBAL_BYTE_BOUND=%s\n' "$RAW_TRACE_GLOBAL_BYTE_BOUND"
  printf 'RAW_TRACE_FILES=%s\n' "$retained_raw_global_files"
  printf 'RAW_TRACE_TOTAL_BYTES=%s\n' "$retained_raw_global_bytes"
  emit_evidence_file RAW_TRACE_INDEX "$retained_raw_trace_index"
  emit_evidence_file RAW_TRACE_RETENTION_LEDGER \
    "$raw_trace_retention_ledger"
  emit_evidence_file TRACE_CHECK_SOURCE "$retained_trace_source"
  emit_evidence_file TRACE_CHECK_ELF "$retained_trace_elf"
  emit_evidence_file TRACE_CHECK_READELF "$retained_trace_readelf"
  emit_evidence_file TRACE_CHECK_DYNAMIC "$retained_trace_dynamic"
  emit_evidence_file TMP_REDIRECT_SOURCE "$retained_tmp_redirect_source"
  emit_evidence_file TMP_REDIRECT_LIBRARY "$retained_tmp_redirect_library"
  emit_evidence_file TMP_REDIRECT_READELF "$retained_tmp_redirect_readelf"
  emit_evidence_file TMP_REDIRECT_DYNAMIC "$retained_tmp_redirect_dynamic"
  emit_evidence_file BOOTSTRAP_TRACE_ALLOWLIST \
    "$retained_bootstrap_trace_allowlist"
  emit_evidence_file TRACE_ALLOWLIST "$retained_trace_allowlist"
  emit_evidence_file TRACE_CHECK_BOOTSTRAP_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.canonical.txt"
  emit_evidence_file TRACE_CHECK_BOOTSTRAP_INPUTS \
    "$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.inputs.txt"
  emit_evidence_file TRACE_CHECK_BOOTSTRAP_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.checked-raw.txt"
  emit_evidence_file TRACE_CHECK_BOOTSTRAP_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.replay.canonical.txt"
  emit_evidence_file TRACE_CHECK_BOOTSTRAP_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.trace-check-bootstrap.replay.inputs.txt"
  emit_evidence_file TMP_REDIRECT_BOOTSTRAP_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.canonical.txt"
  emit_evidence_file TMP_REDIRECT_BOOTSTRAP_INPUTS \
    "$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.inputs.txt"
  emit_evidence_file TMP_REDIRECT_BOOTSTRAP_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.checked-raw.txt"
  emit_evidence_file TMP_REDIRECT_BOOTSTRAP_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.replay.canonical.txt"
  emit_evidence_file TMP_REDIRECT_BOOTSTRAP_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.tmp-redirect-bootstrap.replay.inputs.txt"
  emit_evidence_file GUARD_BOOTSTRAP_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.guard-bootstrap.canonical.txt"
  emit_evidence_file GUARD_BOOTSTRAP_INPUTS \
    "$artifact_dir/fe2o3-host-lld.guard-bootstrap.inputs.txt"
  emit_evidence_file GUARD_BOOTSTRAP_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.guard-bootstrap.checked-raw.txt"
  emit_evidence_file GUARD_BOOTSTRAP_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.guard-bootstrap.replay.canonical.txt"
  emit_evidence_file GUARD_BOOTSTRAP_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.guard-bootstrap.replay.inputs.txt"
  emit_evidence_file CONFIGURE_TRACE_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.configure.canonical.txt"
  emit_evidence_file CONFIGURE_TRACE_INPUTS \
    "$artifact_dir/fe2o3-host-lld.configure.inputs.txt"
  emit_evidence_file CONFIGURE_TRACE_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.configure.checked-raw.txt"
  emit_evidence_file CONFIGURE_TRACE_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.configure.replay.canonical.txt"
  emit_evidence_file CONFIGURE_TRACE_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.configure.replay.inputs.txt"
  emit_evidence_file OBJECT_TRACE_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.object.canonical.txt"
  emit_evidence_file OBJECT_TRACE_INPUTS \
    "$artifact_dir/fe2o3-host-lld.object.inputs.txt"
  emit_evidence_file OBJECT_TRACE_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.object.checked-raw.txt"
  emit_evidence_file OBJECT_TRACE_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.object.replay.canonical.txt"
  emit_evidence_file OBJECT_TRACE_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.object.replay.inputs.txt"
  emit_evidence_file LINK_TRACE_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.link.canonical.txt"
  emit_evidence_file LINK_TRACE_INPUTS \
    "$artifact_dir/fe2o3-host-lld.link.inputs.txt"
  emit_evidence_file LINK_TRACE_CHECKED_RAW \
    "$artifact_dir/fe2o3-host-lld.link.checked-raw.txt"
  emit_evidence_file LINK_TRACE_REPLAY_CANONICAL \
    "$artifact_dir/fe2o3-host-lld.link.replay.canonical.txt"
  emit_evidence_file LINK_TRACE_REPLAY_INPUTS \
    "$artifact_dir/fe2o3-host-lld.link.replay.inputs.txt"
  printf 'TRACE_ADMISSION=observational-gap-detector\n'
  printf 'FINAL_ELF_SHA256=%s\n' "$artifact_tool_sha256"
  printf 'FINAL_ELF_LENGTH=%s\n' "$artifact_tool_length"
  printf 'FINAL_READELF_SHA256=%s\n' "$(sha256_file "$readelf_output")"
  printf 'FINAL_DYNAMIC_SHA256=%s\n' "$(sha256_file "$dynamic_output")"
  printf 'FINAL_IDENTITY_SHA256=%s\n' "$(sha256_file "$identity_output")"
  printf 'FINAL_IDENTITY_FORMAT=%s\n' "$identity_format"
  printf 'FINAL_LINK_PROTOCOL=%s\n' "$identity_protocol"
  printf 'FINAL_INPUT_PROTOCOL=%s\n' "$identity_input_protocol"
  printf 'FINAL_RESULT_PROTOCOL=%s\n' "$identity_result_protocol"
  printf 'FINAL_OUTPUT_STAGING=%s\n' "$identity_output_staging"
  printf 'IDENTITY_CONTRACT_SELF_TEST=passed\n'
  printf 'ARTIFACT_MANIFEST_SHA256=%s\n' "$(sha256_file "$artifact_manifest")"
  printf 'PROTOCOL_CTEST_REGISTRATION=verified-canonical-exactly-once\n'
  printf 'PROTOCOL_CTEST_EXECUTION=not-executed-inside-guarded-build-closure\n'
} >"$build_evidence_manifest"
/usr/bin/chmod 0444 "$artifact_manifest" "$identity_output" "$readelf_output" \
  "$dynamic_output" "$runtime_manifest" "$source_manifest" "$identity_stderr" \
  "$retained_guard_status" "$retained_build_inputs" "$retained_roots" \
  "$retained_guard_readelf" "$retained_guard_dynamic" "$retained_guard_test" \
  "$retained_ctest_policy" "$retained_ctest_policy_status" \
  "$build_evidence_manifest"

directory_identity_matches "$artifact_dir" "$artifact_identity" ||
  die 'artifact directory identity changed before publication'
declare -A expected_artifacts=(
  [fe2o3-host-lld]=555
  [fe2o3-host-lld.artifact-manifest.txt]=444
  [fe2o3-host-lld.build-evidence-manifest.txt]=444
  [fe2o3-host-lld.build-guard-status.txt]=444
  [fe2o3-host-lld.build-guard-test.txt]=444
  [fe2o3-host-lld.ctest-policy.sh]=444
  [fe2o3-host-lld.ctest-policy-status.txt]=444
  [fe2o3-host-lld.build-guard]=555
  [fe2o3-host-lld.build-guard-source.cpp]=444
  [fe2o3-host-lld.build-guard.dynamic.txt]=444
  [fe2o3-host-lld.build-guard.readelf.txt]=444
  [fe2o3-host-lld.build-bootstrap.sh]=444
  [fe2o3-host-lld.build-script.sh]=444
  [fe2o3-host-lld.bootstrap-measurement.txt]=444
  [fe2o3-host-lld.bootstrap-trace-allowlist.txt]=444
  [fe2o3-host-lld.build-inputs.pin]=444
  [fe2o3-host-lld.configure.canonical.txt]=444
  [fe2o3-host-lld.configure.checked-raw.txt]=444
  [fe2o3-host-lld.configure.inputs.txt]=444
  [fe2o3-host-lld.configure.replay.canonical.txt]=444
  [fe2o3-host-lld.configure.replay.inputs.txt]=444
  [fe2o3-host-lld.dynamic.txt]=444
  [fe2o3-host-lld.guard-bootstrap.canonical.txt]=444
  [fe2o3-host-lld.guard-bootstrap.checked-raw.txt]=444
  [fe2o3-host-lld.guard-bootstrap.inputs.txt]=444
  [fe2o3-host-lld.guard-bootstrap.replay.canonical.txt]=444
  [fe2o3-host-lld.guard-bootstrap.replay.inputs.txt]=444
  [fe2o3-host-lld.identity.stderr]=444
  [fe2o3-host-lld.identity.txt]=444
  [fe2o3-host-lld.link.canonical.txt]=444
  [fe2o3-host-lld.link.checked-raw.txt]=444
  [fe2o3-host-lld.link.inputs.txt]=444
  [fe2o3-host-lld.link.replay.canonical.txt]=444
  [fe2o3-host-lld.link.replay.inputs.txt]=444
  [fe2o3-host-lld.object.canonical.txt]=444
  [fe2o3-host-lld.object.checked-raw.txt]=444
  [fe2o3-host-lld.object.inputs.txt]=444
  [fe2o3-host-lld.object.replay.canonical.txt]=444
  [fe2o3-host-lld.object.replay.inputs.txt]=444
  [fe2o3-host-lld.raw-traces.txt]=444
  [fe2o3-host-lld.readelf.txt]=444
  [fe2o3-host-lld.roots.pin]=444
  [fe2o3-host-lld.source-manifest.txt]=444
  [fe2o3-host-lld.static-runtime-manifest.txt]=444
  [fe2o3-host-lld.tool-source.pin]=444
  [fe2o3-host-lld.tmp-redirect-source.cpp]=444
  [fe2o3-host-lld.tmp-redirect.so]=444
  [fe2o3-host-lld.tmp-redirect.dynamic.txt]=444
  [fe2o3-host-lld.tmp-redirect.readelf.txt]=444
  [fe2o3-host-lld.tmp-redirect-bootstrap.canonical.txt]=444
  [fe2o3-host-lld.tmp-redirect-bootstrap.checked-raw.txt]=444
  [fe2o3-host-lld.tmp-redirect-bootstrap.inputs.txt]=444
  [fe2o3-host-lld.tmp-redirect-bootstrap.replay.canonical.txt]=444
  [fe2o3-host-lld.tmp-redirect-bootstrap.replay.inputs.txt]=444
  [fe2o3-host-lld.trace-allowlist.txt]=444
  [fe2o3-host-lld.trace-check]=555
  [fe2o3-host-lld.trace-check-source.cpp]=444
  [fe2o3-host-lld.trace-check-bootstrap.canonical.txt]=444
  [fe2o3-host-lld.trace-check-bootstrap.checked-raw.txt]=444
  [fe2o3-host-lld.trace-check-bootstrap.inputs.txt]=444
  [fe2o3-host-lld.trace-check-bootstrap.replay.canonical.txt]=444
  [fe2o3-host-lld.trace-check-bootstrap.replay.inputs.txt]=444
  [fe2o3-host-lld.trace-check.dynamic.txt]=444
  [fe2o3-host-lld.trace-check.readelf.txt]=444
)
for artifact_name in "${!retained_raw_artifacts[@]}"; do
  expected_artifacts["$artifact_name"]=${retained_raw_artifacts["$artifact_name"]}
done
artifact_count=0
while IFS= read -r -d '' artifact; do
  artifact_name=${artifact##*/}
  [[ -n ${expected_artifacts["$artifact_name"]+present} ]] ||
    die "artifact directory contains an unexpected entry: $artifact_name"
  [[ -f "$artifact" && ! -L "$artifact" ]] ||
    die "artifact directory contains a non-regular entry: $artifact_name"
  [[ $(/usr/bin/stat -Lc '%a' -- "$artifact") == "${expected_artifacts["$artifact_name"]}" ]] ||
    die "artifact has the wrong mode: $artifact_name"
  ((artifact_count += 1))
done < <(/usr/bin/find "$artifact_dir" -mindepth 1 -maxdepth 1 -print0)
[[ $artifact_count -eq ${#expected_artifacts[@]} ]] ||
  die 'artifact directory is missing a retained output'
verify_file "$artifact_tool" "$artifact_tool_sha256" \
  "$artifact_tool_length" 'final host LLD artifact' 555
[[ $(sha256_file "$artifact_tool") == "$artifact_tool_sha256" ]] ||
  die 'final host LLD artifact changed after measurement'
verify_file "$retained_guard_status" "$guard_status_sha256" \
  "$guard_status_length" 'final receiver-retained build guard status' 444
[[ $(/usr/bin/stat -Lc '%d:%i:%s:%a' "/proc/self/fd/$guard_status_fd") == \
  "$guard_status_identity" &&
  $(sha256_file "/proc/self/fd/$guard_status_fd") == "$guard_status_sha256" ]] ||
  die 'build guard status descriptor changed before final evidence handoff'
exec {guard_status_fd}<&-
printf 'fe2o3 static host LLD build passed: %s\n' "$build_evidence_manifest"
