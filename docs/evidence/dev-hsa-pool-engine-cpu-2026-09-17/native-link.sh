#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
build=$(mktemp -d /tmp/fe2o3-hsa-diagnostic-link-XXXXXXXX)
cleanup() {
    rm -f -- "$build/hsa-copy-pool-engine"
    rmdir -- "$build"
}
trap cleanup EXIT
cd -- "$root"
/usr/bin/g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic \
    -isystem /opt/rocm/include \
    benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp \
    -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 \
    -o "$build/hsa-copy-pool-engine"
sha256sum "$build/hsa-copy-pool-engine"
/usr/bin/readelf -d "$build/hsa-copy-pool-engine"
printf 'NATIVE_LINK_OK executable_not_run=1 mock_not_linked=1\n'
