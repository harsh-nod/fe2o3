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

manifest_value() {
  local manifest=$1 key=$2 line value='' count=0
  while IFS= read -r line; do
    if [[ "$line" == "$key="* ]]; then
      value=${line#*=}
      ((count += 1))
    fi
  done <"$manifest"
  [[ $count -eq 1 && -n "$value" ]] ||
    die "$manifest does not contain exactly one nonempty $key row"
  printf '%s\n' "$value"
}

normalize_admitted_inputs() {
  local input=$1 output=$2 report=$3 build_location=$4 artifact_location=$5
  local line kind label path extra normalized_path suffix
  local format_rows=0 status_rows=0 data_rows=0 source_rows=0 build_rows=0
  local artifact_rows=0 build_id_rows=0 prenormalized_build_rows=0

  [[ -f "$input" && ! -L "$input" && ! -e "$output" && ! -L "$output" &&
    ! -e "$report" && ! -L "$report" ]] ||
    die 'admitted-input normalization paths are not fresh regular files'
  : >"$output"
  while IFS= read -r line; do
    IFS=$'\t' read -r kind label path extra <<<"$line"
    case "$kind" in
      FORMAT=fe2o3-static-host-lld-admitted-input-set-v1)
        [[ -z "$label$path$extra" ]] || die 'admitted-input format row has fields'
        ((format_rows += 1))
        printf '%s\n' "$line" >>"$output"
        ;;
      STATUS=measured-observational-admission)
        [[ -z "$label$path$extra" ]] || die 'admitted-input status row has fields'
        ((status_rows += 1))
        printf '%s\n' "$line" >>"$output"
        ;;
      F|K|N|R)
        [[ -n "$label" && -n "$path" && -z "$extra" &&
          "$label" =~ ^[A-Za-z0-9._+-]+$ &&
          "$path" != *$'\n'* && "$path" != *$'\r'* ]] ||
          die 'admitted-input set has a noncanonical data row'
        normalized_path=$path
        if [[ "$label" == llvm-build-id ]]; then
          [[ "$kind" == F && "$path" == "$llvm_build_id_file" ]] ||
            die 'LLVM build-ID admission does not name the supplied exact file'
          normalized_path=\$LLVM_BUILD_ID_FILE
          ((build_id_rows += 1))
        elif [[ "$path" == "$build_location" ]]; then
          normalized_path=\$BUILD_LOCATION
          ((build_rows += 1))
        elif [[ "$path" == "$build_location/"* ]]; then
          suffix=${path#"$build_location"/}
          normalized_path="\$BUILD_LOCATION/$suffix"
          ((build_rows += 1))
        elif [[ "$path" == "$artifact_location" ]]; then
          normalized_path=\$ARTIFACT_LOCATION
          ((artifact_rows += 1))
        elif [[ "$path" == "$artifact_location/"* ]]; then
          suffix=${path#"$artifact_location"/}
          normalized_path="\$ARTIFACT_LOCATION/$suffix"
          ((artifact_rows += 1))
        elif [[ "$path" == "$source_root" ]]; then
          normalized_path=\$SOURCE_ROOT
          ((source_rows += 1))
        elif [[ "$path" == "$source_root/"* ]]; then
          suffix=${path#"$source_root"/}
          normalized_path="\$SOURCE_ROOT/$suffix"
          ((source_rows += 1))
        elif [[ "$path" == \$BUILD || "$path" == \$BUILD/* ]]; then
          ((prenormalized_build_rows += 1))
        fi
        printf '%s\t%s\t%s\n' "$kind" "$label" "$normalized_path" >>"$output"
        ((data_rows += 1))
        ;;
      *) die "admitted-input set has an unknown row: $kind" ;;
    esac
  done <"$input"
  [[ $format_rows -eq 1 && $status_rows -eq 1 && $data_rows -gt 0 ]] ||
    die 'admitted-input set has noncanonical headers or no data'
  {
    printf 'FORMAT=fe2o3-static-host-lld-input-normalization-v1\n'
    printf 'STATUS=only-approved-location-substitutions\n'
    printf 'SOURCE_ROOT_SUBSTITUTIONS=%s\n' "$source_rows"
    printf 'BUILD_LOCATION_SUBSTITUTIONS=%s\n' "$build_rows"
    printf 'ARTIFACT_LOCATION_SUBSTITUTIONS=%s\n' "$artifact_rows"
    printf 'LLVM_BUILD_ID_FILE_SUBSTITUTIONS=%s\n' "$build_id_rows"
    printf 'PRENORMALIZED_BUILD_ROWS=%s\n' "$prenormalized_build_rows"
    printf 'DATA_ROWS=%s\n' "$data_rows"
  } >"$report"
  /usr/bin/chmod 0444 "$output" "$report"
}

assert_run_evidence() {
  local artifact_root=$1 artifact_manifest build_evidence tool digest length
  artifact_manifest="$artifact_root/fe2o3-host-lld.artifact-manifest.txt"
  build_evidence="$artifact_root/fe2o3-host-lld.build-evidence-manifest.txt"
  tool="$artifact_root/fe2o3-host-lld"
  [[ -f "$artifact_manifest" && ! -L "$artifact_manifest" &&
    -f "$build_evidence" && ! -L "$build_evidence" ]] ||
    die 'a fresh build omitted its separate evidence manifests'
  digest=$(sha256_file "$tool")
  length=$(/usr/bin/stat -Lc '%s' -- "$tool")
  [[ $(manifest_value "$artifact_manifest" FORMAT) == fe2o3-host-lld-artifact-v1 &&
    $(manifest_value "$artifact_manifest" TOOL_SHA256) == "$digest" &&
    $(manifest_value "$artifact_manifest" TOOL_LENGTH) == "$length" &&
    $(manifest_value "$artifact_manifest" TOOL_MODE) == 555 &&
    $(manifest_value "$artifact_manifest" AUTHORITY) == none &&
    $(manifest_value "$artifact_manifest" GPU_LINKER) == unchanged &&
    $(manifest_value "$artifact_manifest" COMGR) == absent ]] ||
    die 'a fresh build artifact manifest is not internally exact'
  [[ $(manifest_value "$build_evidence" GUARD_SELF_TEST) == passed &&
    $(manifest_value "$build_evidence" PROTOCOL_CTEST_REGISTRATION) == \
      verified-canonical-exactly-once &&
    $(manifest_value "$build_evidence" PROTOCOL_CTEST_EXECUTION) == \
      not-executed-inside-guarded-build-closure ]] ||
    die 'a fresh build evidence manifest has incorrect test semantics'
  ! /usr/bin/grep -q '^TEST_STATUS=' "$build_evidence" ||
    die 'a fresh build evidence manifest retained the ambiguous test claim'
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

for artifact in fe2o3-host-lld fe2o3-host-lld.identity.txt \
    fe2o3-host-lld.source-manifest.txt \
    fe2o3-host-lld.static-runtime-manifest.txt fe2o3-host-lld.readelf.txt \
    fe2o3-host-lld.dynamic.txt fe2o3-host-lld.build-inputs.pin \
    fe2o3-host-lld.roots.pin fe2o3-host-lld.tool-source.pin \
    fe2o3-host-lld.build-bootstrap.sh \
    fe2o3-host-lld.ctest-policy.sh \
    fe2o3-host-lld.ctest-policy-status.txt \
    fe2o3-host-lld.build-guard-source.cpp \
    fe2o3-host-lld.trace-check-source.cpp \
    fe2o3-host-lld.tmp-redirect-source.cpp; do
  /usr/bin/cmp -s -- "$work_root/artifact-a/$artifact" \
    "$work_root/artifact-b/$artifact" ||
    die "two fresh repeated builds differ for $artifact"
done

assert_run_evidence "$work_root/artifact-a"
assert_run_evidence "$work_root/artifact-b"

for phase in configure object link; do
  normalized_a="$work_root/$phase.inputs.semantic-a.txt"
  normalized_b="$work_root/$phase.inputs.semantic-b.txt"
  report_a="$work_root/$phase.inputs.normalization-a.txt"
  report_b="$work_root/$phase.inputs.normalization-b.txt"
  normalize_admitted_inputs \
    "$work_root/artifact-a/fe2o3-host-lld.$phase.inputs.txt" \
    "$normalized_a" "$report_a" "$work_root/build-a" "$work_root/artifact-a"
  normalize_admitted_inputs \
    "$work_root/artifact-b/fe2o3-host-lld.$phase.inputs.txt" \
    "$normalized_b" "$report_b" "$work_root/build-b" "$work_root/artifact-b"
  /usr/bin/cmp -s -- "$normalized_a" "$normalized_b" ||
    die "fresh $phase admitted-input identities differ semantically"
  /usr/bin/cmp -s -- "$report_a" "$report_b" ||
    die "fresh $phase inputs require different location substitutions"
  source_substitutions=$(manifest_value "$report_a" SOURCE_ROOT_SUBSTITUTIONS)
  build_id_substitutions=$(manifest_value \
    "$report_a" LLVM_BUILD_ID_FILE_SUBSTITUTIONS)
  [[ "$source_substitutions" =~ ^[1-9][0-9]*$ ]] ||
    die "$phase inputs did not prove source-root normalization"
  if [[ "$phase" == configure ]]; then
    [[ "$build_id_substitutions" == 1 ]] ||
      die 'configure inputs did not prove the exact build-ID location substitution'
  else
    [[ "$build_id_substitutions" == 0 ]] ||
      die "$phase inputs unexpectedly admitted the LLVM build-ID file"
  fi
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
printf 'artifact_a_manifest_sha256=%s\n' "$(sha256_file "$manifest")"
printf 'artifact_b_manifest_sha256=%s\n' \
  "$(sha256_file "$work_root/artifact-b/fe2o3-host-lld.artifact-manifest.txt")"
printf 'evidence_a=%s\n' \
  "$work_root/artifact-a/fe2o3-host-lld.build-evidence-manifest.txt"
printf 'evidence_b=%s\n' \
  "$work_root/artifact-b/fe2o3-host-lld.build-evidence-manifest.txt"
printf 'secure_a_trace=%s\n' "$work_root/secure-a"
printf 'secure_b_trace=%s\n' "$work_root/secure-b"
printf 'work_root=%s\n' "$work_root"
