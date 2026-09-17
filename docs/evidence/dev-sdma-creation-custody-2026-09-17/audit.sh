#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
sha256sum --check "$archive/source-files.sha256"
base=$(<"$archive/source-base.txt")
test "$base" = 5db1450574a12053d46d890a0bfdc2a8c4f17e9c
git diff "$base" --binary -- crates/fe2o3-kfd/src | cmp - "$archive/source.patch"
test "$(wc -l < "$archive/source-files.sha256")" = 8
cmp <(git diff "$base" --name-only -- crates | LC_ALL=C sort) \
    <(sed -E 's/^[0-9a-f]{64}  //' "$archive/source-files.sha256")
check_command() {
    local name=$1
    shift
    { printf '%q ' "$@"; printf '\n'; } | cmp - "$archive/raw/$name.command"
}
roster() { sed -n 's/: test$//p' "$1" | LC_ALL=C sort; }
results() { sed -nE 's/^test (.*) \.\.\. (ok|ignored.*)$/\1/p' "$1" | LC_ALL=C sort; }
check_roster() {
    local file=$1 count=$2
    test "$(roster "$file" | wc -l)" = "$count"
    cmp <(roster "$file") <(roster "$file" | LC_ALL=C sort -u)
    test "$(rg -c "^$count tests, 0 benchmarks$" "$file")" = 1
    awk '!/: test$/ && !/^[0-9]+ tests, 0 benchmarks$/ && NF {exit 1}' "$file"
}
check_results() {
    local file=$1
    cmp <(results "$file") <(results "$file" | LC_ALL=C sort -u)
    awk '/^test / && !/^test result:/ && !/ \.\.\. (ok|ignored.*)$/ &&
        !/ has been running for over [0-9]+ seconds$/ {exit 1}' "$file"
}
filters=(
    queue::live::model_loan::tests::
    queue::live::tests::
    sdma::tests::
    queue_linux::tests::
    shared_memory::tests::live_foundation_
    shared_memory::tests::panic_before_and_after_allocation_map_projection_remains_retakeable
    queue::live::construction_primary::integration_tests::release_cases::sdma_creation_cases::
)
selected_roster() {
    roster "$1" | awk -v filters="$(printf '%s\n' "${filters[@]}")" '
        BEGIN { n = split(filters, parts, "\n") }
        { for (i = 1; i <= n; i++) if (index($0, parts[i])) { print; break } }
    '
}
names=(focused-preliminary focused-revised focused-corrected clippy
    gnu-build gnu-roster gnu-binary gnu-selected)
for name in focused-preliminary focused-revised focused-corrected; do
    check_command "$name" prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 \
        cargo test --locked --offline -p fe2o3-kfd --all-features --lib sdma_creation -- --test-threads=4
done
check_command clippy env CARGO_INCREMENTAL=0 cargo clippy --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
rg -F 'error[E0277]' "$archive/raw/focused-preliminary.log" >/dev/null
rg -F '7 passed; 1 failed' "$archive/raw/focused-revised.log"
rg -F '11 passed; 0 failed' "$archive/raw/focused-corrected.log"
rg -F 'clippy::result_large_err' "$archive/raw/clippy.log" >/dev/null
check_command gnu-build env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p fe2o3-kfd --all-features --lib --no-run --message-format=json
gnu=target/debug/deps/fe2o3_kfd-5074a99fa693d94b
check_command gnu-roster "$gnu" --list
check_command gnu-binary sha256sum "$gnu"
check_command gnu-selected prlimit --core=0:0 -- "$gnu" "${filters[@]:0:6}" \
    queue::live::construction_primary:: queue::live::construction_auxiliary:: --test-threads=4
check_roster "$archive/raw/gnu-roster.log" 1400
test "$(rg -c '^queue::live::.*::sdma_creation_cases::.*: test$' "$archive/raw/gnu-roster.log")" = 11
test "$(rg -c '^test result:' "$archive/raw/gnu-selected.log" || true)" = ''
for target in gnu musl; do
    flags=()
    if [[ $target == musl ]]; then flags=(--target x86_64-unknown-linux-musl); fi
    architecture=$target
    target=$target-final
    names+=("$target-build" "$target-roster" "$target-binary" "$target-selected"
        "$target-runtime" "$target-runtime-roster" "$target-runtime-binary")
    check_command "$target-build" env CARGO_INCREMENTAL=0 cargo test --locked --offline \
        -p fe2o3-kfd --all-features --lib "${flags[@]}" --no-run --message-format=json
    binary=$(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_kfd") | .executable // empty' "$archive/raw/$target-build.log")
    [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
    binary=${binary#"$root/"}
    case $architecture in
        gnu) [[ $binary == target/debug/deps/fe2o3_kfd-* ]] ;;
        musl) [[ $binary == target/x86_64-unknown-linux-musl/debug/deps/fe2o3_kfd-* ]] ;;
    esac
    jq -Rse --arg root "$root" --arg binary "$root/$binary" '
        [split("\n")[] | fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_kfd")] as $a |
        ($a | length) == 1 and
        ($a[0] | .package_id == ("path+file://" + $root + "/crates/fe2o3-kfd#0.1.0") and
            .manifest_path == ($root + "/crates/fe2o3-kfd/Cargo.toml") and
            .target.src_path == ($root + "/crates/fe2o3-kfd/src/lib.rs") and
            .target.kind == ["lib"] and .target.crate_types == ["lib"] and
            .profile.test == true and .features == ["default", "live-validation"] and
            .executable == $binary and (.filenames | index($binary) != null))
    ' "$archive/raw/$target-build.log" >/dev/null
    check_command "$target-roster" "$binary" --list
    check_command "$target-binary" sha256sum "$binary"
    sha256sum "$binary" | cmp - "$archive/raw/$target-binary.log"
    check_roster "$archive/raw/$target-roster.log" 1400
    cmp "$archive/raw/gnu-roster.log" "$archive/raw/$target-roster.log"
    check_results "$archive/raw/$target-selected.log"
    check_command "$target-selected" prlimit --core=0:0 -- "$binary" "${filters[@]}" --test-threads=4
    cmp <(selected_roster "$archive/raw/$target-roster.log") \
        <(results "$archive/raw/$target-selected.log")
    count=$(selected_roster "$archive/raw/$target-roster.log" | wc -l)
    total=$(roster "$archive/raw/$target-roster.log" | wc -l)
    test "$(rg -c '^test .* \.\.\. ok$' "$archive/raw/$target-selected.log")" = "$count"
    test "$(rg -c '^test result:' "$archive/raw/$target-selected.log")" = 1
    rg -F "test result: ok. $count passed; 0 failed; 0 ignored; 0 measured; $((total-count)) filtered out;" "$archive/raw/$target-selected.log"
    test "$(rg -c '^test .*::sdma_creation_cases::.* \.\.\. ok$' "$archive/raw/$target-selected.log")" = 11
    check_command "$target-runtime" prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 \
        cargo test --locked --offline -p fe2o3-runtime --all-features --lib "${flags[@]}" -- --test-threads=4
    runtime=$(sed -n 's/.*Running unittests src\/lib.rs (\(.*\)).*/\1/p' "$archive/raw/$target-runtime.log")
    [[ -n $runtime && $runtime != *$'\n'* && -x $runtime ]]
    case $architecture in
        gnu) [[ $runtime == target/debug/deps/fe2o3_runtime-* ]] ;;
        musl) [[ $runtime == target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-* ]] ;;
    esac
    check_command "$target-runtime-roster" "$runtime" --list
    check_command "$target-runtime-binary" sha256sum "$runtime"
    sha256sum "$runtime" | cmp - "$archive/raw/$target-runtime-binary.log"
    check_roster "$archive/raw/$target-runtime-roster.log" 882
    check_results "$archive/raw/$target-runtime.log"
    cmp <(roster "$archive/raw/$target-runtime-roster.log") \
        <(results "$archive/raw/$target-runtime.log")
    test "$(rg -c '^test .* \.\.\. ok$' "$archive/raw/$target-runtime.log")" = 869
    test "$(rg -c '^test .* \.\.\. ignored' "$archive/raw/$target-runtime.log")" = 13
    test "$(rg -c '^test result:' "$archive/raw/$target-runtime.log")" = 1
    rg -F 'test result: ok. 869 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out;' "$archive/raw/$target-runtime.log"
    [[ $(<"$archive/raw/$target-build.finished") < $(<"$archive/raw/$target-selected.started") ]]
    [[ $(<"$archive/raw/$target-selected.finished") < $(<"$archive/raw/$target-runtime.started") ]]
done
cmp "$archive/raw/gnu-final-roster.log" "$archive/raw/musl-final-roster.log"
cmp "$archive/raw/gnu-binary.log" "$archive/raw/gnu-final-binary.log"
cmp "$archive/raw/gnu-final-runtime-roster.log" "$archive/raw/musl-final-runtime-roster.log"
names+=(clippy-final fmt doctests no-default unsafe-policy)
check_command clippy-final env CARGO_INCREMENTAL=0 cargo clippy --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
check_command fmt cargo fmt --all -- --check
check_command doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --all-features --doc
check_command no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime --no-default-features
check_command unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p cargo-fe2o3 --test unsafe_source_policy
cmp <(printf '%s\n' "${names[@]}" | LC_ALL=C sort) \
    <(find "$archive/raw" -maxdepth 1 -name '*.command' -printf '%f\n' | sed 's/\.command$//' | LC_ALL=C sort)
test "$(find "$archive/raw" -maxdepth 1 -type f | wc -l)" = "$((${#names[@]} * 5))"
for name in "${names[@]}"; do
    prefix=$archive/raw/$name
    for suffix in command started finished exit log; do test -f "$prefix.$suffix"; done
    started=$(<"$prefix.started")
    finished=$(<"$prefix.finished")
    date -d "$started" >/dev/null
    date -d "$finished" >/dev/null
    [[ $started < $finished || $started == $finished ]]
    status=$(<"$prefix.exit")
    case $name in
        focused-preliminary|focused-revised|clippy) test "$status" = 101 ;;
        gnu-selected) test "$status" = 143 ;;
        *) test "$status" = 0 ;;
    esac
done
previous=gnu-selected
for name in "${names[@]:8}"; do
    [[ $(<"$archive/raw/$previous.finished") < $(<"$archive/raw/$name.started") ]]
    previous=$name
done
for name in focused-preliminary focused-revised focused-corrected clippy; do
    [[ $(<"$archive/raw/$name.finished") < $(<"$archive/raw/gnu-build.started") ]]
done
test "$(rg -c '^test result:' "$archive/raw/doctests.log")" = 2
test "$(rg -c '^test .* \.\.\. ok$' "$archive/raw/doctests.log")" = 60
rg -F '27 passed; 0 failed' "$archive/raw/doctests.log"
rg -F '33 passed; 0 failed' "$archive/raw/doctests.log"
rg -F '5 passed; 0 failed; 1 ignored' "$archive/raw/unsafe-policy.log"
if [[ -f "$archive/SHA256SUMS" ]]; then
    (
        cd -- "$archive"
        cmp <(find . -type f ! -name SHA256SUMS -printf '%P\n' | LC_ALL=C sort) \
            <(sed -E 's/^[0-9a-f]{64}  //' SHA256SUMS)
        sha256sum --check --status SHA256SUMS
    )
fi
printf 'SDMA creation development source and result audit passed; no native execution claimed.\n'
