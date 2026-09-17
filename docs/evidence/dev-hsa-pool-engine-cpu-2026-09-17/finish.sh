#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ $(< "$archive/raw/benchmark-suite.exit") == 1 ]]
record() { bash "$archive/record.sh" "$@"; }
record fixtures-before sha256sum --check "$archive/fixture-files.sha256"
record r26-fixtures /usr/bin/python3 -B -I -m unittest discover -s benchmarks/runtime_gfx942 -p test_r26_host_guard.py
record r60-fixtures /usr/bin/python3 -B -I -m unittest discover -s benchmarks/runtime_gfx942 -p test_run_r60_pipeline.py
record benchmark-suite-corrected /usr/bin/python3 -B -I -m unittest discover -s benchmarks/runtime_gfx942 -p 'test_*.py'
record native-link bash "$archive/native-link.sh"
record python-lint ruff check benchmarks/runtime_gfx942/test_hsa_copy_diagnostic.py
record python-format ruff format --check benchmarks/runtime_gfx942/test_hsa_copy_diagnostic.py
record cpp-format clang-format --dry-run --Werror \
    benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp \
    benchmarks/runtime_gfx942/hsa_copy_diagnostic.hpp \
    benchmarks/runtime_gfx942/hsa_copy_diagnostic_mock.cpp \
    benchmarks/runtime_gfx942/hsa_copy_diagnostic_test.cpp
record unchanged git diff --exit-code 5ed58ea896e492f5bbc135e9f8449eabd51165bd -- \
    crates benchmarks/runtime_gfx942/async_copy_hsa.cpp \
    benchmarks/runtime_gfx942/async_copy_hip.cpp \
    benchmarks/runtime_gfx942/run-directional-window-mi300x.sh \
    benchmarks/runtime_gfx942/r26-host-guard.py \
    benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py
record source-after sha256sum --check "$archive/source-files.sha256"
record fixtures-after sha256sum --check "$archive/fixture-files.sha256"
record tools-after sha256sum --check "$archive/raw/tool-inputs.log"
record mi300x-observation ssh -o BatchMode=yes mi300x \
    'rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --showpids'
