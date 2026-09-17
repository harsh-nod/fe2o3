#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
sha256sum --check "$archive/source-files.sha256"
mapfile -t sources < "$archive/source-files.list"
[[ ${#sources[@]} == 18 ]]
base=$(< "$archive/source-base.txt")
[[ $base == 0967725d2e44e6d79df4a9d7cf21cd162df8b534 ]]
git diff --binary "$base" -- "${sources[@]}" | cmp - "$archive/source.patch"
for name in gnu-qualified-build musl-qualified-build gnu-final-runtime musl-qualified-runtime \
    gnu-qualified-roster musl-qualified-roster gnu-qualified-binary musl-qualified-binary \
    clippy-final fmt doctests no-default unsafe-policy source-check \
    remote-create remote-qualified-upload remote-guard-upload \
    native-n5-cold native-n5-bootstrap native-i2-cold native-i2-bootstrap \
    remote-scratch-inventory remote-remove-files remote-remove-directory remote-absence; do
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
done
for exit_file in "$archive"/raw/*.exit; do
    name=${exit_file##*/}; name=${name%.exit}
    expected=0
    case "$name" in clippy) expected=101 ;; gnu-qualified-runtime) expected=127 ;; esac
    [[ $(< "$exit_file") == "$expected" ]]
    [[ -s "$archive/raw/$name.command" && -s "$archive/raw/$name.started" && -s "$archive/raw/$name.finished" ]]
    [[ $(< "$archive/raw/$name.started") < $(< "$archive/raw/$name.finished") ]]
done
cmp "$archive/raw/gnu-qualified-roster.log" "$archive/raw/musl-qualified-roster.log"
rg -q '^907 tests, 0 benchmarks$' "$archive/raw/gnu-qualified-roster.log"
for target in gnu musl; do
    binary=$(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and .target.name == "fe2o3_runtime") | .executable // empty' "$archive/raw/$target-qualified-build.log")
    [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
    binary=${binary#"$root/"}
    [[ $(awk '{print $2}' "$archive/raw/$target-qualified-binary.log") == "$binary" ]]
    sha256sum --check "$archive/raw/$target-qualified-binary.log"
    results=$archive/raw/musl-qualified-runtime.log
    [[ $target == musl ]] || results=$archive/raw/gnu-final-runtime.log
    rg -q '^test result: ok\. 892 passed; 0 failed; 15 ignored; 0 measured; 0 filtered out;' "$results"
    diff -u <(sed -n 's/^\(.*\): test$/\1/p' "$archive/raw/$target-qualified-roster.log") \
        <(sed -n 's/^test \(.*\) \.\.\. \(ok\|ignored.*\)$/\1/p' "$results" | LC_ALL=C sort)
done
expected=46d233f46b5ae8eb6c5d09a98ff1ec8e682a591c703462c46fee0d8e52b7db9e
[[ $(awk '{print $1}' "$archive/raw/musl-qualified-binary.log") == "$expected" ]]
names=(
    generated_native_cold_primary_auxiliary_rebound_abort_and_shutdown
    generated_native_bootstrap_primary_auxiliary_rebound_abort_and_shutdown
    generated_native_cold_issue_complete_readback_retire
    generated_native_bootstrap_issue_complete_readback_retire
)
records=(native-n5-cold native-n5-bootstrap native-i2-cold native-i2-bootstrap)
for index in 0 1 2 3; do
    name=${records[$index]}
    exact=kfd_backend::generated_adoption::tests::native::${names[$index]}
    rg -Fq "$expected $exact" "$archive/raw/$name.command"
    rg -Fq "admitted UID=0xab83d2ffef0d3cdf BDF=0000:26:00.0 exact_test=$exact" "$archive/raw/$name.log"
    rg -Fqx "test $exact ... ok" "$archive/raw/$name.log"
    rg -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 906 filtered out;' "$archive/raw/$name.log"
done
[[ $(< "$archive/raw/native-i2-bootstrap.finished") < $(< "$archive/raw/remote-remove-files.started") ]]
rg -Fq '/tmp/fe2o3-i2-harsh-20260917.mUcACc' "$archive/raw/remote-create.log"
rg -Fq 'test \! -e /tmp/fe2o3-i2-harsh-20260917.mUcACc' "$archive/raw/remote-absence.command"
rg -q '^test result: ok\. 33 passed; 0 failed;' "$archive/raw/doctests.log"
rg -q '^test result: ok\. 5 passed; 0 failed; 1 ignored;' "$archive/raw/unsafe-policy.log"
printf 'PASS: 18 source files; GNU/musl 892 passed + 15 ignored; 4 exact native probes; owned cleanup verified.\n'
