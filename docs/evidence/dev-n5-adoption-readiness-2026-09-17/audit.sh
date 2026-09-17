#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
sha256sum --check "$archive/source-files.sha256"
base=$(<"$archive/source-base.txt")
test "$base" = ad622fe331cd97d6d2da5db260221dd8c9f76432
git diff "$base" --binary -- crates/fe2o3-runtime/src | cmp - "$archive/source.patch"
test "$(wc -l < "$archive/source-files.sha256")" = 9
cmp <(git diff "$base" --name-only -- crates/fe2o3-runtime/src | LC_ALL=C sort) \
    <(sed -E 's/^[0-9a-f]{64}  //' "$archive/source-files.sha256")

check_command() {
    local name=$1
    shift
    { printf '%q ' "$@"; printf '\n'; } | cmp - "$archive/raw/$name.command"
}
gnu=target/debug/deps/fe2o3_runtime-d209755ce6df5405
musl=target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-6cd4f20cb887eaec
check_command preliminary-binary sha256sum "$gnu"
check_command preliminary-fairness prlimit --core=0:0 -- "$gnu" adoption_readiness_waits_fairly --nocapture --test-threads=1
check_command preliminary-terminal-drop prlimit --core=0:0 -- "$gnu" generated_readiness_waits_for_leases --nocapture --test-threads=1
check_command gnu-final prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-runtime --all-features --lib -- --test-threads=4
check_command musl-final prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-runtime --all-features --lib --target x86_64-unknown-linux-musl -- --test-threads=4
for target in gnu musl; do
    binary=${!target}
    check_command "$target-roster" "$binary" --list
    check_command "$target-binary" sha256sum "$binary"
    sha256sum "$binary" | cmp - "$archive/raw/$target-binary.log"
    rg -F "Running unittests src/lib.rs ($binary)" "$archive/raw/$target-final.log"
done
check_command clippy env CARGO_INCREMENTAL=0 cargo clippy --locked --offline -p fe2o3-runtime --all-features --all-targets -- -D warnings
check_command fmt cargo fmt --all -- --check
check_command doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-runtime --all-features --doc
check_command no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline -p fe2o3-runtime --no-default-features
check_command unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
commands=("$archive"/raw/*.command)
test "${#commands[@]}" = 14
for command in "$archive"/raw/*.command; do
    prefix=${command%.command}
    for suffix in started finished exit log; do test -f "$prefix.$suffix"; done
    started=$(<"$prefix.started")
    finished=$(<"$prefix.finished")
    date -d "$started" >/dev/null
    date -d "$finished" >/dev/null
    [[ "$started" < "$finished" || "$started" == "$finished" ]]
    status=$(<"$prefix.exit")
    case ${prefix##*/} in
        preliminary-fairness) test "$status" = 101 ;;
        preliminary-terminal-drop) test "$status" = 134 ;;
        *) test "$status" = 0 ;;
    esac
done
[[ $(<"$archive/raw/preliminary-binary.finished") < $(<"$archive/raw/gnu-final.started") ]]
[[ $(<"$archive/raw/gnu-final.finished") < $(<"$archive/raw/musl-final.started") ]]
for name in gnu-final musl-final; do
    test "$(rg -c '^test result:' "$archive/raw/$name.log")" = 1
    rg -F 'test result: ok. 869 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out;' "$archive/raw/$name.log"
    test "$(rg -c '^test .* \.\.\. ok$' "$archive/raw/$name.log")" = 869
    test "$(rg -c '^test .* \.\.\. ignored' "$archive/raw/$name.log")" = 13
    cmp <(sed -nE 's/^test (.*) \.\.\. (ok|ignored.*)$/\1/p' "$archive/raw/$name.log" | LC_ALL=C sort) \
        <(sed -n 's/: test$//p' "$archive/raw/gnu-roster.log" | LC_ALL=C sort -u)
    for test in \
        adoption_readiness_waits_fairly_without_repeating_preflight_or_transferring_payload \
        adoption_readiness_error_or_panic_retains_custody_without_retry \
        adoption_readiness_owned_waiting_stop_and_drain_retire_empty_prefix \
        generated_readiness_requires_exact_empty_hold_without_changing_credits \
        generated_readiness_waits_for_leases_without_mutation_or_poison \
        generated_readiness_observes_both_active_and_pipeline_lanes_without_progress \
        nonpersistent_compute_waits_for_active_persistent_compute; do
        rg -F "::$test ... ok" "$archive/raw/$name.log"
    done
    rg -F 'generated_native_cold_primary_auxiliary_rebound_abort_and_shutdown ... ignored' "$archive/raw/$name.log"
    rg -F 'generated_native_bootstrap_primary_auxiliary_rebound_abort_and_shutdown ... ignored' "$archive/raw/$name.log"
done
cmp "$archive/raw/gnu-roster.log" "$archive/raw/musl-roster.log"
rg -F '882 tests, 0 benchmarks' "$archive/raw/gnu-roster.log"
rg -F '33 passed; 0 failed' "$archive/raw/doctests.log"
rg -F '5 passed; 0 failed; 1 ignored' "$archive/raw/unsafe-policy.log"
if [[ -f "$archive/SHA256SUMS" ]]; then
    (cd -- "$archive" && sha256sum --check --status SHA256SUMS)
fi
printf 'N5 readiness development source and command-result audit passed; no native execution claimed.\n'
