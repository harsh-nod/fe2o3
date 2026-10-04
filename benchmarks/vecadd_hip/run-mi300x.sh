#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-vecadd-bench.XXXXXX")"
trap 'rm -rf "$build_dir"' EXIT

export FE2O3_ALLOW_GPU_SMOKE=1
export FE2O3_TARGET="${FE2O3_TARGET:-gfx942:xnack-}"
n="${FE2O3_VECADD_N:-16777216}"
warmups="${FE2O3_VECADD_WARMUPS:-20}"
samples="${FE2O3_VECADD_SAMPLES:-30}"
launches="${FE2O3_VECADD_LAUNCHES_PER_SAMPLE:-100}"

cd "$repo_root"
echo "Building and running the production fe2o3 VecAdd..."
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd -- \
    --benchmark "$n" "$warmups" "$samples" "$launches"

echo "Building and running the algorithm-matched HIP VecAdd..."
hipcc -O3 --offload-arch=gfx942 benchmarks/vecadd_hip/vecadd.hip \
    -o "$build_dir/vecadd-hip"
"$build_dir/vecadd-hip" --benchmark "$n" "$warmups" "$samples" "$launches"
