#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-vecadd-profile.XXXXXX")"
trap 'rm -rf "$build_dir"' EXIT

export FE2O3_ALLOW_GPU_SMOKE=1
export FE2O3_TARGET="${FE2O3_TARGET:-gfx942:xnack-}"
n="${FE2O3_VECADD_N:-16777216}"
warmups="${FE2O3_VECADD_PROFILE_WARMUPS:-5}"
samples="${FE2O3_VECADD_PROFILE_SAMPLES:-5}"
launches="${FE2O3_VECADD_PROFILE_LAUNCHES_PER_SAMPLE:-20}"

cd "$repo_root"
command -v rocprofv3 >/dev/null

echo "Generating and validating the production fe2o3 VecAdd artifact..."
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd

echo "Building the algorithm-matched HIP VecAdd..."
hipcc -O3 --offload-arch=gfx942 benchmarks/vecadd_hip/vecadd.hip \
    -o "$build_dir/vecadd-hip"

echo "Profiling fe2o3 GPU dispatches..."
FE2O3_HSACO_DIR="$repo_root/target/fe2o3" \
    rocprofv3 --kernel-trace --stats --summary --summary-units usec \
    --summary-output-file stdout --output-directory "$build_dir/fe2o3" -- \
    "$repo_root/target/debug/fe2o3-vecadd" \
    --benchmark "$n" "$warmups" "$samples" "$launches"

echo "Profiling HIP GPU dispatches..."
rocprofv3 --kernel-trace --stats --summary --summary-units usec \
    --summary-output-file stdout --output-directory "$build_dir/hip" -- \
    "$build_dir/vecadd-hip" \
    --benchmark "$n" "$warmups" "$samples" "$launches"
