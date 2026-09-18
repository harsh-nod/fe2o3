#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/raw" && ! -e "$archive/SHA256SUMS" ]]
mkdir -- "$archive/raw"
record() {
    local name=$1 status
    shift
    [[ ! -e "$archive/raw/$name.command" ]]
    printf '%q ' "$@" > "$archive/raw/$name.command"
    printf '\n' >> "$archive/raw/$name.command"
    date -u +%FT%T.%NZ > "$archive/raw/$name.started"
    if "$@" > "$archive/raw/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%T.%NZ > "$archive/raw/$name.finished"
    printf '%s\n' "$status" > "$archive/raw/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}
inputs="$root/docs/evidence/dev-hip-copy-only-cpu-2026-09-18/inputs.py"
python_files=(benchmarks/runtime_gfx942/hip_copy_diagnostic.py benchmarks/runtime_gfx942/test_hip_copy_payload.py benchmarks/runtime_gfx942/test_hip_copy_diagnostic.py "$archive/verify.py")
record inputs-before python3 -I "$inputs"
record tests env ROCM_PATH=/opt/rocm python3 -B -m unittest discover -s benchmarks/runtime_gfx942 -p 'test_hip_copy*.py' -v
record arguments python3 -B -m unittest discover -s benchmarks/runtime_gfx942 -p test_native_benchmark_args.py -v
record lint ruff check --no-cache "${python_files[@]}"
record format ruff format --check --no-cache "${python_files[@]}"
record shellcheck shellcheck "$archive/qualify.sh" "$archive/seal.sh"
record inputs-after python3 -I "$inputs"
record verify-final python3 -I "$archive/verify.py"
