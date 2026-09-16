#!/usr/bin/env bash
set -euo pipefail

root=/home/harsh/.codex-tmp/fe2o3-r61-execution
out=/home/harsh/.codex-tmp/r126-runtime-sync.Q2AAUj
cd "$root"

run() {
    local name=$1 status
    shift
    printf '%q ' "$@" > "$out/$name.command"
    printf '\n' >> "$out/$name.command"
    date -u +%FT%TZ > "$out/$name.started"
    if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%TZ > "$out/$name.finished"
    printf '%s\n' "$status" > "$out/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}

run source-before sha256sum --check "$out/source-files.sha256"
for target in gnu musl; do
    args=()
    if [[ $target == musl ]]; then args=(--target x86_64-unknown-linux-musl); fi
    run "$target-build" cargo test --locked --offline -p fe2o3-runtime --all-features --lib "${args[@]}" --no-run --message-format=json
    mapfile -t executables < <(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_runtime" and .profile.test == true) | .executable' "$out/$target-build.log")
    [[ ${#executables[@]} == 1 && -x ${executables[0]} ]]
    sha256sum "${executables[0]}" > "$out/$target-binary.sha256"
    run "$target-runtime-full" prlimit --core=0:0 -- "${executables[0]}" --test-threads=4
done
run no-default cargo check --locked --offline -p fe2o3-runtime --no-default-features
run unsafe-policy cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
run whitespace git diff --check HEAD
run source-after sha256sum --check "$out/source-files.sha256"
