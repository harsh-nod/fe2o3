#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=$(< "$archive/source-base.txt")
[[ $base == c29f4f46aed32a0317d5d1cfd39dfdbaa495cf57 ]]
[[ $(wc -l < "$archive/source-files.list") == 4 ]]
[[ -z $(git ls-files --others --exclude-standard -- crates scripts) ]]
sha256sum --check "$archive/source-files.sha256"
diff -u "$archive/source-files.list" <(sed 's/^[0-9a-f]\{64\}  //' "$archive/source-files.sha256")
git diff --binary "$base" -- crates scripts | cmp - "$archive/source.patch"
diff -u "$archive/source-files.list" <(git diff --name-only "$base" -- crates scripts)

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
    [[ -s "$archive/raw/$name.exit" && -s "$archive/raw/$name.started" && -s "$archive/raw/$name.finished" ]]
    [[ $(< "$archive/raw/$name.started") < $(< "$archive/raw/$name.finished") ]]
    case "$name" in
        exploratory-settled-stop)
            [[ $(< "$archive/raw/$name.exit") == 101 ]]
            rg -Fq 'mode 0: settled Stop retained Context' "$archive/raw/$name.log"
            rg -Fq 'test result: FAILED. 0 passed; 1 failed;' "$archive/raw/$name.log"
            ;;
        exploratory-owned-stop)
            [[ $(< "$archive/raw/$name.exit") == 101 ]]
            rg -Fq 'test result: FAILED. 20 passed; 4 failed;' "$archive/raw/$name.log"
            ;;
        *) success "$name" ;;
    esac
done
for receipt in "$archive"/raw/*.exit; do
    [[ -s ${receipt%.exit}.command && -s ${receipt%.exit}.started && -s ${receipt%.exit}.finished ]]
done

profile=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0)
command_is freeze bash "$archive/freeze-source.sh"
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
        if [[ $crate == host ]]; then count=275; passes=271; ignores=4; else count=959; passes=942; ignores=17; fi
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
command_is clippy "${profile[@]}" cargo clippy --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features --all-targets -- -D warnings
command_is fmt cargo fmt --all -- --check
command_is no-default "${profile[@]}" cargo check --locked --offline -p fe2o3-host -p fe2o3-runtime --no-default-features
command_is unsafe-policy "${profile[@]}" cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
command_is doctests "${profile[@]}" cargo test --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features --doc
command_is parser bash "$archive/check-parser.sh"
command_is gates-source-before sha256sum --check "$archive/source-files.sha256"
command_is gates-source-after sha256sum --check "$archive/source-files.sha256"
before freeze gates-source-before
before gnu-source-after musl-no-hip-source-before
before musl-no-hip-source-after gates-source-before
before gates-source-after parser
previous=gates-source-before
for gate in clippy fmt no-default unsafe-policy doctests gates-source-after; do
    success "$gate"
    before "$previous" "$gate"
    previous=$gate
done
success parser
bash "$archive/check-parser.sh"
awk '/^test result:/ { if ($0 !~ /^test result: ok\. (3|16|1|41) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;/ || seen[$4]++) exit 1; rows++ }
    END { if (rows != 4 || !seen[3] || !seen[16] || !seen[1] || !seen[41]) exit 1 }' "$archive/raw/doctests.log"
rg -q '^test result: ok\. 5 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;' "$archive/raw/unsafe-policy.log"
printf 'PASS: exact frozen C6 owned-Stop source and GNU/musl-no-hip CPU rosters; host 271+4 ignored, runtime 942+17 ignored per target; quality gates and 61 doctests. No native integration, formal or milestone acceptance.\n'
