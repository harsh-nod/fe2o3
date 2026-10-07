#!/bin/sh
set -eu

# The bundle is a flat read-only copy of the exact manifest-named source files.
bundle=${1:?usage: sh run-kfd-gfx950-queue-resources-oracle.sh source-bundle}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$here/../../../fe2o3-kfd/src/gfx950_queue_resources/profile.manifest"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-gfx950-geometry-oracle.XXXXXXXX")
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
    esac
done < "$manifest"

cc -std=c11 -Wall -Wextra -Werror -I"$bundle" \
    "$here/kfd_gfx950_queue_resources_1_18.c" -o "$scratch/oracle"
"$scratch/oracle" > "$scratch/observed"
diff -u "$here/../../../fe2o3-kfd/src/gfx950_queue_resources/oracle.txt" "$scratch/observed"
cat "$scratch/observed"

# Independent positive size precedence, including zero-as-absent.
"$scratch/oracle" 0 0 > "$scratch/zero"
cmp "$scratch/observed" "$scratch/zero"
"$scratch/oracle" 0x15b3000 0 > "$scratch/context"
"$scratch/oracle" 0 0x2000 > "$scratch/control"
"$scratch/oracle" 0x15b3000 0x4000 > "$scratch/both"
grep -q 'control=12288 .*context=22753280 ' "$scratch/context"
grep -q 'control=8192 .*context=22687744 ' "$scratch/control"
grep -q 'control=16384 .*context=22753280 ' "$scratch/both"
for values in '1 0' '4096 0' '0 1' '0 16384' '18446744073709551615 0' '0 18446744073709551615' '4294967296 0'; do
    # Only fixed literal numeric test cases are split here.
    set -- $values
    if "$scratch/oracle" "$1" "$2" > "$scratch/rejected" 2>&1; then
        printf 'unexpected oracle admission: %s\n' "$values" >&2
        exit 1
    fi
done
printf 'gfx950 oracle: source hashes, geometry, headers, shadows, precedence and negative controls passed\n'
