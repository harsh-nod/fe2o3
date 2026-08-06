#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 8 ]]; then
  printf 'usage: %s BUILD_DIR LLVM_DIR LLD_DIR LLVM_VERSION BUILD_ID_FILE TARGET CARGO RUST_TOOLCHAIN\n' "$0" >&2
  exit 64
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
build_dir=$1
llvm_dir=$2
lld_dir=$3
llvm_version=$4
build_id_file=$5
target=$6
cargo_bin=$7
rust_toolchain=$8

if [[ $build_dir != /* ]]; then
  build_dir="$repo_root/$build_dir"
fi
if [[ $target != gfx942 ]]; then
  printf 'error: the exported mixed-input fixture is pinned to gfx942, got %s\n' "$target" >&2
  exit 65
fi
for command in cmake ctest; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'error: required command is unavailable: %s\n' "$command" >&2
    exit 69
  fi
done
if [[ ! -x $cargo_bin ]]; then
  printf 'error: pinned Cargo executable is unavailable: %s\n' "$cargo_bin" >&2
  exit 69
fi
if [[ -z $rust_toolchain ]]; then
  printf 'error: pinned Rust toolchain is empty\n' >&2
  exit 65
fi
"$cargo_bin" "+$rust_toolchain" --version
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
mkdir -p "$fixture_dir"
native_hsaco="$fixture_dir/native-pipeline.hsaco"
bitcode="$fixture_dir/mixed.bc"
object="$fixture_dir/mixed.o"
rust_hsaco="$fixture_dir/pinned-worker.hsaco"
"$build_dir/fe2o3-worker-pipeline-tests" \
  "$native_hsaco" "$bitcode" "$object"

worker="$build_dir/fe2o3-llvm-link-worker"
worker_build_id_file="$build_dir/fe2o3-worker-build-id.txt"
if [[ ! -x $worker || ! -f $worker_build_id_file ]]; then
  printf 'error: Release worker or measured build claim is absent\n' >&2
  exit 70
fi
worker_build_id=$(<"$worker_build_id_file")

"$cargo_bin" "+$rust_toolchain" test --manifest-path "$repo_root/Cargo.toml" \
  -p fe2o3-hsaco-finalize --locked
FE2O3_DIRECT_LLVM_WORKER="$worker" \
FE2O3_DIRECT_LLVM_WORKER_BUILD_ID="$worker_build_id" \
FE2O3_DIRECT_LLVM_BUILD_ID="$build_id" \
FE2O3_DIRECT_LLVM_BITCODE="$bitcode" \
FE2O3_DIRECT_LLVM_OBJECT="$object" \
FE2O3_DIRECT_LLVM_OUTPUT="$rust_hsaco" \
FE2O3_DIRECT_LLVM_TARGET="$target" \
  "$cargo_bin" "+$rust_toolchain" test --manifest-path "$repo_root/Cargo.toml" \
    -p fe2o3-hsaco-finalize --locked \
    --test direct_llvm_worker_integration -- \
    --ignored --exact real_worker_links_mixed_inputs_through_pinned_supervision --nocapture

llvm_root=${llvm_dir%/lib/cmake/llvm}
llvm_readelf="$llvm_root/bin/llvm-readelf"
if [[ ! -x $llvm_readelf ]]; then
  printf 'error: pinned llvm-readelf is unavailable: %s\n' "$llvm_readelf" >&2
  exit 69
fi
"$llvm_readelf" --file-headers --dyn-symbols "$rust_hsaco"
printf 'worker build claim: %s\n' "$worker_build_id"
printf 'LLVM build identity: %s\n' "$build_id"
printf 'native integration: PASS (%s)\n' "$rust_hsaco"
printf 'hardware execution: NOT RUN (this test performs no GPU dispatch)\n'
