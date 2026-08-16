#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 ABSOLUTE_CMAKE_BUILD_DIR ABSOLUTE_CARGO_TARGET_DIR" >&2
  exit 2
fi

build_dir=$1
cargo_target_dir=$2
case "$build_dir:$cargo_target_dir" in
  /*:/*) ;;
  *) echo "release-gate paths must be absolute" >&2; exit 2 ;;
esac

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd -- "$script_dir/../.." && pwd -P)
cache="$build_dir/CMakeCache.txt"
worker="$build_dir/fe2o3-llvm-link-worker"
probe="$build_dir/fe2o3-row-softmax-llvm22-layout-probe"
worker_id_file="$build_dir/fe2o3-worker-build-id.txt"
llvm_id_file="$build_dir/fe2o3-llvm-build-id.txt"
provider_file="$build_dir/fe2o3-gfx942-ocml-provider.txt"

for required in "$cache" "$worker_id_file" "$llvm_id_file" "$provider_file"; do
  [[ -f "$required" ]] || {
    echo "row-softmax release gate missing $required" >&2
    exit 1
  }
done
grep -qx 'FE2O3_ROW_SOFTMAX_RELEASE_GATE:BOOL=ON' "$cache" || {
  echo "CMake build was not configured with FE2O3_ROW_SOFTMAX_RELEASE_GATE=ON" >&2
  exit 1
}
grep -qx 'BUILD_TESTING:BOOL=ON' "$cache" || {
  echo "row-softmax release gate requires BUILD_TESTING=ON" >&2
  exit 1
}
grep -qx 'enabled=1' "$provider_file" || {
  echo "row-softmax release gate has no measured gfx942 OCML closure" >&2
  exit 1
}
if grep -q '=disabled$' "$provider_file"; then
  echo "row-softmax release gate contains a disabled OCML provider identity" >&2
  exit 1
fi

cmake --build "$build_dir" --target \
  fe2o3-llvm-link-worker \
  fe2o3-worker-pipeline-tests \
  fe2o3-row-softmax-llvm22-layout-probe --parallel

for executable in "$worker" "$probe"; do
  [[ -x "$executable" ]] || {
    echo "row-softmax release gate missing executable $executable" >&2
    exit 1
  }
done
if ldd "$worker" | grep -qi comgr; then
  echo "row-softmax release worker unexpectedly depends on COMGR" >&2
  exit 1
fi

ctest --test-dir "$build_dir" --output-on-failure \
  -R '^fe2o3-(worker-exact-row-softmax-v1-tests|row-softmax-llvm22-layout-probe)$'

worker_build_id=$(tr -d '\n' < "$worker_id_file")
llvm_build_id=$(tr -d '\n' < "$llvm_id_file")
[[ "$worker_build_id" == fe2o3-worker-v1-sha256-* ]] || {
  echo "row-softmax release gate has a malformed worker build identity" >&2
  exit 1
}
[[ "$llvm_build_id" == upstream-llvmorg-22.1.8-* ]] || {
  echo "row-softmax release gate has an unexpected LLVM build identity" >&2
  exit 1
}

cargo_bin=${FE2O3_CARGO:-cargo}
export CARGO_TARGET_DIR="$cargo_target_dir"
export FE2O3_ROW_SOFTMAX_RELEASE_GATE=1
export FE2O3_TEST_ROW_SOFTMAX_LLVM22_LAYOUT_PROBE="$probe"
export FE2O3_TEST_ROW_SOFTMAX_WORKER="$worker"
export FE2O3_TEST_ROW_SOFTMAX_WORKER_BUILD_ID="$worker_build_id"
export FE2O3_TEST_ROW_SOFTMAX_LLVM_BUILD_ID="$llvm_build_id"

cd "$repo_root"
"$cargo_bin" test -p rustc-codegen-fe2o3 --features object/std --lib \
  configured_upstream_ -- --test-threads=1 --nocapture
"$cargo_bin" test -p fe2o3-hsaco-finalize --features object/std --lib \
  row_softmax_v1_worker -- --test-threads=1
"$cargo_bin" test -p fe2o3-hsaco-finalize --features object/std \
  --test worker_v2_hsaco_finalization row_softmax -- --test-threads=1

echo "row-softmax-v1-release-gate=passed"
