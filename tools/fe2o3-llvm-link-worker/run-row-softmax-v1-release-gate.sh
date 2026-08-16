#!/usr/bin/env bash
set -euo pipefail

readonly MANIFEST_REPOSITORY_PATH="tools/fe2o3-llvm-link-worker/row-softmax-v1-release-manifest.txt"
readonly PROVIDER_FILES=(
  ocml.bc
  oclc_isa_version_942.bc
  oclc_unsafe_math_off.bc
  oclc_finite_only_off.bc
)

die() {
  printf 'row-softmax release gate: %s\n' "$*" >&2
  exit 1
}

usage() {
  printf 'usage: MANIFEST_PATH=/absolute/path EXPECTED_MANIFEST_SHA256=<sha256> %s ABSOLUTE_NEW_CMAKE_BUILD_DIR ABSOLUTE_NEW_CARGO_TARGET_DIR\n' "$0" >&2
  exit 2
}

sha256_file() {
  /usr/bin/sha256sum -- "$1" | { read -r digest _; printf '%s\n' "$digest"; }
}

file_length() {
  /usr/bin/stat -Lc '%s' -- "$1"
}

file_matches() {
  local path=$1 expected_sha256=$2 expected_length=$3
  [[ -f "$path" && ! -L "$path" ]] || return 1
  [[ $(file_length "$path") == "$expected_length" ]] || return 1
  [[ $(sha256_file "$path") == "$expected_sha256" ]]
}

verify_file() {
  local path=$1 expected_sha256=$2 expected_length=$3 label=$4
  file_matches "$path" "$expected_sha256" "$expected_length" ||
    die "$label differs from the operator-selected reviewed manifest: $path"
}

canonical_existing_directory() {
  local path=$1 label=$2
  [[ "$path" == /* && -d "$path" && ! -L "$path" ]] ||
    die "$label is not an absolute, non-symlink directory: $path"
  local canonical
  canonical=$(/usr/bin/readlink -f -- "$path")
  [[ "$canonical" == "$path" ]] || die "$label is not canonical: $path"
  printf '%s\n' "$canonical"
}

prepare_fresh_directory() {
  local path=$1 label=$2
  [[ "$path" == /* && "$path" != */ && ! -e "$path" && ! -L "$path" ]] ||
    die "$label must be an absolute, nonexistent, non-symlink path: $path"
  local parent base canonical_parent canonical
  parent=$(/usr/bin/dirname -- "$path")
  base=$(/usr/bin/basename -- "$path")
  [[ "$base" != . && "$base" != .. ]] || die "$label has an invalid basename"
  canonical_parent=$(/usr/bin/readlink -f -- "$parent")
  [[ -d "$canonical_parent" && ! -L "$parent" ]] ||
    die "$label parent is not a non-symlink directory: $parent"
  canonical="$canonical_parent/$base"
  [[ "$canonical" == "$path" ]] || die "$label is not canonical: $path"
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
  local root=$1 format=$2 output=$3
  {
    printf '%s\n' "$format"
    printf 'root=%s\n' "$root"
    while IFS= read -r -d '' path; do
      local relative
      relative=${path#"$root"/}
      [[ "$relative" != *$'\n'* && "$relative" != *$'\r'* &&
        "$relative" != *$'\t'* ]] || die "LLVM package contains a noncanonical path"
      if [[ -L "$path" ]]; then
        local target
        target=$(/usr/bin/readlink -- "$path")
        [[ "$target" != *$'\n'* && "$target" != *$'\r'* &&
          "$target" != *$'\t'* ]] || die "LLVM package symlink is noncanonical"
        printf 'L\t%s\t%s\n' "$relative" "$target"
      else
        printf 'F\t%s\t%s\t%s\n' "$relative" "$(file_length "$path")" \
          "$(sha256_file "$path")"
      fi
    done < <(
      /usr/bin/find "$root" -mindepth 1 \( -type f -o -type l \) -print0 |
        /usr/bin/sort -z
    )
  } >"$output"
}

verify_tree_closure() {
  local root=$1 format=$2 expected=$3 observed=$4 label=$5
  generate_tree_manifest "$root" "$format" "$observed"
  /usr/bin/cmp -s -- "$observed" "$expected" ||
    die "$label closure differs from its reviewed manifest"
}

generate_runtime_provider_manifest() {
  local worker=$1 requested_provider=$2 canonical_provider=$3 output=$4 scratch=$5
  local ldd_output="$scratch/worker.ldd"
  local dso_paths="$scratch/worker.dso-paths"
  env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin /usr/bin/ldd "$worker" >"$ldd_output"
  : >"$dso_paths"
  while IFS= read -r line; do
    local path=''
    if [[ "$line" =~ \=\>[[:space:]]+(/[^[:space:]]+) ]]; then
      path=${BASH_REMATCH[1]}
    elif [[ "$line" =~ ^[[:space:]]*(/[^[:space:]]+) ]]; then
      path=${BASH_REMATCH[1]}
    fi
    if [[ -n "$path" ]]; then
      /usr/bin/readlink -f -- "$path" >>"$dso_paths"
    fi
  done <"$ldd_output"
  LC_ALL=C /usr/bin/sort -u -o "$dso_paths" "$dso_paths"
  {
    printf 'fe2o3-row-softmax-runtime-provider-v1\n'
    printf 'provider-requested=%s\n' "$requested_provider"
    printf 'provider-canonical=%s\n' "$canonical_provider"
    local provider_file canonical_file
    for provider_file in "${PROVIDER_FILES[@]}"; do
      canonical_file=$(/usr/bin/readlink -f -- "$requested_provider/$provider_file")
      printf 'P\t%s\t%s\t%s\n' "$canonical_file" \
        "$(file_length "$canonical_file")" "$(sha256_file "$canonical_file")"
    done
    local dso
    while IFS= read -r dso; do
      [[ -f "$dso" ]] || die "runtime DSO is not a regular file: $dso"
      printf 'D\t%s\t%s\t%s\n' "$dso" "$(file_length "$dso")" \
        "$(sha256_file "$dso")"
    done <"$dso_paths"
  } >"$output"
}

verify_cmake_source() {
  local cache=$1 expected_source=$2
  /usr/bin/grep -Fqx "CMAKE_HOME_DIRECTORY:INTERNAL=$expected_source" "$cache"
}

verify_source_state() {
  local git=$1 repo_root=$2 implementation_commit=$3 implementation_tree=$4
  [[ -z $(env -i HOME="$HOME" LC_ALL=C PATH=/usr/bin:/bin \
    "$git" -C "$repo_root" status --porcelain=v1 --untracked-files=all) ]] ||
    die "release source checkout is not clean"
  local committed_tree parent commit_count
  committed_tree=$("$git" -C "$repo_root" rev-parse "${implementation_commit}^{tree}")
  parent=$("$git" -C "$repo_root" rev-parse HEAD^)
  commit_count=$("$git" -C "$repo_root" rev-list --count "$implementation_commit..HEAD")
  [[ "$committed_tree" == "$implementation_tree" ]] ||
    die "implementation tree pin does not match its commit"
  [[ "$parent" == "$implementation_commit" ]] ||
    die "release manifest commit is not directly based on implementation Commit A"
  [[ "$commit_count" == 1 ]] ||
    die "release checkout contains commits beyond manifest-only Commit B"
  local changed
  changed=$("$git" -C "$repo_root" diff --name-only "$implementation_commit..HEAD")
  [[ "$changed" == "$MANIFEST_REPOSITORY_PATH" ]] ||
    die "Commit B differs from Commit A by more than the release manifest"
}

[[ $# -eq 2 ]] || usage
readonly build_dir=$1
readonly cargo_target_dir=$2
readonly manifest_path=${MANIFEST_PATH:-}
readonly expected_manifest_sha256=${EXPECTED_MANIFEST_SHA256:-}
[[ -n "$manifest_path" && -n "$expected_manifest_sha256" ]] || usage
[[ "$manifest_path" == /* && -f "$manifest_path" && ! -L "$manifest_path" ]] ||
  die "MANIFEST_PATH must name an absolute regular non-symlink file"
[[ "$expected_manifest_sha256" =~ ^[0-9a-f]{64}$ ]] ||
  die "EXPECTED_MANIFEST_SHA256 is not canonical lowercase SHA-256"
[[ $(sha256_file "$manifest_path") == "$expected_manifest_sha256" ]] ||
  die "caller-supplied release manifest digest does not match MANIFEST_PATH"
[[ $(/usr/bin/tail -c 1 -- "$manifest_path" | /usr/bin/od -An -tuC | /usr/bin/tr -d ' ') == 10 ]] ||
  die "release manifest is not newline terminated"
if LC_ALL=C /usr/bin/grep -n '[^ -~]' "$manifest_path" >/dev/null; then
  die "release manifest contains noncanonical characters"
fi

readonly manifest_keys=(
  FORMAT PROFILE STATUS MANIFEST_REPOSITORY_PATH
  IMPLEMENTATION_COMMIT IMPLEMENTATION_TREE
  LLVM_SOURCE_DIR LLVM_COMMIT LLVM_TREE
  LLVM_PACKAGE_ROOT LLVM_PACKAGE_MANIFEST_PATH LLVM_PACKAGE_MANIFEST_SHA256
  LLVM_PACKAGE_MANIFEST_LENGTH LLVM_DIR LLD_DIR LLVM_PACKAGE_VERSION
  LLVM_BUILD_ID LLVM_BUILD_ID_FILE LLVM_BUILD_ID_FILE_SHA256
  LLVM_BUILD_ID_FILE_LENGTH
  CMAKE_PATH CMAKE_SHA256 CMAKE_LENGTH CMAKE_VERSION
  CTEST_PATH CTEST_SHA256 CTEST_LENGTH CTEST_VERSION
  NINJA_PATH NINJA_SHA256 NINJA_LENGTH NINJA_VERSION
  CXX_PATH CXX_SHA256 CXX_LENGTH CXX_VERSION
  CARGO_PATH CARGO_SHA256 CARGO_LENGTH CARGO_VERSION
  CARGO_LOCK_SHA256 CARGO_LOCK_LENGTH
  CARGO_VENDOR_ROOT CARGO_VENDOR_MANIFEST_PATH CARGO_VENDOR_MANIFEST_SHA256
  CARGO_VENDOR_MANIFEST_LENGTH
  RUSTC_PATH RUSTC_SHA256 RUSTC_LENGTH RUSTC_VERSION RUSTC_SYSROOT
  RUSTC_SYSROOT_MANIFEST_PATH RUSTC_SYSROOT_MANIFEST_SHA256
  RUSTC_SYSROOT_MANIFEST_LENGTH
  DEVICE_LIBRARY_REQUESTED_DIR DEVICE_LIBRARY_CANONICAL_DIR
  OCML_SHA256 ISA942_SHA256 UNSAFE_MATH_OFF_SHA256 FINITE_ONLY_OFF_SHA256
  WORKER_BUILD_CLAIM WORKER_SHA256 WORKER_LENGTH
  PROBE_SHA256 PROBE_LENGTH PROBE_STDOUT_SHA256 PROBE_STDOUT_LENGTH
  RUNTIME_PROVIDER_MANIFEST_PATH RUNTIME_PROVIDER_MANIFEST_SHA256
  RUNTIME_PROVIDER_MANIFEST_LENGTH HSACO_SHA256 HSACO_LENGTH
)
mapfile -t manifest_lines <"$manifest_path"
[[ ${#manifest_lines[@]} -eq ${#manifest_keys[@]} ]] ||
  die "release manifest has the wrong number of fixed fields"
declare -A manifest=()
for index in "${!manifest_keys[@]}"; do
  key=${manifest_keys[$index]}
  line=${manifest_lines[$index]}
  [[ "$line" == "$key="* ]] || die "release manifest field $index is not $key"
  value=${line#*=}
  [[ -n "$value" && "$value" != *'='* && "$value" != ' '* &&
    "$value" != *' ' ]] || die "release manifest value for $key is noncanonical"
  manifest[$key]=$value
done

[[ ${manifest[FORMAT]} == fe2o3-row-softmax-v1-release-manifest-v1 &&
  ${manifest[PROFILE]} == row-softmax-v1-gfx942-cov6-llvm22-v1 ]] ||
  die "release manifest profile is not the exact row-softmax V1 profile"
[[ ${manifest[STATUS]} == ready ]] ||
  die "operator-selected release manifest is explicitly blocked"
[[ ${manifest[MANIFEST_REPOSITORY_PATH]} == "$MANIFEST_REPOSITORY_PATH" ]] ||
  die "release manifest repository path is not canonical"
for key in IMPLEMENTATION_COMMIT LLVM_COMMIT; do
  [[ ${manifest[$key]} =~ ^[0-9a-f]{40}$ ]] || die "$key is not a canonical commit"
done
for key in IMPLEMENTATION_TREE LLVM_TREE; do
  [[ ${manifest[$key]} =~ ^[0-9a-f]{40}$ ]] || die "$key is not a canonical tree"
done
for key in LLVM_PACKAGE_MANIFEST_SHA256 LLVM_BUILD_ID_FILE_SHA256 CMAKE_SHA256 \
  CTEST_SHA256 NINJA_SHA256 CXX_SHA256 CARGO_SHA256 RUSTC_SHA256 OCML_SHA256 \
  ISA942_SHA256 UNSAFE_MATH_OFF_SHA256 FINITE_ONLY_OFF_SHA256 WORKER_SHA256 \
  PROBE_SHA256 PROBE_STDOUT_SHA256 RUNTIME_PROVIDER_MANIFEST_SHA256 HSACO_SHA256 \
  CARGO_LOCK_SHA256 CARGO_VENDOR_MANIFEST_SHA256 RUSTC_SYSROOT_MANIFEST_SHA256; do
  [[ ${manifest[$key]} =~ ^[0-9a-f]{64}$ ]] || die "$key is not canonical SHA-256"
done
for key in LLVM_PACKAGE_MANIFEST_LENGTH LLVM_BUILD_ID_FILE_LENGTH CMAKE_LENGTH \
  CTEST_LENGTH NINJA_LENGTH CXX_LENGTH CARGO_LENGTH RUSTC_LENGTH WORKER_LENGTH \
  PROBE_LENGTH PROBE_STDOUT_LENGTH RUNTIME_PROVIDER_MANIFEST_LENGTH HSACO_LENGTH \
  CARGO_LOCK_LENGTH CARGO_VENDOR_MANIFEST_LENGTH RUSTC_SYSROOT_MANIFEST_LENGTH; do
  [[ ${manifest[$key]} =~ ^(0|[1-9][0-9]*)$ ]] || die "$key is not a canonical length"
done
[[ ${manifest[LLVM_COMMIT]} == ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 &&
  ${manifest[LLVM_TREE]} == 1e4fdb95266974a0cbca9ec4c6f740488322f238 &&
  ${manifest[LLVM_PACKAGE_VERSION]} == 22.1.8 &&
  ${manifest[LLVM_BUILD_ID]} == upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 ]] ||
  die "LLVM source/package identity differs from the reviewed LLVM 22 closure"
[[ ${manifest[CARGO_SHA256]} == c9ad606cb1dbb4a65aa27c80be88ed61eb2b811b6450eeec6794f60ed78b94a3 &&
  ${manifest[CARGO_VERSION]} == "cargo 1.96.0-nightly (888f67534 2026-03-30)" &&
  ${manifest[RUSTC_SHA256]} == 08dfef109ad22d90556dbd2f964543cd93843dcd75a2e9792c173667392a1950 &&
  ${manifest[RUSTC_VERSION]} == "rustc 1.96.0-nightly (55e86c996 2026-04-02)" ]] ||
  die "Cargo/rustc identities differ from nightly-2026-04-03"
[[ ${manifest[OCML_SHA256]} == cfe97fe9ee29379f522e5f20ae55aae1cdb96eb41d6aa250ea11c4941c54e019 &&
  ${manifest[ISA942_SHA256]} == 580d540cc738c0f9554c8710575bbc9b51ebacdcbc29aa0074ed05d3691dea1d &&
  ${manifest[UNSAFE_MATH_OFF_SHA256]} == 22c799b9154389f050f8f3368762636b9954a2ea25622199c359366bbd84657f &&
  ${manifest[FINITE_ONLY_OFF_SHA256]} == f3138eeee65c1d83234260728d124f635f021abb37c495f4ed027dfe92bcb1dd ]] ||
  die "gfx942 provider identities differ from the externally reviewed closure"
[[ ${manifest[WORKER_BUILD_CLAIM]} =~ ^fe2o3-worker-v1-sha256-[0-9a-f]{64}$ ]] ||
  die "worker build claim is malformed"

script_dir=$(/usr/bin/readlink -f -- "$(/usr/bin/dirname -- "${BASH_SOURCE[0]}")")
readonly script_dir
repo_root=$(/usr/bin/readlink -f -- "$script_dir/../..")
readonly repo_root
readonly checked_manifest=$repo_root/$MANIFEST_REPOSITORY_PATH
[[ $(/usr/bin/readlink -f -- "$manifest_path") == "$checked_manifest" ]] ||
  die "MANIFEST_PATH is not the manifest committed by Commit B"
readonly git=/usr/bin/git
verify_source_state "$git" "$repo_root" "${manifest[IMPLEMENTATION_COMMIT]}" \
  "${manifest[IMPLEMENTATION_TREE]}"
verify_file "$manifest_path" "$expected_manifest_sha256" "$(file_length "$manifest_path")" \
  "release manifest"
readonly cargo_lock=$repo_root/Cargo.lock
verify_file "$cargo_lock" "${manifest[CARGO_LOCK_SHA256]}" \
  "${manifest[CARGO_LOCK_LENGTH]}" "Cargo.lock"

llvm_source=$(canonical_existing_directory "${manifest[LLVM_SOURCE_DIR]}" "LLVM source")
readonly llvm_source
[[ -z $("$git" -C "$llvm_source" status --porcelain=v1 --untracked-files=all) &&
  $("$git" -C "$llvm_source" rev-parse HEAD) == "${manifest[LLVM_COMMIT]}" &&
  $("$git" -C "$llvm_source" rev-parse 'HEAD^{tree}') == "${manifest[LLVM_TREE]}" ]] ||
  die "LLVM source checkout differs from the reviewed commit/tree"
llvm_package_root=$(canonical_existing_directory \
  "${manifest[LLVM_PACKAGE_ROOT]}" "LLVM package root")
readonly llvm_package_root
llvm_dir=$(canonical_existing_directory "${manifest[LLVM_DIR]}" "LLVM_DIR")
readonly llvm_dir
lld_dir=$(canonical_existing_directory "${manifest[LLD_DIR]}" "LLD_DIR")
readonly lld_dir
[[ "$llvm_dir" == "$llvm_package_root"/* && "$lld_dir" == "$llvm_package_root"/* ]] ||
  die "LLVM/LLD CMake packages are outside the reviewed package root"
verify_file "${manifest[LLVM_PACKAGE_MANIFEST_PATH]}" \
  "${manifest[LLVM_PACKAGE_MANIFEST_SHA256]}" "${manifest[LLVM_PACKAGE_MANIFEST_LENGTH]}" \
  "LLVM package manifest"
verify_file "${manifest[LLVM_BUILD_ID_FILE]}" "${manifest[LLVM_BUILD_ID_FILE_SHA256]}" \
  "${manifest[LLVM_BUILD_ID_FILE_LENGTH]}" "LLVM build-ID file"
[[ $(/usr/bin/tr -d '\n' <"${manifest[LLVM_BUILD_ID_FILE]}") == "${manifest[LLVM_BUILD_ID]}" ]] ||
  die "LLVM build-ID file content differs"

for tool in CMAKE CTEST NINJA CXX CARGO RUSTC; do
  verify_file "${manifest[${tool}_PATH]}" "${manifest[${tool}_SHA256]}" \
    "${manifest[${tool}_LENGTH]}" "$tool executable"
done
[[ $("${manifest[CMAKE_PATH]}" --version | /usr/bin/head -n 1) == "${manifest[CMAKE_VERSION]}" &&
  $("${manifest[CTEST_PATH]}" --version | /usr/bin/head -n 1) == "${manifest[CTEST_VERSION]}" &&
  $("${manifest[NINJA_PATH]}" --version) == "${manifest[NINJA_VERSION]}" &&
  $("${manifest[CXX_PATH]}" --version | /usr/bin/head -n 1) == "${manifest[CXX_VERSION]}" &&
  $("${manifest[CARGO_PATH]}" --version) == "${manifest[CARGO_VERSION]}" &&
  $("${manifest[RUSTC_PATH]}" --version) == "${manifest[RUSTC_VERSION]}" ]] ||
  die "one or more pinned tool versions differ from the manifest"
rustc_sysroot=$(canonical_existing_directory "${manifest[RUSTC_SYSROOT]}" "rustc sysroot")
readonly rustc_sysroot
[[ $("${manifest[RUSTC_PATH]}" --print sysroot) == "$rustc_sysroot" ]] ||
  die "rustc reports a sysroot outside the reviewed closure"
verify_file "${manifest[RUSTC_SYSROOT_MANIFEST_PATH]}" \
  "${manifest[RUSTC_SYSROOT_MANIFEST_SHA256]}" \
  "${manifest[RUSTC_SYSROOT_MANIFEST_LENGTH]}" "rustc sysroot manifest"
cargo_vendor_root=$(canonical_existing_directory "${manifest[CARGO_VENDOR_ROOT]}" \
  "Cargo vendor root")
readonly cargo_vendor_root
[[ "$cargo_vendor_root" =~ ^/[A-Za-z0-9._/-]+$ ]] ||
  die "Cargo vendor root cannot be encoded canonically in generated Cargo config"
verify_file "${manifest[CARGO_VENDOR_MANIFEST_PATH]}" \
  "${manifest[CARGO_VENDOR_MANIFEST_SHA256]}" \
  "${manifest[CARGO_VENDOR_MANIFEST_LENGTH]}" "Cargo vendor manifest"

readonly requested_provider=${manifest[DEVICE_LIBRARY_REQUESTED_DIR]}
[[ "$requested_provider" == /* && -d "$requested_provider" ]] ||
  die "requested gfx942 provider path is unavailable"
canonical_provider=$(/usr/bin/readlink -f -- "$requested_provider")
readonly canonical_provider
[[ "$canonical_provider" == "${manifest[DEVICE_LIBRARY_CANONICAL_DIR]}" ]] ||
  die "gfx942 provider symlink resolves outside the reviewed directory"
readonly provider_hashes=(
  "${manifest[OCML_SHA256]}"
  "${manifest[ISA942_SHA256]}"
  "${manifest[UNSAFE_MATH_OFF_SHA256]}"
  "${manifest[FINITE_ONLY_OFF_SHA256]}"
)
for index in "${!PROVIDER_FILES[@]}"; do
  provider_path=$requested_provider/${PROVIDER_FILES[$index]}
  [[ -f "$provider_path" ]] || die "gfx942 provider file is absent: $provider_path"
  [[ $(sha256_file "$provider_path") == "${provider_hashes[$index]}" ]] ||
    die "gfx942 provider file digest differs: $provider_path"
done
verify_file "${manifest[RUNTIME_PROVIDER_MANIFEST_PATH]}" \
  "${manifest[RUNTIME_PROVIDER_MANIFEST_SHA256]}" \
  "${manifest[RUNTIME_PROVIDER_MANIFEST_LENGTH]}" "runtime/provider manifest"

case "$build_dir:$cargo_target_dir" in
  /*:/*) ;;
  *) usage ;;
esac
[[ "$build_dir" != "$cargo_target_dir" && "$build_dir" != "$cargo_target_dir"/* &&
  "$cargo_target_dir" != "$build_dir"/* ]] || die "build and Cargo target directories overlap"
for output_dir in "$build_dir" "$cargo_target_dir"; do
  [[ "$output_dir" != "$repo_root"/* && "$output_dir" != "$llvm_package_root"/* ]] ||
    die "release output directory overlaps reviewed source/package inputs"
done
build_identity=$(prepare_fresh_directory "$build_dir" "CMake build directory")
readonly build_identity
cargo_target_identity=$(prepare_fresh_directory "$cargo_target_dir" "Cargo target directory")
readonly cargo_target_identity
readonly observed_package_before=$build_dir/observed-llvm-package-before.txt
readonly observed_package_after=$build_dir/observed-llvm-package-after.txt
readonly observed_vendor_before=$build_dir/observed-cargo-vendor-before.txt
readonly observed_vendor_after=$build_dir/observed-cargo-vendor-after.txt
readonly observed_sysroot_before=$build_dir/observed-rustc-sysroot-before.txt
readonly observed_sysroot_after=$build_dir/observed-rustc-sysroot-after.txt
verify_tree_closure "$llvm_package_root" fe2o3-llvm-package-closure-v1 \
  "${manifest[LLVM_PACKAGE_MANIFEST_PATH]}" "$observed_package_before" "LLVM package"
verify_tree_closure "$cargo_vendor_root" fe2o3-cargo-vendor-closure-v1 \
  "${manifest[CARGO_VENDOR_MANIFEST_PATH]}" "$observed_vendor_before" "Cargo vendor"
verify_tree_closure "$rustc_sysroot" fe2o3-rustc-sysroot-closure-v1 \
  "${manifest[RUSTC_SYSROOT_MANIFEST_PATH]}" "$observed_sysroot_before" "rustc sysroot"

readonly cmake=${manifest[CMAKE_PATH]}
readonly ctest=${manifest[CTEST_PATH]}
readonly ninja=${manifest[NINJA_PATH]}
readonly cxx=${manifest[CXX_PATH]}
env -i HOME="$HOME" LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  "$cmake" -S "$script_dir" -B "$build_dir" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_MAKE_PROGRAM="$ninja" \
  -DCMAKE_CXX_COMPILER="$cxx" -DBUILD_TESTING=ON \
  -DLLVM_DIR="$llvm_dir" -DLLD_DIR="$lld_dir" \
  -DFE2O3_PINNED_LLVM_VERSION="${manifest[LLVM_PACKAGE_VERSION]}" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="${manifest[LLVM_BUILD_ID]}" \
  -DFE2O3_LLVM_BUILD_ID_FILE="${manifest[LLVM_BUILD_ID_FILE]}" \
  -DFE2O3_GFX942_DEVICE_LIB_DIR="$requested_provider" \
  -DFE2O3_ROW_SOFTMAX_RELEASE_GATE=ON \
  -DFE2O3_EXPECTED_GFX942_DEVICE_LIB_CANONICAL_DIR="$canonical_provider" \
  -DFE2O3_EXPECTED_GFX942_OCML_SHA256="${manifest[OCML_SHA256]}" \
  -DFE2O3_EXPECTED_GFX942_ISA_SHA256="${manifest[ISA942_SHA256]}" \
  -DFE2O3_EXPECTED_GFX942_UNSAFE_MATH_SHA256="${manifest[UNSAFE_MATH_OFF_SHA256]}" \
  -DFE2O3_EXPECTED_GFX942_FINITE_ONLY_SHA256="${manifest[FINITE_ONLY_OFF_SHA256]}" \
  -DFE2O3_EXPECTED_WORKER_BUILD_ID="${manifest[WORKER_BUILD_CLAIM]}"
readonly cache=$build_dir/CMakeCache.txt
verify_cmake_source "$cache" "$script_dir" || die "CMake configured a substituted source tree"
env -i HOME="$HOME" LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  "$cmake" --build "$build_dir" --target fe2o3-llvm-link-worker \
  fe2o3-worker-pipeline-tests fe2o3-row-softmax-llvm22-layout-probe --parallel 16

readonly worker=$build_dir/fe2o3-llvm-link-worker
readonly probe=$build_dir/fe2o3-row-softmax-llvm22-layout-probe
readonly worker_id_file=$build_dir/fe2o3-worker-build-id.txt
readonly llvm_id_file=$build_dir/fe2o3-llvm-build-id.txt
[[ $(/usr/bin/tr -d '\n' <"$worker_id_file") == "${manifest[WORKER_BUILD_CLAIM]}" &&
  $(/usr/bin/tr -d '\n' <"$llvm_id_file") == "${manifest[LLVM_BUILD_ID]}" ]] ||
  die "configured Worker/LLVM build claims differ from the manifest"
verify_file "$worker" "${manifest[WORKER_SHA256]}" "${manifest[WORKER_LENGTH]}" \
  "Worker ELF"
verify_file "$probe" "${manifest[PROBE_SHA256]}" "${manifest[PROBE_LENGTH]}" \
  "layout probe ELF"
if env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin /usr/bin/ldd "$worker" |
  /usr/bin/grep -qi comgr; then
  die "row-softmax Worker unexpectedly depends on COMGR"
fi

readonly probe_stdout=$build_dir/layout-probe.stdout
readonly probe_stderr=$build_dir/layout-probe.stderr
env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin "$probe" >"$probe_stdout" 2>"$probe_stderr"
[[ ! -s "$probe_stderr" ]] || die "layout probe wrote unexpected stderr"
verify_file "$probe_stdout" "${manifest[PROBE_STDOUT_SHA256]}" \
  "${manifest[PROBE_STDOUT_LENGTH]}" "layout probe stdout"
verify_file "$probe" "${manifest[PROBE_SHA256]}" "${manifest[PROBE_LENGTH]}" \
  "layout probe ELF after execution"

readonly observed_runtime=$build_dir/observed-runtime-provider.txt
readonly runtime_scratch=$build_dir/runtime-provider-scratch
/usr/bin/mkdir --mode=700 -- "$runtime_scratch"
generate_runtime_provider_manifest "$worker" "$requested_provider" "$canonical_provider" \
  "$observed_runtime" "$runtime_scratch"
/usr/bin/cmp -s -- "$observed_runtime" "${manifest[RUNTIME_PROVIDER_MANIFEST_PATH]}" ||
  die "runtime DSO/provider closure differs from its reviewed manifest"

env -i HOME="$HOME" LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  "$ctest" --test-dir "$build_dir" --output-on-failure \
  -R '^fe2o3-(worker-exact-row-softmax-v1-tests|row-softmax-llvm22-layout-probe)$'

readonly cargo=${manifest[CARGO_PATH]}
readonly rustc=${manifest[RUSTC_PATH]}
readonly retained_hsaco=$build_dir/row-softmax-v1.hsaco
readonly cargo_home=$build_dir/cargo-home
/usr/bin/mkdir --mode=700 -- "$cargo_home"
{
  printf '[source.crates-io]\n'
  printf 'replace-with = "vendored-sources"\n'
  printf '[source."git+https://github.com/harsh-nod/fe2o3.git?rev=e4ad3159491d14d48ffda099e34967341ec31c72"]\n'
  printf 'git = "https://github.com/harsh-nod/fe2o3.git"\n'
  printf 'rev = "e4ad3159491d14d48ffda099e34967341ec31c72"\n'
  printf 'replace-with = "vendored-sources"\n'
  printf '[source.vendored-sources]\n'
  printf 'directory = "%s"\n' "$cargo_vendor_root"
  printf '[net]\n'
  printf 'offline = true\n'
} >"$cargo_home/config.toml"
run_cargo() {
  env -i HOME="$HOME" USER="${USER:-harsh}" LOGNAME="${LOGNAME:-harsh}" \
    LC_ALL=C LANG=C PATH=/usr/bin:/bin CARGO_HOME="$cargo_home" \
    CARGO_TARGET_DIR="$cargo_target_dir" CARGO_NET_OFFLINE=true RUSTC="$rustc" \
    LD_LIBRARY_PATH="$rustc_sysroot/lib" \
    FE2O3_ROW_SOFTMAX_RELEASE_GATE=1 \
    FE2O3_TEST_ROW_SOFTMAX_LLVM22_LAYOUT_PROBE="$probe" \
    FE2O3_TEST_ROW_SOFTMAX_WORKER="$worker" \
    FE2O3_TEST_ROW_SOFTMAX_WORKER_BUILD_ID="${manifest[WORKER_BUILD_CLAIM]}" \
    FE2O3_TEST_ROW_SOFTMAX_LLVM_BUILD_ID="${manifest[LLVM_BUILD_ID]}" \
    FE2O3_TEST_ROW_SOFTMAX_WORKER_SHA256="${manifest[WORKER_SHA256]}" \
    FE2O3_TEST_ROW_SOFTMAX_WORKER_LENGTH="${manifest[WORKER_LENGTH]}" \
    FE2O3_TEST_RETAIN_ROW_SOFTMAX_HSACO="$retained_hsaco" \
    "$cargo" "$@"
}
cd "$repo_root"
run_cargo test --locked --offline -p rustc-codegen-fe2o3 --features object/std --lib \
  configured_upstream_ -- --test-threads=1 --nocapture
verify_file "$worker" "${manifest[WORKER_SHA256]}" "${manifest[WORKER_LENGTH]}" \
  "Worker ELF after Rust execution"
verify_file "$retained_hsaco" "${manifest[HSACO_SHA256]}" "${manifest[HSACO_LENGTH]}" \
  "real row-softmax HSACO"
run_cargo test --locked --offline -p fe2o3-hsaco-finalize --features object/std --lib \
  row_softmax_v1_worker -- --test-threads=1
run_cargo test --locked --offline -p fe2o3-hsaco-finalize --features object/std \
  --test worker_v2_hsaco_finalization row_softmax -- --test-threads=1

# Exercise the predicates with byte- and source-substituted copies. These are
# regression denials, not an authentication claim about the original files.
readonly denial_dir=$build_dir/substitution-denials
/usr/bin/mkdir --mode=700 -- "$denial_dir"
/usr/bin/cp -- "$worker" "$denial_dir/worker"
printf 'x' >>"$denial_dir/worker"
! file_matches "$denial_dir/worker" "${manifest[WORKER_SHA256]}" \
  "${manifest[WORKER_LENGTH]}" || die "substituted Worker was accepted"
/usr/bin/cp -- "$probe" "$denial_dir/probe"
printf 'x' >>"$denial_dir/probe"
! file_matches "$denial_dir/probe" "${manifest[PROBE_SHA256]}" \
  "${manifest[PROBE_LENGTH]}" || die "substituted probe was accepted"
/usr/bin/cp -- "$requested_provider/${PROVIDER_FILES[0]}" "$denial_dir/ocml.bc"
printf 'x' >>"$denial_dir/ocml.bc"
[[ $(sha256_file "$denial_dir/ocml.bc") != "${manifest[OCML_SHA256]}" ]] ||
  die "substituted provider was accepted"
/usr/bin/cp -- "$cache" "$denial_dir/CMakeCache.txt"
/usr/bin/sed -i "s#CMAKE_HOME_DIRECTORY:INTERNAL=$script_dir#CMAKE_HOME_DIRECTORY:INTERNAL=/substituted/source#" \
  "$denial_dir/CMakeCache.txt"
! verify_cmake_source "$denial_dir/CMakeCache.txt" "$script_dir" ||
  die "substituted CMake source was accepted"

verify_file "$manifest_path" "$expected_manifest_sha256" "$(file_length "$manifest_path")" \
  "release manifest after execution"
verify_source_state "$git" "$repo_root" "${manifest[IMPLEMENTATION_COMMIT]}" \
  "${manifest[IMPLEMENTATION_TREE]}"
verify_file "$cargo_lock" "${manifest[CARGO_LOCK_SHA256]}" \
  "${manifest[CARGO_LOCK_LENGTH]}" "Cargo.lock after execution"
[[ -z $("$git" -C "$llvm_source" status --porcelain=v1 --untracked-files=all) &&
  $("$git" -C "$llvm_source" rev-parse HEAD) == "${manifest[LLVM_COMMIT]}" &&
  $("$git" -C "$llvm_source" rev-parse 'HEAD^{tree}') == "${manifest[LLVM_TREE]}" ]] ||
  die "LLVM source changed during release-gate execution"
verify_file "${manifest[LLVM_PACKAGE_MANIFEST_PATH]}" \
  "${manifest[LLVM_PACKAGE_MANIFEST_SHA256]}" "${manifest[LLVM_PACKAGE_MANIFEST_LENGTH]}" \
  "LLVM package manifest after execution"
verify_file "${manifest[CARGO_VENDOR_MANIFEST_PATH]}" \
  "${manifest[CARGO_VENDOR_MANIFEST_SHA256]}" \
  "${manifest[CARGO_VENDOR_MANIFEST_LENGTH]}" "Cargo vendor manifest after execution"
verify_file "${manifest[RUSTC_SYSROOT_MANIFEST_PATH]}" \
  "${manifest[RUSTC_SYSROOT_MANIFEST_SHA256]}" \
  "${manifest[RUSTC_SYSROOT_MANIFEST_LENGTH]}" "rustc sysroot manifest after execution"
verify_file "${manifest[RUNTIME_PROVIDER_MANIFEST_PATH]}" \
  "${manifest[RUNTIME_PROVIDER_MANIFEST_SHA256]}" \
  "${manifest[RUNTIME_PROVIDER_MANIFEST_LENGTH]}" "runtime/provider manifest after execution"
verify_file "${manifest[LLVM_BUILD_ID_FILE]}" "${manifest[LLVM_BUILD_ID_FILE_SHA256]}" \
  "${manifest[LLVM_BUILD_ID_FILE_LENGTH]}" "LLVM build-ID file after execution"
verify_tree_closure "$llvm_package_root" fe2o3-llvm-package-closure-v1 \
  "${manifest[LLVM_PACKAGE_MANIFEST_PATH]}" "$observed_package_after" "LLVM package"
verify_tree_closure "$cargo_vendor_root" fe2o3-cargo-vendor-closure-v1 \
  "${manifest[CARGO_VENDOR_MANIFEST_PATH]}" "$observed_vendor_after" "Cargo vendor"
verify_tree_closure "$rustc_sysroot" fe2o3-rustc-sysroot-closure-v1 \
  "${manifest[RUSTC_SYSROOT_MANIFEST_PATH]}" "$observed_sysroot_after" "rustc sysroot"
generate_runtime_provider_manifest "$worker" "$requested_provider" "$canonical_provider" \
  "$build_dir/observed-runtime-provider-after.txt" "$runtime_scratch"
/usr/bin/cmp -s -- "$build_dir/observed-runtime-provider-after.txt" \
  "${manifest[RUNTIME_PROVIDER_MANIFEST_PATH]}" ||
  die "runtime DSO/provider closure changed during execution"
for index in "${!PROVIDER_FILES[@]}"; do
  [[ $(sha256_file "$requested_provider/${PROVIDER_FILES[$index]}") == "${provider_hashes[$index]}" ]] ||
    die "provider changed during execution"
done
for tool in CMAKE CTEST NINJA CXX CARGO RUSTC; do
  verify_file "${manifest[${tool}_PATH]}" "${manifest[${tool}_SHA256]}" \
    "${manifest[${tool}_LENGTH]}" "$tool executable at final recheck"
done
[[ $("${manifest[RUSTC_PATH]}" --print sysroot) == "$rustc_sysroot" ]] ||
  die "rustc sysroot changed during execution"
verify_file "$worker" "${manifest[WORKER_SHA256]}" "${manifest[WORKER_LENGTH]}" \
  "Worker ELF at final recheck"
verify_file "$probe" "${manifest[PROBE_SHA256]}" "${manifest[PROBE_LENGTH]}" \
  "layout probe ELF at final recheck"
verify_file "$retained_hsaco" "${manifest[HSACO_SHA256]}" "${manifest[HSACO_LENGTH]}" \
  "real row-softmax HSACO at final recheck"
directory_identity_matches "$build_dir" "$build_identity" ||
  die "CMake build directory was replaced during execution"
directory_identity_matches "$cargo_target_dir" "$cargo_target_identity" ||
  die "Cargo target directory was replaced during execution"

printf 'row-softmax-v1-release-gate=passed\n'
printf 'evidence-boundary=operator-selected-reviewed-integrity\n'
printf 'replacement-denials=passed\n'
