#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
prior="$repo/docs/evidence/dev-runtime-request-witness-2026-09-26/qualification"
copy=${1:?qualification source directory required}
cd -- "$repo"
sha256sum -c "$packet/qualification/inherited.sha256" > "$packet/raw/inherited-check.log"
inputs=(Cargo.toml Cargo.lock rust-toolchain.toml crates examples)
find "${inputs[@]}" ! -type f ! -type d -print > "$packet/raw/source-nonordinary.list"
test ! -s "$packet/raw/source-nonordinary.list"
(
    cd -- "$copy"
    find "${inputs[@]}" ! -type f ! -type d -print
) > "$packet/raw/overlay-nonordinary.list"
test ! -s "$packet/raw/overlay-nonordinary.list"
find "${inputs[@]}" \
    -type f | LC_ALL=C sort > "$packet/raw/source-files.list"
xargs -d '\n' sha256sum < "$packet/raw/source-files.list" > "$packet/raw/sources.sha256"
sed -n 's@^+++ b/@@p' "$prior/overlay.patch" "$packet/qualification/overlay.patch" > "$packet/raw/overlay-files.list"
rg -F -x -v -f "$packet/raw/overlay-files.list" "$packet/raw/source-files.list" > "$packet/raw/overlay-unchanged-files.list"
xargs -d '\n' sha256sum < "$packet/raw/overlay-unchanged-files.list" > "$packet/raw/overlay-unchanged.sha256"
(
    cd -- "$copy"
    sha256sum -c "$packet/raw/overlay-unchanged.sha256"
) > "$packet/raw/overlay-unchanged-check.log"
while IFS= read -r path; do
    status=0
    diff -u --label "a/$path" --label "b/$path" "$path" "$copy/$path" || status=$?
    if (( status != 1 )); then exit 2; fi
done < "$packet/raw/overlay-files.list" > "$packet/raw/observed-overlay.patch"
sed '/^@@ /d' "$prior/overlay.patch" "$packet/qualification/overlay.patch" > "$packet/raw/expected-overlay.normalized"
sed '/^@@ /d' "$packet/raw/observed-overlay.patch" > "$packet/raw/observed-overlay.normalized"
cmp "$packet/raw/expected-overlay.normalized" "$packet/raw/observed-overlay.normalized"
for fixtures in "$prior" "$packet/qualification"; do
    while IFS=$'\t' read -r source target; do
        cmp "$fixtures/$source" "$copy/$target"
    done < "$fixtures/files.tsv"
done
{
    cat "$packet/raw/source-files.list"
    cut -f2 "$prior/files.tsv" "$packet/qualification/files.tsv"
} | LC_ALL=C sort > "$packet/raw/overlay-expected-files.list"
(
    cd -- "$copy"
    find "${inputs[@]}" \
        -type f | LC_ALL=C sort
) > "$packet/raw/overlay-actual-files.list"
cmp "$packet/raw/overlay-expected-files.list" "$packet/raw/overlay-actual-files.list"
printf 'All copied Cargo inputs match except the recorded accounting/model/test fixtures.\n'
