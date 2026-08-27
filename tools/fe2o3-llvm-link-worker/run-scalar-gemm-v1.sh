#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 6 ]]; then
  printf 'usage: %s BUILD_DIR LLVM_DIR LLD_DIR LLVM_VERSION LLVM_BUILD_ID_FILE OUTPUT_HSACO\n' "$0" >&2
  exit 64
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
build_dir=$1
llvm_dir=$2
lld_dir=$3
llvm_version=$4
llvm_build_id_file=$5
output_hsaco=$6

for path in "$llvm_dir/LLVMConfig.cmake" "$lld_dir/LLDConfig.cmake" "$llvm_build_id_file"; do
  if [[ ! -f $path ]]; then
    printf 'error: required LLVM/LLD pin is unavailable: %s\n' "$path" >&2
    exit 66
  fi
done
if [[ $build_dir != /* || $output_hsaco != /* ]]; then
  printf 'error: BUILD_DIR and OUTPUT_HSACO must be absolute paths\n' >&2
  exit 65
fi
if [[ -e $build_dir || -e $output_hsaco ]]; then
  printf 'error: build and output paths must be fresh\n' >&2
  exit 73
fi
for command in cmake ctest grep realpath sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'error: required command is unavailable: %s\n' "$command" >&2
    exit 69
  fi
done
cargo_bin=${FE2O3_CARGO:-$(command -v cargo || true)}
rustc_bin=${FE2O3_RUSTC:-$(command -v rustc || true)}
for tool in "$cargo_bin" "$rustc_bin"; do
  if [[ $tool != /* || ! -f $tool || ! -x $tool ]]; then
    printf 'error: FE2O3_CARGO and FE2O3_RUSTC must resolve to absolute executables\n' >&2
    exit 69
  fi
done
if [[ ${cargo_bin%/*} != "${rustc_bin%/*}" ]]; then
  printf 'error: FE2O3_CARGO and FE2O3_RUSTC must come from one toolchain\n' >&2
  exit 70
fi
toolchain_lib=$(realpath -e -- "${rustc_bin%/*}/../lib")

llvm_build_id=$(<"$llvm_build_id_file")
cmake -S "$repo_root/tools/fe2o3-llvm-link-worker" -B "$build_dir" \
  -DLLVM_DIR="$llvm_dir" \
  -DLLD_DIR="$lld_dir" \
  -DFE2O3_PINNED_LLVM_VERSION="$llvm_version" \
  -DFE2O3_LLVM_BUILD_ID_FILE="$llvm_build_id_file" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID="$llvm_build_id" \
  -DBUILD_TESTING=ON \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_dir" --parallel "${FE2O3_BUILD_JOBS:-8}"
ctest --test-dir "$build_dir" --output-on-failure

worker="$build_dir/fe2o3-llvm-link-worker"
worker_build_id_file="$build_dir/fe2o3-worker-build-id.txt"
if [[ ! -x $worker || ! -s $worker_build_id_file ]]; then
  printf 'error: measured direct LLVM/LLD worker was not produced\n' >&2
  exit 70
fi
worker_build_id=$(<"$worker_build_id_file")
llvm_root=${llvm_dir%/lib/cmake/llvm}
llvm_readelf="$llvm_root/bin/llvm-readelf"
if [[ ! -x $llvm_readelf ]]; then
  printf 'error: pinned llvm-readelf is unavailable: %s\n' "$llvm_readelf" >&2
  exit 69
fi
if "$llvm_readelf" --dynamic-table "$worker" | grep -qi comgr; then
  printf 'error: direct LLVM/LLD worker has a COMGR dependency\n' >&2
  exit 70
fi

frontend_handoff="$build_dir/scalar-gemm-v1.frontend-handoff-v2"
FE2O3_SCALAR_GEMM_V1_HANDOFF_OUTPUT="$frontend_handoff" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" --locked \
    -p rustc-codegen-fe2o3 \
    --test collected_executable_scalar_control_flow_v2 \
    scalar_gemm_v1_frontend_receipt_selects_only_the_reviewed_full_portable_mir \
    -- --exact
if [[ ! -s $frontend_handoff || -L $frontend_handoff ]]; then
  printf 'error: rustc frontend integration did not produce a regular scalar GEMM handoff\n' >&2
  exit 70
fi

FE2O3_SCALAR_GEMM_V1_WORKER="$worker" \
FE2O3_SCALAR_GEMM_V1_WORKER_BUILD_ID="$worker_build_id" \
FE2O3_SCALAR_GEMM_V1_LLVM_BUILD_ID="$llvm_build_id" \
FE2O3_SCALAR_GEMM_V1_HANDOFF="$frontend_handoff" \
FE2O3_SCALAR_GEMM_V1_OUTPUT="$output_hsaco" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --manifest-path "$repo_root/Cargo.toml" --locked \
    -p fe2o3-hsaco-finalize \
    --test scalar_gemm_v1_direct_llvm_worker \
    real_worker_produces_deterministic_finalized_scalar_gemm_v1_cov6_hsaco \
    -- --ignored --exact

if [[ ! -s $output_hsaco || -L $output_hsaco ]]; then
  printf 'error: scalar GEMM integration test did not produce a regular HSACO\n' >&2
  exit 70
fi
FE2O3_SCALAR_GEMM_V1_HSACO="$output_hsaco" \
  ctest --test-dir "$build_dir" --output-on-failure \
    --tests-regex '^fe2o3-worker-machine-effect-tests$'
"$llvm_readelf" --file-headers --notes --dyn-symbols "$output_hsaco"
sha256sum "$worker" "$output_hsaco"
sha256sum "$frontend_handoff"
printf 'worker build identity: %s\n' "$worker_build_id"
printf 'LLVM build identity: %s\n' "$llvm_build_id"
printf 'scalar GEMM gfx942:xnack- COV6 artifact: PASS (%s)\n' "$output_hsaco"
printf 'hardware execution: NOT RUN (this tool produces and inspects only)\n'
