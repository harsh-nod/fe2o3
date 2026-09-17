#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=$(< "$archive/source-base.txt")
[[ $base == 13c5e5b8126dd461c5d7eb8972613d3ff6f4019b ]]
[[ $(wc -l < "$archive/source-files.list") == 38 ]]
[[ -z $(git ls-files --others --exclude-standard -- crates scripts) ]]
sha256sum --check "$archive/source-files.sha256"
git diff --binary "$base" -- crates scripts | cmp - "$archive/source.patch"
git diff --cached --binary "$base" -- crates scripts | cmp - "$archive/prerequisite.patch"
git diff --binary -- crates scripts | cmp - "$archive/integration.patch"
diff -u "$archive/source-files.list" <(git diff --name-only "$base" -- crates scripts)
[[ $(sha256sum "$archive/source.patch" | cut -d ' ' -f 1) == a4bf87129e3775df70877fca47ac53b465ff9fd06c0951ad44da8d90640b55c8 ]]
[[ $(sha256sum "$archive/prerequisite.patch" | cut -d ' ' -f 1) == fd3f82ee6330210ec91157acfdbd9815ef913e1e0378fcbef7337a850be14c3b ]]

success() {
    [[ $(< "$archive/raw/$1.exit") == 0 ]]
    [[ -s "$archive/raw/$1.command" && -s "$archive/raw/$1.started" && -s "$archive/raw/$1.finished" ]]
}
command_is() {
    local name=$1
    shift
    diff -u "$archive/raw/$name.command" <(printf '%q ' "$@"; printf '\n')
}
before() { [[ $(< "$archive/raw/$1.finished") < $(< "$archive/raw/$2.started") ]]; }
for receipt in "$archive"/raw/*.command; do
    name=${receipt##*/}; name=${name%.command}
    [[ -s "$archive/raw/$name.exit" && -s "$archive/raw/$name.finished" ]]
    [[ $(< "$archive/raw/$name.started") < $(< "$archive/raw/$name.finished") ]]
    if [[ $name == musl-build ]]; then
        [[ $(< "$archive/raw/$name.exit") == 101 ]]
        rg -Fq 'failed to find tool "x86_64-linux-musl-gcc"' "$archive/raw/$name.log"
        jq -Rse '[split("\n")[] | fromjson? | select(.reason == "build-finished")] == [{"reason":"build-finished","success":false}]' "$archive/raw/$name.log" > /dev/null
    else
        success "$name"
    fi
done

profile=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0)
for target in gnu musl-no-hip; do
    build_env=(); target_args=()
    if [[ $target == musl-no-hip ]]; then
        build_env=(FE2O3_HIP_SYS_DISABLE=1)
        target_args=(--target x86_64-unknown-linux-musl)
    fi
    command_is "$target-build" "${profile[@]}" "${build_env[@]}" cargo test --locked --offline \
        -p fe2o3-host -p fe2o3-runtime --all-features --lib --no-run "${target_args[@]}" --message-format=json
    jq -Rse '[split("\n")[] | fromjson? | select(.reason == "build-finished")] == [{"reason":"build-finished","success":true}]' "$archive/raw/$target-build.log" > /dev/null
    for endpoint in before after; do
        command_is "$target-source-$endpoint" sha256sum --check "$archive/source-files.sha256"
    done
    before freeze "$target-source-before"
    before "$target-source-before" "$target-build"
    previous=$target-build
    for crate in host runtime; do
        binary=$(jq -Rr --arg name "fe2o3_$crate" 'fromjson? | select(.reason == "compiler-artifact" and
            .target.name == $name and .profile.test == true) | .executable // empty' "$archive/raw/$target-build.log")
        [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
        command_is "$target-$crate-binary" sha256sum "$binary"
        [[ $(awk '{print $2}' "$archive/raw/$target-$crate-binary.log") == "$binary" ]]
        sha256sum --check "$archive/raw/$target-$crate-binary.log"
        command_is "$target-$crate-roster" "$binary" --list
        command_is "$target-$crate" "$binary" --test-threads=1
        command_is "$target-$crate-binary-after" sha256sum --check "$archive/raw/$target-$crate-binary.log"
        if [[ $crate == host ]]; then count=266; passes=262; ignores=4; else count=925; passes=908; ignores=17; fi
        rg -qx "$count tests, 0 benchmarks" "$archive/raw/$target-$crate-roster.log"
        parsed=$(awk -v count="$count" -v passes="$passes" -v ignores="$ignores" \
            -f "$archive/parse-results.awk" "$archive/raw/$target-$crate.log")
        diff -u <(sed -n 's/^\(.*\): test$/\1/p' "$archive/raw/$target-$crate-roster.log" | LC_ALL=C sort) \
            <(printf '%s\n' "$parsed" | LC_ALL=C sort)
        cmp "$archive/raw/gnu-$crate-roster.log" "$archive/raw/musl-no-hip-$crate-roster.log"
        for record in "$target-$crate-binary" "$target-$crate-roster" "$target-$crate" "$target-$crate-binary-after"; do
            success "$record"
            before "$previous" "$record"
            previous=$record
        done
    done
    before "$previous" "$target-source-after"
done
command_is musl-build "${profile[@]}" cargo test --locked --offline -p fe2o3-host -p fe2o3-runtime \
    --all-features --lib --no-run --target x86_64-unknown-linux-musl --message-format=json
command_is musl-source-before sha256sum --check "$archive/source-files.sha256"
before freeze musl-source-before
before musl-source-before musl-build
[[ ! -e "$archive/raw/musl-host.command" && ! -e "$archive/raw/musl-runtime.command" ]]
command_is clippy "${profile[@]}" cargo clippy --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features --all-targets -- -D warnings
command_is fmt cargo fmt --all -- --check
command_is no-default "${profile[@]}" cargo check --locked --offline -p fe2o3-runtime --no-default-features
command_is unsafe-policy "${profile[@]}" cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
command_is doctests "${profile[@]}" cargo test --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features --doc
command_is parser bash "$archive/check-parser.sh"
command_is gates-source-before sha256sum --check "$archive/source-files.sha256"
command_is gates-source-after sha256sum --check "$archive/source-files.sha256"
previous=gates-source-before
for gate in clippy fmt no-default unsafe-policy doctests gates-source-after; do
    success "$gate"
    before "$previous" "$gate"
    previous=$gate
done
success parser
bash "$archive/check-parser.sh"
awk '/^test result:/ { if ($0 !~ /^test result: ok\. (2|15|36) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;/ || seen[$4]++) exit 1; rows++ }
    END { if (rows != 3 || !seen[2] || !seen[15] || !seen[36]) exit 1 }' "$archive/raw/doctests.log"
rg -q '^test result: ok\. 5 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;' "$archive/raw/unsafe-policy.log"
printf 'PASS: exact frozen C4 source and GNU/musl-no-hip CPU rosters; host 262+4 ignored, runtime 908+17 ignored per target; quality gates and 53 doctests. No native integration, formal or milestone acceptance.\n'
