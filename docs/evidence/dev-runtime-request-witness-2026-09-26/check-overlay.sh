#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
copy=${1:?qualification source directory required}
cd -- "$repo"
rg -F -x -v -f "$packet/qualification/overlay-files.list" "$packet/raw/source-files.list" \
    > "$packet/raw/overlay-unchanged-files.list"
xargs -d '\n' sha256sum < "$packet/raw/overlay-unchanged-files.list" \
    > "$packet/raw/overlay-unchanged.sha256"
(
    cd -- "$copy"
    sha256sum -c "$packet/raw/overlay-unchanged.sha256"
) > "$packet/raw/overlay-unchanged-check.log"
while IFS= read -r path; do
    status=0
    diff -u --label "a/$path" --label "b/$path" "$path" "$copy/$path" || status=$?
    if (( status != 1 )); then exit 2; fi
done < "$packet/qualification/overlay-files.list" > "$packet/raw/observed-overlay.patch"
cmp "$packet/qualification/overlay.patch" "$packet/raw/observed-overlay.patch"
while IFS=$'\t' read -r source target; do
    cmp "$packet/qualification/$source" "$copy/$target"
done < "$packet/qualification/files.tsv"
{
    cat "$packet/raw/source-files.list"
    cut -f2 "$packet/qualification/files.tsv"
} | LC_ALL=C sort > "$packet/raw/overlay-expected-files.list"
(
    cd -- "$copy"
    find crates/fe2o3-resource-accounting crates/fe2o3-runtime-model crates/fe2o3-kfd crates/fe2o3-runtime \
        -type f | LC_ALL=C sort
) > "$packet/raw/overlay-actual-files.list"
cmp "$packet/raw/overlay-expected-files.list" "$packet/raw/overlay-actual-files.list"
(
    cd -- "$copy"
    xargs -d '\n' sha256sum < "$packet/raw/overlay-actual-files.list"
    sha256sum Cargo.toml Cargo.lock rust-toolchain.toml
) > "$packet/raw/overlay-sources.sha256"
printf 'Only the recorded fixture additions differ from the four production package trees.\n'
