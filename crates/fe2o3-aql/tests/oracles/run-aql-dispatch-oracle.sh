#!/bin/sh
set -eu

rocr_source=${1:-/home/harsh/work/rocm-systems-7.2.4-issue137-r4-readonly}
root=$rocr_source/projects/rocr-runtime/runtime/hsa-runtime
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$here/../../../.." && pwd)
binary=${TMPDIR:-/tmp}/fe2o3-aql-dispatch-oracle

check() {
    expected=$1
    file=$2
    actual=$(sha256sum -- "$file" | awk '{print $1}')
    test "$actual" = "$expected" || {
        printf '%s: expected %s, observed %s\n' "$file" "$expected" "$actual" >&2
        exit 1
    }
}

check 51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5 "$root/inc/hsa.h"
check ba429b422e91fe370e4241ce8c8d934738b6e3c59b10c1eefd2370d76afe5020 "$root/inc/amd_hsa_signal.h"
check 615199b8f8321de9f766d3be4d17caaec58e5057c6113767f6181c455fb7667a "$root/core/inc/signal.h"
check 2faa5a0a554a4c15d9a83991f02717afb0436eceedaf51040b74defbb61c5c73 "$root/core/runtime/signal.cpp"
check 440c98decfaa80db761ca5ec0add6f8956e4bbbaeff1ff9b185096c9852629ce "$root/core/inc/default_signal.h"
check 8bc963899a366b4de8745c88939b9f9ab2d779cecffe42f4171a8e6a796a3cd2 "$root/core/runtime/default_signal.cpp"
check 291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14 "$root/core/runtime/amd_aql_queue.cpp"
check e6ce094b32e4f300bd574db1a056fdd91740dbcae722da0824eb119a7e6490a2 "$root/core/runtime/amd_blit_kernel.cpp"
check aa1cd1acea3405e8c18076b406dd91b5433438792f7cbe8ac5bc3d46df25a9ca "$root/core/inc/queue.h"
check 2f48b1fff5432fb96aa460d3c5ac0bccb2e8996adfa5ecdb508722f3911ff9d0 "$root/inc/amd_hsa_kernel_code.h"
check 99dc188ad8b12561b66ac4a156fdbcfec068c1797fad75afa43a45d3a830554f "$repo_root/crates/fe2o3-hsa-runtime/native/runtime.c"

cc -std=c11 -Wall -Wextra -Werror \
    -I"$root/inc" \
    "$here/aql_dispatch_7_2_4.c" -o "$binary"
"$binary"
