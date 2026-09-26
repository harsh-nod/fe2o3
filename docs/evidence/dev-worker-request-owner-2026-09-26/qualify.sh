#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
prior="$repo/docs/evidence/dev-runtime-request-witness-2026-09-26/qualification"
destination=${1:?new isolated source directory required}
cd -- "$repo"
sha256sum -c "$packet/qualification/inherited.sha256"
mkdir -- "$destination"
mkdir -p -- "$packet/raw"
cp -a "$repo/Cargo.toml" "$repo/Cargo.lock" "$repo/rust-toolchain.toml" \
    "$repo/crates" "$repo/examples" "$destination/"
cd -- "$destination"
patch --batch --fuzz=0 --no-backup-if-mismatch -p1 < "$prior/overlay.patch"
while IFS=$'\t' read -r source target; do
    mkdir -p -- "$(dirname -- "$target")"
    cp -- "$prior/$source" "$target"
done < "$prior/files.tsv"
patch --batch --fuzz=0 --no-backup-if-mismatch -p1 < "$packet/qualification/overlay.patch"
while IFS=$'\t' read -r source target; do
    mkdir -p -- "$(dirname -- "$target")"
    cp -- "$packet/qualification/$source" "$target"
done < "$packet/qualification/files.tsv"
bash "$packet/check-overlay.sh" "$destination"
bash "$packet/test-qualification.sh" "$destination"
