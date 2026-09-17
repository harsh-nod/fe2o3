#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/source-files.sha256" && ! -e "$archive/source.patch" && ! -e "$archive/SHA256SUMS" ]]
sources=(
    crates/fe2o3-runtime-model/src/context_read_leases/tests.rs
    crates/fe2o3-runtime-model/verus/context_read_commit_v1.rs
    crates/fe2o3-runtime-model/verus/check-read-commit.py
    crates/fe2o3-runtime-model/verus/check-negative-quality.py
    crates/fe2o3-runtime-model/verus/verify-verus.sh
    crates/fe2o3-runtime-model/verus/pins/CONTEXT_READ_COMMIT_SHA256
    crates/fe2o3-runtime-model/verus/pins/READ_COMMIT_CHECKER_SHA256
    crates/fe2o3-runtime-model/verus/pins/NEGATIVE_QUALITY_CHECKER_SHA256
    crates/fe2o3-runtime-model/verus/pins/TRANSCRIPT_SHA256
    docs/runtime-context-read-commit-v1.md
    docs/runtime-a1-a2-swarm-current.md
)
for path in "${sources[@]}"; do git ls-files --error-unmatch -- "$path" >/dev/null; done
printf '%s\n' "${sources[@]}" > "$archive/source-files.list"
sha256sum -- "${sources[@]}" > "$archive/source-files.sha256"
git diff HEAD --binary -- "${sources[@]}" > "$archive/source.patch"
sha256sum "$archive/source-files.sha256" "$archive/source.patch"
