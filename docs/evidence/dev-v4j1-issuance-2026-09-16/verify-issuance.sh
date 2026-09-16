#!/usr/bin/env bash
set -euo pipefail

receipt=$(cd -- "$(dirname -- "$0")" && pwd)
root=${1:?usage: verify-issuance.sh CHECKOUT OUTPUT_DIRECTORY VERUS}
out=$(realpath -- "${2:?output directory required}")
verus=$(realpath -- "${3:?pinned Verus executable required}")
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

proof=crates/fe2o3-runtime-model/verus
run source-before sha256sum --check "$receipt/source-files.sha256"
run rustc rustc -vV
run cargo cargo -V
run python python3 --version
run helper-selftest python3 -I "$proof/check-journal-issuance.py" --self-test "$proof/context_version_journal_issuance_v1.rs"
run negative-quality-selftest python3 -I "$proof/check-negative-quality.py" --self-test \
    "$proof/tests/fixtures/negative-quality-direct-literal.rs" "$proof/tests/fixtures/negative-quality-adverse-input.rs"
run negative-quality-inventory python3 -I "$proof/check-negative-quality.py" "$proof/negative" "$proof/verify-verus.sh"
run formatting cargo fmt --all -- --check
run clippy cargo clippy --locked --offline -p fe2o3-runtime-model --all-features --all-targets -- -D warnings
for target in gnu musl; do
    args=()
    if [[ $target == musl ]]; then args=(--target x86_64-unknown-linux-musl); fi
    run "$target-build" cargo test --locked --offline -p fe2o3-runtime-model --all-features --lib "${args[@]}" --no-run --message-format=json
    mapfile -t executables < <(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_runtime_model" and .profile.test == true) | .executable' "$out/$target-build.log")
    [[ ${#executables[@]} == 1 && -x ${executables[0]} ]]
    sha256sum "${executables[0]}" > "$out/$target-binary.sha256"
    run "$target-roster" "${executables[0]}" --list
    run "$target-full" prlimit --core=0:0 -- "${executables[0]}" --test-threads=4
    grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$out/$target-full.log"
done
run no-default cargo check --locked --offline -p fe2o3-runtime-model --no-default-features
run unsafe-policy cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
run journal-campaign prlimit --core=0:0 -- python3 -I "$proof/check-journal-issuance.py" \
    "$proof/context_version_journal_issuance_v1.rs" "$verus" 120 "$out/mutations"
run full-proof-gate prlimit --core=0:0 -- env VERUS="$verus" "$proof/verify-verus.sh"
run source-after sha256sum --check "$receipt/source-files.sha256"
