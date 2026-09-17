#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/corrected-source-base.txt" && ! -e "$archive/SHA256SUMS" ]]
base=$(git rev-parse HEAD)
[[ $base == 13c5e5b8126dd461c5d7eb8972613d3ff6f4019b ]]
git diff --quiet -- crates
mapfile -t sources < <(git diff --cached --name-only "$base" -- crates)
expected=(
    crates/fe2o3-kfd/src/queue.rs
    crates/fe2o3-kfd/src/queue_dispatch_binding.rs
    crates/fe2o3-kfd/src/queue_live.rs
    crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs
    crates/fe2o3-kfd/src/semantic_observation.rs
    crates/fe2o3-runtime/src/context/generated_preparation/tests.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption/issue.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption/readback.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption/readback/tests.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption/tests.rs
    crates/fe2o3-runtime/src/kfd_backend/generated_adoption/tests/native.rs
    crates/fe2o3-service-host/src/queue.rs
)
[[ ${#sources[@]} == ${#expected[@]} ]]
diff -u <(printf '%s\n' "${expected[@]}") <(printf '%s\n' "${sources[@]}")
printf '%s\n' "$base" > "$archive/corrected-source-base.txt"
printf '%s\n' "${sources[@]}" > "$archive/corrected-source-files.list"
sha256sum "${sources[@]}" > "$archive/corrected-source-files.sha256"
git diff --binary "$base" -- crates > "$archive/corrected-source.patch"
sha256sum "$archive"/corrected-source*
