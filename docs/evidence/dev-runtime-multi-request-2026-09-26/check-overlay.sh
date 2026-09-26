#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
prior="$repo/docs/evidence/dev-runtime-request-witness-2026-09-26/qualification"
copy=${1:?qualification source directory required}
cd -- "$repo"
find crates/fe2o3-resource-accounting crates/fe2o3-runtime-model crates/fe2o3-kfd crates/fe2o3-runtime \
    -type f | LC_ALL=C sort > "$packet/raw/source-files.list"
xargs -d '\n' sha256sum < "$packet/raw/source-files.list" > "$packet/raw/sources.sha256"
sed -n 's@^+++ b/@@p' "$prior/overlay.patch" "$packet/qualification/overlay.patch" \
    > "$packet/raw/overlay-files.list"
rg -F -x -v -f "$packet/raw/overlay-files.list" "$packet/raw/source-files.list" \
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
done < "$packet/raw/overlay-files.list" > "$packet/raw/observed-overlay.patch"
# Production line numbers may move; every context/addition/removal must match.
sed '/^@@ /d' "$prior/overlay.patch" "$packet/qualification/overlay.patch" > "$packet/raw/expected-overlay.normalized"
sed '/^@@ /d' "$packet/raw/observed-overlay.patch" > "$packet/raw/observed-overlay.normalized"
cmp "$packet/raw/expected-overlay.normalized" "$packet/raw/observed-overlay.normalized"
while IFS=$'\t' read -r source target; do
    cmp "$prior/$source" "$copy/$target"
done < "$prior/files.tsv"
cmp "$packet/qualification/multi-tests.rs" "$copy/crates/fe2o3-runtime/src/kfd_backend/multi_admission/qualification.rs"
cmp "$packet/qualification/cold-tests.rs" "$copy/crates/fe2o3-runtime/src/kfd_backend/tests/sdma_allocation_tests/qualification.rs"
{
    cat "$packet/raw/source-files.list"
    cut -f2 "$prior/files.tsv"
    printf '%s\n' crates/fe2o3-runtime/src/kfd_backend/multi_admission/qualification.rs \
        crates/fe2o3-runtime/src/kfd_backend/tests/sdma_allocation_tests/qualification.rs
} | LC_ALL=C sort > "$packet/raw/overlay-expected-files.list"
(
    cd -- "$copy"
    find crates/fe2o3-resource-accounting crates/fe2o3-runtime-model crates/fe2o3-kfd crates/fe2o3-runtime \
        -type f | LC_ALL=C sort
) > "$packet/raw/overlay-actual-files.list"
cmp "$packet/raw/overlay-expected-files.list" "$packet/raw/overlay-actual-files.list"
printf 'Only recorded accounting/test fixtures differ from the four production package trees.\n'
