#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 13 ]]; then
  printf 'usage: %s BUILD_DIR LLVM_DIR LLD_DIR LLVM_VERSION BUILD_ID_FILE TARGET CARGO CARGO_SHA256 RUSTC RUSTC_SHA256 RUST_TOOLCHAIN TOOLCHAIN_MANIFEST_SHA256 SOURCE_COMMIT\n' "$0" >&2
  exit 64
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
build_dir=$1
llvm_dir=$2
lld_dir=$3
llvm_version=$4
build_id_file=$5
target=$6
cargo_bin=$7
cargo_sha256=$8
rustc_bin=$9
rustc_sha256=${10}
rust_toolchain=${11}
toolchain_manifest_sha256=${12}
source_commit=${13}
test_target=direct_llvm_worker_integration
test_name=real_worker_links_mixed_inputs_through_pinned_supervision

if [[ $build_dir != /* ]]; then
  printf 'error: BUILD_DIR must be absolute\n' >&2
  exit 65
fi
if [[ -e $build_dir ]]; then
  printf 'error: BUILD_DIR must not already exist: %s\n' "$build_dir" >&2
  exit 73
fi
build_parent=$(realpath -e -- "$(dirname -- "$build_dir")")
canonical_build_dir="$build_parent/$(basename -- "$build_dir")"
if [[ $canonical_build_dir != "$build_dir" ]]; then
  printf 'error: BUILD_DIR must have a canonical existing parent: %s\n' "$build_dir" >&2
  exit 65
fi
case "$build_dir" in
  "$repo_root" | "$repo_root"/*)
    printf 'error: BUILD_DIR must be outside the source tree\n' >&2
    exit 65
    ;;
esac
if [[ ! -w $build_parent ]]; then
  printf 'error: BUILD_DIR parent is not writable: %s\n' "$build_parent" >&2
  exit 73
fi
if [[ $target != gfx942 ]]; then
  printf 'error: the exported mixed-input fixture is pinned to gfx942, got %s\n' "$target" >&2
  exit 65
fi
for command in cmake ctest cmp git grep python3 realpath sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'error: required command is unavailable: %s\n' "$command" >&2
    exit 69
  fi
done

verify_executable() {
  local label=$1
  local path=$2
  local expected_sha256=$3
  local actual_sha256 ignored
  if [[ $path != /* || ! -f $path || ! -x $path || -L $path ]]; then
    printf 'error: pinned %s must be an absolute regular executable: %s\n' \
      "$label" "$path" >&2
    exit 69
  fi
  if [[ $(realpath -e -- "$path") != "$path" ]]; then
    printf 'error: pinned %s path is not canonical: %s\n' "$label" "$path" >&2
    exit 69
  fi
  if [[ ! $expected_sha256 =~ ^[0-9a-f]{64}$ ]]; then
    printf 'error: pinned %s SHA-256 is malformed\n' "$label" >&2
    exit 65
  fi
  read -r actual_sha256 ignored < <(sha256sum -- "$path")
  if [[ $actual_sha256 != "$expected_sha256" ]]; then
    printf 'error: pinned %s SHA-256 mismatch\n' "$label" >&2
    exit 70
  fi
}

verify_file_identity() {
  local label=$1
  local path=$2
  local expected_sha256=$3
  local actual_sha256 ignored
  if [[ ! -f $path || -L $path || ! $expected_sha256 =~ ^[0-9a-f]{64}$ ]]; then
    printf 'error: pinned %s identity is invalid\n' "$label" >&2
    exit 66
  fi
  read -r actual_sha256 ignored < <(sha256sum -- "$path")
  if [[ $actual_sha256 != "$expected_sha256" ]]; then
    printf 'error: pinned %s SHA-256 mismatch\n' "$label" >&2
    exit 70
  fi
}

verify_executable Cargo "$cargo_bin" "$cargo_sha256"
verify_executable rustc "$rustc_bin" "$rustc_sha256"
toolchain_bin_dir=${rustc_bin%/*}
if [[ ${cargo_bin%/*} != "$toolchain_bin_dir" ]]; then
  printf 'error: pinned Cargo and rustc must come from one toolchain bin directory\n' >&2
  exit 70
fi
toolchain_root=$(realpath -e -- "$toolchain_bin_dir/..")
toolchain_directory=${toolchain_root##*/}
if [[ $toolchain_directory != "$rust_toolchain"-* || ! -d $toolchain_root/lib ]]; then
  printf 'error: pinned executables do not belong to the declared Rust toolchain\n' >&2
  exit 70
fi
toolchain_lib="$toolchain_root/lib"
cargo_version=$(LD_LIBRARY_PATH="$toolchain_lib" "$cargo_bin" --version)
rustc_version=$(LD_LIBRARY_PATH="$toolchain_lib" "$rustc_bin" --version --verbose)
if [[ $cargo_version != cargo\ * ]]; then
  printf 'error: pinned Cargo executable returned an invalid version\n' >&2
  exit 70
fi
if [[ $rustc_version != rustc\ * ]]; then
  printf 'error: pinned rustc executable returned an invalid version\n' >&2
  exit 70
fi
printf '%s\n' "$cargo_version"
printf '%s\n' "$rustc_version"

toolchain_manifest="$repo_root/rust-toolchain.toml"
verify_file_identity rust-toolchain.toml \
  "$toolchain_manifest" "$toolchain_manifest_sha256"
if [[ -z $rust_toolchain ]] ||
  ! grep -Fqx "channel = \"$rust_toolchain\"" "$toolchain_manifest"; then
  printf 'error: pinned Rust toolchain does not match rust-toolchain.toml\n' >&2
  exit 70
fi
if [[ ! $source_commit =~ ^[0-9a-f]{40}$ ]]; then
  printf 'error: SOURCE_COMMIT must be a full lowercase Git object ID\n' >&2
  exit 65
fi
actual_commit=$(git -C "$repo_root" rev-parse HEAD)
if [[ $actual_commit != "$source_commit" ]]; then
  printf 'error: source commit mismatch: expected %s, found %s\n' \
    "$source_commit" "$actual_commit" >&2
  exit 70
fi
source_status=$(git -C "$repo_root" status --porcelain --untracked-files=all)
if [[ -n $source_status ]]; then
  printf 'error: source tree is not clean at pinned commit %s\n' "$source_commit" >&2
  exit 70
fi

for path in "$llvm_dir/LLVMConfig.cmake" "$lld_dir/LLDConfig.cmake" "$build_id_file"; do
  if [[ ! -f $path ]]; then
    printf 'error: required pinned input is unavailable: %s\n' "$path" >&2
    exit 66
  fi
done
build_id=$(<"$build_id_file")

cmake -S "$repo_root/tools/fe2o3-llvm-link-worker" -B "$build_dir" \
  -DLLVM_DIR="$llvm_dir" \
  -DLLD_DIR="$lld_dir" \
  -DFE2O3_PINNED_LLVM_VERSION="$llvm_version" \
  -DFE2O3_LLVM_BUILD_ID_FILE="$build_id_file" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="$build_id" \
  -DBUILD_TESTING=ON \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_dir" --parallel "${FE2O3_BUILD_JOBS:-8}"
ctest --test-dir "$build_dir" --output-on-failure

fixture_dir="$build_dir/direct-llvm-integration"
mkdir "$fixture_dir"
native_hsaco="$fixture_dir/native-pipeline.hsaco"
bitcode="$fixture_dir/mixed.bc"
object="$fixture_dir/mixed.o"
rust_hsaco="$fixture_dir/pinned-worker.hsaco"
cargo_json="$fixture_dir/cargo-test.jsonl"
for path in "$native_hsaco" "$bitcode" "$object" "$rust_hsaco" "$cargo_json"; do
  if [[ -e $path ]]; then
    printf 'error: fresh evidence path already exists: %s\n' "$path" >&2
    exit 73
  fi
done
"$build_dir/fe2o3-worker-pipeline-tests" \
  "$native_hsaco" "$bitcode" "$object"
for path in "$native_hsaco" "$bitcode" "$object"; do
  if [[ ! -f $path || ! -s $path || -L $path ]]; then
    printf 'error: native fixture generation omitted output: %s\n' "$path" >&2
    exit 70
  fi
done

worker="$build_dir/fe2o3-llvm-link-worker"
worker_build_id_file="$build_dir/fe2o3-worker-build-id.txt"
if [[ ! -x $worker || ! -f $worker_build_id_file ]]; then
  printf 'error: Release worker or measured build claim is absent\n' >&2
  exit 70
fi
worker_build_id=$(<"$worker_build_id_file")

FE2O3_LLVM_LINK_WORKER="$worker" \
FE2O3_LLVM_LINK_WORKER_BUILD_ID="$worker_build_id" \
FE2O3_LLVM_BUILD_ID="$build_id" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" \
    -p fe2o3-hsaco-finalize --locked \
    --test direct_llvm_worker_protocol \
    cpp_worker_cross_language_failure_round_trip_when_configured -- --exact

FE2O3_LLVM_LINK_WORKER="$worker" \
FE2O3_LLVM_LINK_WORKER_BUILD_ID="$worker_build_id" \
FE2O3_LLVM_BUILD_ID="$build_id" \
FE2O3_LLVM_V2_MODULE="$bitcode" \
FE2O3_LLVM_V2_PROVIDER="$object" \
FE2O3_LLVM_V2_EXPECTED_OUTPUT="$native_hsaco" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" \
    -p fe2o3-hsaco-finalize --locked --lib \
    worker_executor::configured_v2_tests::configured_cpp_worker_v2_round_trip \
    -- --ignored --exact

LD_LIBRARY_PATH="$toolchain_lib" RUSTC="$rustc_bin" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" \
  -p fe2o3-hsaco-finalize --locked --tests
FE2O3_DIRECT_LLVM_WORKER="$worker" \
FE2O3_DIRECT_LLVM_WORKER_BUILD_ID="$worker_build_id" \
FE2O3_DIRECT_LLVM_BUILD_ID="$build_id" \
FE2O3_DIRECT_LLVM_BITCODE="$bitcode" \
FE2O3_DIRECT_LLVM_OBJECT="$object" \
FE2O3_DIRECT_LLVM_EXPECTED_OUTPUT="$native_hsaco" \
FE2O3_DIRECT_LLVM_OUTPUT="$rust_hsaco" \
FE2O3_DIRECT_LLVM_TARGET="$target" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" \
    -p fe2o3-hsaco-finalize --locked --message-format=json \
    --test "$test_target" "$test_name" -- \
    --ignored --exact -Z unstable-options --format json >"$cargo_json"
python3 "$repo_root/scripts/verify-cargo-test-json.py" "$cargo_json" \
  --test-target "$test_target" --test-name "$test_name"
if [[ ! -f $rust_hsaco || ! -s $rust_hsaco || -L $rust_hsaco ]]; then
  printf 'error: verified integration test did not create a fresh HSACO\n' >&2
  exit 70
fi
cmp -- "$native_hsaco" "$rust_hsaco"

llvm_root=${llvm_dir%/lib/cmake/llvm}
llvm_readelf="$llvm_root/bin/llvm-readelf"
if [[ ! -x $llvm_readelf ]]; then
  printf 'error: pinned llvm-readelf is unavailable: %s\n' "$llvm_readelf" >&2
  exit 69
fi
"$llvm_readelf" --file-headers --dyn-symbols "$rust_hsaco"
printf 'worker build claim: %s\n' "$worker_build_id"
printf 'LLVM build identity: %s\n' "$build_id"
printf 'source commit: %s\n' "$source_commit"
printf 'Cargo SHA-256: %s\n' "$cargo_sha256"
printf 'rustc SHA-256: %s\n' "$rustc_sha256"
printf 'Cargo/libtest evidence: %s\n' "$cargo_json"
printf 'native integration: PASS (%s)\n' "$rust_hsaco"
printf 'hardware execution: NOT RUN (this test performs no GPU dispatch)\n'
