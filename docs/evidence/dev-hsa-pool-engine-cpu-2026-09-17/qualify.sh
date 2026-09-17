#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
record() { bash "$archive/record.sh" "$@"; }
record base git rev-parse HEAD
record gcc /usr/bin/g++ --version
record python /usr/bin/python3 --version
record source-before sha256sum --check "$archive/source-files.sha256"
record tool-inputs sha256sum /usr/bin/g++ /usr/bin/python3 /usr/bin/readelf \
    /opt/rocm/include/hsa/hsa.h /opt/rocm/include/hsa/hsa_ext_amd.h \
    /opt/rocm/lib/libhsa-runtime64.so
record focused /usr/bin/python3 -B -I benchmarks/runtime_gfx942/test_hsa_copy_diagnostic.py -v
record benchmark-suite /usr/bin/python3 -B -I -m unittest discover -s benchmarks/runtime_gfx942 -p 'test_*.py'
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
    benchmarks/runtime_gfx942/run-directional-window-mi300x.sh
record source-after sha256sum --check "$archive/source-files.sha256"
record tools-after sha256sum --check "$archive/raw/tool-inputs.log"
record mi300x-observation ssh -o BatchMode=yes mi300x \
    'rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --showpids'
