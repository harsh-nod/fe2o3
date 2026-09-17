#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
sha256sum --check "$archive/source-files.sha256"
mapfile -t sources < "$archive/source-files.list"
[[ ${#sources[@]} == 10 ]]
base=$(< "$archive/source-base.txt")
[[ $base == 13c5e5b8126dd461c5d7eb8972613d3ff6f4019b ]]
git diff --binary "$base" -- crates | cmp - "$archive/source.patch"
for name in gnu musl gnu-final-build musl-final-build gnu-final-runtime \
    gnu-binary musl-binary gnu-kfd-binary musl-kfd-binary \
    gnu-runtime-roster musl-runtime-roster gnu-kfd-roster musl-kfd-roster \
    clippy fmt no-default unsafe-policy doctests source-check parser-closed \
    remote-create remote-binary-upload remote-guard-upload \
    native-c4-cold native-c4-bootstrap native-i2-cold native-i2-bootstrap \
    remote-inventory-final remote-remove-files remote-remove-directory remote-absence; do
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
done
for exit_file in "$archive"/raw/*.exit; do
    name=${exit_file##*/}; name=${name%.exit}
    [[ $(< "$exit_file") == 0 ]]
    [[ -s "$archive/raw/$name.command" && -s "$archive/raw/$name.started" && -s "$archive/raw/$name.finished" ]]
    [[ $(< "$archive/raw/$name.started") < $(< "$archive/raw/$name.finished") ]]
done
for target in gnu musl; do
    for crate in kfd runtime; do
        binary=$(jq -Rr --arg name "fe2o3_$crate" 'fromjson? | select(.reason == "compiler-artifact" and .target.name == $name) | .executable // empty' "$archive/raw/$target-final-build.log")
        [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
        digest=$archive/raw/$target-binary.log
        [[ $crate == runtime ]] || digest=$archive/raw/$target-kfd-binary.log
        [[ $(awk '{print $2}' "$digest") == "$binary" ]]
        sha256sum --check "$digest"
        cmp "$archive/raw/gnu-$crate-roster.log" "$archive/raw/musl-$crate-roster.log"
        count=1403
        [[ $crate == kfd ]] || count=913
        passes=1403
        ignores=0
        if [[ $crate == runtime ]]; then passes=896; ignores=17; fi
        rg -qx "$count tests, 0 benchmarks" "$archive/raw/$target-$crate-roster.log"
        results=$archive/raw/$target.log
        if [[ $target == gnu && $crate == runtime ]]; then results=$archive/raw/gnu-final-runtime.log; fi
        parsed=$(awk -v count="$count" -v passes="$passes" -v ignores="$ignores" -f "$archive/parse-results.awk" "$results")
        diff -u <(sed -n 's/^\(.*\): test$/\1/p' "$archive/raw/$target-$crate-roster.log") \
            <(printf '%s\n' "$parsed" | LC_ALL=C sort)
        if [[ $crate == kfd ]]; then
            rg -q '^test result: ok\. 1403 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;' "$results"
        else
            rg -q '^test result: ok\. 896 passed; 0 failed; 17 ignored; 0 measured; 0 filtered out;' "$results"
        fi
    done
done
expected=b8ebfd9bd6c69622372c92a6c477b37d6141d4f01f0012da5496d754b42f3b3f
[[ $(awk '{print $1}' "$archive/raw/musl-binary.log") == "$expected" ]]
records=(native-c4-cold native-c4-bootstrap native-i2-cold native-i2-bootstrap)
names=(generated_native_cold_full_roster_readback generated_native_bootstrap_full_roster_readback generated_native_cold_issue_complete_readback_retire generated_native_bootstrap_issue_complete_readback_retire)
for index in 0 1 2 3; do
    name=${records[$index]}
    exact=kfd_backend::generated_adoption::tests::native::${names[$index]}
    rg -Fq "$expected $exact" "$archive/raw/$name.command"
    rg -Fq "admitted UID=0xab83d2ffef0d3cdf BDF=0000:26:00.0 exact_test=$exact" "$archive/raw/$name.log"
    rg -Fq "test $exact ..." "$archive/raw/$name.log"
    rg -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 912 filtered out;' "$archive/raw/$name.log"
    [[ $(jq -Rr 'fromjson? | .card1["VRAM Total Used Memory (B)"] // empty' "$archive/raw/$name.log") == $'298647552\n298647552' ]]
done
rg -Fq 'full_roster=true, primary/AUX/rebound, 12 DATA disposals' "$archive/raw/native-c4-cold.log"
rg -Fq 'full_roster=true, primary/AUX/rebound, 12 DATA disposals' "$archive/raw/native-c4-bootstrap.log"
[[ $(< "$archive/raw/native-i2-bootstrap.finished") < $(< "$archive/raw/remote-remove-files.started") ]]
rg -Fq '/tmp/fe2o3-c4-harsh-20260917.myDEsE' "$archive/raw/remote-create.log"
rg -Fq 'test \! -e /tmp/fe2o3-c4-harsh-20260917.myDEsE' "$archive/raw/remote-absence.command"
rg -q '^test result: ok\. 27 passed; 0 failed;' "$archive/raw/doctests.log"
rg -q '^test result: ok\. 33 passed; 0 failed;' "$archive/raw/doctests.log"
rg -q '^test result: ok\. 5 passed; 0 failed; 1 ignored;' "$archive/raw/unsafe-policy.log"
printf 'PASS: 10 source files; GNU/musl 1403 KFD + 896 runtime passes; 17 native ignores per target; 4 exact native probes; owned cleanup verified.\n'
