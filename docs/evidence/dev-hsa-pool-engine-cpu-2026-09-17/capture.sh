#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ $(git rev-parse HEAD) == 5ed58ea896e492f5bbc135e9f8449eabd51165bd ]]
[[ ! -e "$archive/source-files.sha256" && ! -e "$archive/source.patch" ]]
changed=(
    benchmarks/runtime_gfx942/HSA-COPY-DIAGNOSTIC.md
    benchmarks/runtime_gfx942/README.md
    benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp
    benchmarks/runtime_gfx942/hsa_copy_diagnostic.hpp
    benchmarks/runtime_gfx942/hsa_copy_diagnostic_mock.cpp
    benchmarks/runtime_gfx942/hsa_copy_diagnostic_test.cpp
    benchmarks/runtime_gfx942/test_hsa_copy_diagnostic.py
)
sources=("${changed[@]}" benchmarks/runtime_gfx942/native_benchmark_args.hpp)
for path in "${sources[@]}"; do git ls-files --error-unmatch -- "$path" >/dev/null; done
git diff --quiet -- "${sources[@]}"
printf '%s\n' "${changed[@]}" > "$archive/changed-files.list"
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff HEAD --binary -- "${changed[@]}" > "$archive/source.patch"
sha256sum "$archive/source-files.sha256" "$archive/source.patch"
