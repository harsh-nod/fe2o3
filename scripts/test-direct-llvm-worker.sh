#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  printf 'usage: %s BUILD_DIR LLVM_DIR LLD_DIR LLVM_VERSION BUILD_ID_FILE\n' "$0" >&2
  exit 64
fi

build_dir=$1
llvm_dir=$2
lld_dir=$3
llvm_version=$4
build_id_file=$5
build_id=$(<"$build_id_file")

cmake -S tools/fe2o3-llvm-link-worker -B "$build_dir" \
  -DLLVM_DIR="$llvm_dir" \
  -DLLD_DIR="$lld_dir" \
  -DFE2O3_PINNED_LLVM_VERSION="$llvm_version" \
  -DFE2O3_LLVM_BUILD_ID_FILE="$build_id_file" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="$build_id"
cmake --build "$build_dir" --parallel
ctest --test-dir "$build_dir" --output-on-failure
