#!/usr/bin/env bash
set -euo pipefail

manifest=$(cd -- "$(dirname -- "$0")" && pwd)/source-files.sha256
root=${1:?usage: verify-pending-allocation.sh CHECKOUT OUTPUT_DIRECTORY}
out=${2:?usage: verify-pending-allocation.sh CHECKOUT OUTPUT_DIRECTORY}
out=$(realpath -- "$out")
cd -- "$root"

run() {
    local name=$1 status
    shift
    printf '%q ' "$@" > "$out/$name.command"
    printf '\n' >> "$out/$name.command"
    date -u +%FT%T.%NZ > "$out/$name.started"
    if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%T.%NZ > "$out/$name.finished"
    printf '%s\n' "$status" > "$out/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}

run source-before sha256sum --check "$manifest"
run rustc rustc -vV
run cargo cargo -V
run clippy cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
run formatting cargo fmt --all -- --check
for target in gnu musl; do
    args=()
    if [[ $target == musl ]]; then args=(--target x86_64-unknown-linux-musl); fi
    run "$target-build" cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib "${args[@]}" --no-run --message-format=json
    for crate in fe2o3_kfd fe2o3_runtime; do
        mapfile -t executables < <(jq -Rr --arg crate "$crate" 'fromjson? | select(.reason == "compiler-artifact" and .target.name == $crate and .profile.test == true) | .executable' "$out/$target-build.log")
        [[ ${#executables[@]} == 1 && -x ${executables[0]} ]]
        sha256sum "${executables[0]}" > "$out/$target-$crate-binary.sha256"
        if [[ $crate == fe2o3_runtime ]]; then
            run "$target-runtime-full" prlimit --core=0:0 -- "${executables[0]}" --test-threads=4
            grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$out/$target-runtime-full.log"
        else
            filters=(
                queue::dispatch_binding::tests::exact_epoch_authentication_rejects_every_identity_and_completion_substitution
                queue::dispatch_binding::tests::r66_published_occurrence_observation_preserves_digest_and_owner_state
                queue::live::tests::auxiliary_lane_reuse_advances_generation_and_rejects_substitution
                queue::live::compute_sdma_coexistence::tests::
            )
            for index in "${!filters[@]}"; do
                name="$target-kfd-focused-$index"
                run "$name" prlimit --core=0:0 -- "${executables[0]}" "${filters[$index]}" --test-threads=1
                grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$out/$name.log"
            done
        fi
    done
done
run no-default cargo check --locked --offline -p fe2o3-runtime --no-default-features
run unsafe-policy cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
run source-after sha256sum --check "$manifest"
