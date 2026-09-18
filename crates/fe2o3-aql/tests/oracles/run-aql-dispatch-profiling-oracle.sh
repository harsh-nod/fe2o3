#!/bin/sh
set -eu

reference_dir=${1:?usage: run-aql-dispatch-profiling-oracle.sh pinned-header-dir output-dir}
output_dir=${2:?usage: run-aql-dispatch-profiling-oracle.sh pinned-header-dir output-dir}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

check() {
    expected=$1
    file=$2
    actual=$(sha256sum -- "$file" | awk '{print $1}')
    test "$actual" = "$expected" || {
        printf '%s: expected %s, observed %s\n' "$file" "$expected" "$actual" >&2
        exit 1
    }
}

check 51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5 "$reference_dir/hsa.h"
check abfccdd1eabe77047b16743ce0f729577c99caf8c7b28b880232602a5b46a17f "$reference_dir/amd_hsa_common.h"
check 1f45345473ea2a02200748106a43f8aaf97568e11c8182a789684320008592e3 "$reference_dir/amd_hsa_queue.h"
check ba429b422e91fe370e4241ce8c8d934738b6e3c59b10c1eefd2370d76afe5020 "$reference_dir/amd_hsa_signal.h"

mkdir -p -- "$output_dir"
binary=$output_dir/aql-dispatch-profiling-oracle
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror -I"$reference_dir" \
    "$here/aql_dispatch_profiling_7_2_4.c" -o "$binary"
"$binary"
