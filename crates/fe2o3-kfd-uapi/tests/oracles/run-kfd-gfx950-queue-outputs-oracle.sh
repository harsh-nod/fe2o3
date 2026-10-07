#!/bin/sh
set -eu

# Read-only bundle contains the queue-source/ and doorbell-audit/ archive members.
bundle=${1:?usage: sh run-kfd-gfx950-queue-outputs-oracle.sh source-bundle}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$here/../../src/gfx950_queue_outputs/profile.manifest"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-gfx950-output-oracle.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT HUP INT TERM

while IFS='=' read -r key expected; do
    case "$key" in
        source.*)
            file="$bundle/${key#source.}"
            actual=$(sha256sum -- "$file" | awk '{print $1}')
            test "$actual" = "$expected" || {
                printf '%s: expected %s, observed %s\n' "$file" "$expected" "$actual" >&2
                exit 1
            }
            ;;
        geometry_profile_sha256)
            geometry="$here/../../../fe2o3-kfd/src/gfx950_queue_resources/profile.manifest"
            actual=$(sha256sum -- "$geometry" | awk '{print $1}')
            test "$actual" = "$expected" || {
                printf 'geometry profile mismatch: expected %s, observed %s\n' "$expected" "$actual" >&2
                exit 1
            }
            ;;
    esac
done < "$manifest"

# Copy only the exact object/function macros needed by the standalone C oracle;
# their text is compiled unchanged. Hash verification precedes this extraction.
awk '
BEGIN {
    split("KFD_GPU_ID_HASH_WIDTH KFD_MMAP_TYPE_SHIFT KFD_MMAP_TYPE_MASK KFD_MMAP_TYPE_DOORBELL KFD_MMAP_GPU_ID_SHIFT KFD_MMAP_GPU_ID_MASK KFD_MMAP_GPU_ID KFD_MMAP_GET_GPU_ID KFD_MAX_NUM_OF_QUEUES_PER_PROCESS", names)
    for (item in names) wanted[names[item]] = 1
}
{
    if (!continuation && $1 == "#define") {
        name = $2
        sub(/\(.*/, "", name)
        if (name in wanted) {
            seen[name]++
            continuation = 1
        }
    }
    if (continuation) {
        print
        continuation = /\\$/
    }
}
END {
    for (name in wanted) if (seen[name] != 1) exit 1
    if (continuation) exit 1
}' "$bundle/queue-source/kfd_priv.h" > "$scratch/reviewed-output-macros.h"

cc -std=c11 -Wall -Wextra -Werror -I"$bundle/queue-source" -I"$scratch" \
    "$here/kfd_gfx950_queue_outputs_1_18.c" -o "$scratch/oracle"
"$scratch/oracle" > "$scratch/observed"
diff -u "$here/kfd_gfx950_queue_outputs_1_18.txt" "$scratch/observed"
cat "$scratch/observed"
printf 'gfx950 output oracle: source hashes, extracted macros and numeric record passed\n'
