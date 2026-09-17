#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
base=$(< "$archive/corrected-source-base.txt")
[[ $base == 13c5e5b8126dd461c5d7eb8972613d3ff6f4019b ]]
sha256sum --check "$archive/corrected-source-files.sha256"
git diff --quiet -- crates
[[ $(wc -l < "$archive/corrected-source-files.list") == 13 ]]
git diff --binary "$base" -- crates | cmp - "$archive/corrected-source.patch"
diff -u "$archive/corrected-source-files.list" <(git diff --name-only "$base" -- crates)

success() {
    local name=$1
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
    [[ -s "$archive/raw/$name.command" && -s "$archive/raw/$name.started" && -s "$archive/raw/$name.finished" ]]
}
roster() { sed -n 's/^\(.*\): test$/\1/p' "$archive/raw/$1.log"; }
command_is() {
    local name=$1
    shift
    diff -u "$archive/raw/$name.command" <(printf '%q ' "$@"; printf '\n')
}
results() {
    local record=$1 list=$2 count=$3 passes=$4 ignores=$5 filtered=$6 progress=${7:-0} parsed
    success "$record"
    success "$list"
    rg -qx "$count tests, 0 benchmarks" "$archive/raw/$list.log"
    parsed=$(awk -v count="$count" -v passes="$passes" -v ignores="$ignores" \
        -v filtered="$filtered" -v allow_progress="$progress" \
        -f "$archive/parse-results.awk" "$archive/raw/$record.log")
    diff -u <(roster "$list" | LC_ALL=C sort) <(printf '%s\n' "$parsed" | LC_ALL=C sort)
}

# These failed initial-source runs remain history, never corrected qualification.
for target in gnu musl; do
    [[ $(< "$archive/raw/$target.exit") == 101 ]]
    rg -q '^test result: FAILED\. 1401 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out;' "$archive/raw/$target.log"
    rg -qx '    queue::dispatch_binding::tests::manifest_digest_is_frozen' "$archive/raw/$target.log"
    rg -qx '    queue::live::tests::session_manifest_digest_is_frozen' "$archive/raw/$target.log"
done
for exit_file in "$archive"/raw/*.exit; do
    name=${exit_file##*/}; name=${name%.exit}
    [[ -s "$archive/raw/$name.command" && -s "$archive/raw/$name.started" && -s "$archive/raw/$name.finished" ]]
    [[ $(< "$archive/raw/$name.started") < $(< "$archive/raw/$name.finished") ]]
    case "$name" in gnu|musl) ;; *) success "$name" ;; esac
done
for command in "$archive"/raw/*.command; do
    [[ -s ${command%.command}.exit && -s ${command%.command}.finished ]]
done

for target in gnu musl; do
    prefix=corrected-$target
    success "$prefix-build"
    success "$prefix-source-before"
    success "$prefix-source-after"
    command_is "$prefix-source-before" sha256sum --check "$archive/corrected-source-files.sha256"
    command_is "$prefix-source-after" sha256sum --check "$archive/corrected-source-files.sha256"
    target_args=()
    [[ $target == gnu ]] || target_args=(--target x86_64-unknown-linux-musl)
    command_is "$prefix-build" env CARGO_INCREMENTAL=0 cargo test --locked --offline \
        -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-service-host --all-features --lib \
        --no-run "${target_args[@]}" --message-format=json
    for crate in kfd runtime service_host; do
        binary=$(jq -Rr --arg name "fe2o3_$crate" '
            fromjson? | select(.reason == "compiler-artifact" and
            .target.name == $name and .profile.test == true) | .executable // empty
        ' "$archive/raw/$prefix-build.log")
        [[ -n $binary && $binary != *$'\n'* && -x $binary ]]
        success "$prefix-$crate-binary"
        success "$prefix-$crate-binary-after"
        command_is "$prefix-$crate-binary" sha256sum "$binary"
        command_is "$prefix-$crate-binary-after" sha256sum --check "$archive/raw/$prefix-$crate-binary.log"
        [[ $(awk '{print $2}' "$archive/raw/$prefix-$crate-binary.log") == "$binary" ]]
        sha256sum --check "$archive/raw/$prefix-$crate-binary.log"
        success "$prefix-$crate-roster"
        command_is "$prefix-$crate-roster" "$binary" --list
        if [[ $crate == kfd ]]; then
            command_is "$prefix-kfd-main-roster" "$binary" --list --skip queue_linux::tests::
            command_is "$prefix-kfd-abort-roster" "$binary" --list queue_linux::tests::
            command_is "$prefix-kfd-main" "$binary" --test-threads=4 --skip queue_linux::tests::
            command_is "$prefix-kfd-abort" "$binary" --test-threads=1 queue_linux::tests::
        else
            command_is "$prefix-$crate" "$binary" --test-threads=1
        fi
        cmp "$archive/raw/corrected-gnu-$crate-roster.log" "$archive/raw/corrected-musl-$crate-roster.log"
    done
    rg -qx '1403 tests, 0 benchmarks' "$archive/raw/$prefix-kfd-roster.log"
    results "$prefix-kfd-main" "$prefix-kfd-main-roster" 1383 1383 0 20 1
    results "$prefix-kfd-abort" "$prefix-kfd-abort-roster" 20 20 0 1383
    ! roster "$prefix-kfd-main-roster" | rg '^queue_linux::tests::'
    ! roster "$prefix-kfd-abort-roster" | rg -v '^queue_linux::tests::'
    diff -u <(roster "$prefix-kfd-roster" | LC_ALL=C sort) \
        <({ roster "$prefix-kfd-main-roster"; roster "$prefix-kfd-abort-roster"; } | LC_ALL=C sort)
    results "$prefix-runtime" "$prefix-runtime-roster" 913 896 17 0
    results "$prefix-service_host" "$prefix-service_host-roster" 45 45 0 0
done
for gate in clippy fmt no-default unsafe-policy doctests parser source-check; do
    success "corrected-$gate"
done
command_is corrected-clippy env CARGO_INCREMENTAL=0 cargo clippy --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-service-host --all-features --all-targets -- -D warnings
command_is corrected-fmt cargo fmt --all -- --check
command_is corrected-no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline \
    -p fe2o3-runtime --no-default-features
command_is corrected-unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p cargo-fe2o3 --test unsafe_source_policy
command_is corrected-doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-service-host --all-features --doc
command_is corrected-parser bash "$archive/check-parser.sh"
command_is corrected-source-check sha256sum --check "$archive/corrected-source-files.sha256"
rg -q '^test result: ok\. 27 passed; 0 failed;' "$archive/raw/corrected-doctests.log"
rg -q '^test result: ok\. 33 passed; 0 failed;' "$archive/raw/corrected-doctests.log"
rg -q '^test result: ok\. 22 passed; 0 failed;' "$archive/raw/corrected-doctests.log"
rg -q '^test result: ok\. 5 passed; 0 failed; 1 ignored;' "$archive/raw/corrected-unsafe-policy.log"

expected=$(awk '{print $1}' "$archive/raw/corrected-musl-runtime-binary.log")
[[ $expected =~ ^[0-9a-f]{64}$ ]]
binary=$(jq -Rr 'fromjson? | select(.reason == "compiler-artifact" and
    .target.name == "fe2o3_runtime" and .profile.test == true) | .executable // empty' \
    "$archive/raw/corrected-musl-build.log")
audit_native() {
local prefix=$1
scratch=$(< "$archive/raw/$prefix-remote-create.log")
[[ $scratch =~ ^/tmp/fe2o3-c4-harsh-20260917\.[a-zA-Z0-9]+$ ]]
ssh_args=(-o BatchMode=yes -o ConnectTimeout=10)
command_is "$prefix-native-binary" sha256sum "$binary"
cmp "$archive/raw/$prefix-native-binary.log" "$archive/raw/corrected-musl-runtime-binary.log"
command_is "$prefix-native-binary-after" sha256sum --check "$archive/raw/$prefix-native-binary.log"
for endpoint in before after; do
    command_is "$prefix-native-source-$endpoint" sha256sum --check "$archive/corrected-source-files.sha256"
done
command_is "$prefix-remote-create" ssh "${ssh_args[@]}" mi300x mktemp -d /tmp/fe2o3-c4-harsh-20260917.XXXXXX
command_is "$prefix-remote-binary-upload" scp "${ssh_args[@]}" "$binary" "mi300x:$scratch/runtime"
command_is "$prefix-remote-guard-upload" scp "${ssh_args[@]}" "$archive/native-guard-corrected.sh" "mi300x:$scratch/native-guard.sh"
command_is "$prefix-remote-bytes" ssh "${ssh_args[@]}" mi300x sha256sum "$scratch/runtime" "$scratch/native-guard.sh"
guard_hash=$(sha256sum "$archive/native-guard-corrected.sh" | cut -d ' ' -f 1)
diff -u <(printf '%s  %s\n' "$expected" "$scratch/runtime" "$guard_hash" "$scratch/native-guard.sh") \
    "$archive/raw/$prefix-remote-bytes.log"
command_is "$prefix-remote-inventory-final" ssh "${ssh_args[@]}" mi300x ls -la -- "$scratch"
command_is "$prefix-remote-remove-files" ssh "${ssh_args[@]}" mi300x rm -f -- "$scratch/runtime" "$scratch/native-guard.sh"
command_is "$prefix-remote-remove-directory" ssh "${ssh_args[@]}" mi300x rmdir -- "$scratch"
command_is "$prefix-remote-absence" ssh "${ssh_args[@]}" mi300x test ! -e "$scratch"
records=(native-c4-cold native-c4-bootstrap native-i2-cold native-i2-bootstrap)
names=(generated_native_cold_full_roster_readback generated_native_bootstrap_full_roster_readback generated_native_cold_issue_complete_readback_retire generated_native_bootstrap_issue_complete_readback_retire)
for index in 0 1 2 3; do
    name=$prefix-${records[$index]}
    exact=kfd_backend::generated_adoption::tests::native::${names[$index]}
    success "$name"
    command_is "$name" ssh "${ssh_args[@]}" mi300x bash "$scratch/native-guard.sh" "$expected" "$exact"
    [[ $(rg -Fxc "$scratch/runtime: OK" "$archive/raw/$name.log") == 1 ]]
    [[ $(rg -Fxc "admitted UID=0xab83d2ffef0d3cdf BDF=0000:26:00.0 exact_test=$exact" "$archive/raw/$name.log") == 1 ]]
    bootstrap=false; full_roster=true; disposals=12
    (( index % 2 == 0 )) || bootstrap=true
    if (( index >= 2 )); then full_roster=false; disposals=9; fi
    fixture="generated native fixture: bootstrap=$bootstrap, full_roster=$full_roster, primary/AUX/rebound, $disposals DATA disposals, shutdown complete; no protected Worker/carrier or typed reply claim"
    awk -v exact="$exact" -v fixture="$fixture" '
        function fail() { bad=1; exit 1 }
        /^running [0-9]+ tests?$/ {
            if ($0 != "running 1 test" || state != 0) fail()
            state=1; next
        }
        /^test result:/ {
            if (state != 2 || $0 !~ /^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 912 filtered out; finished in [0-9]+(\.[0-9]+)?s$/) fail()
            state=3; next
        }
        /^test / {
            prefix="test " exact " ... "
            if (state != 1 || test_seen++ || index($0, prefix) != 1) fail()
            suffix=substr($0, length(prefix)+1)
            if (suffix == "ok") state=2
            else if (suffix == fixture) fixtures++
            else if (suffix != "") fail()
            next
        }
        $0 == "ok" { if (state != 1 || test_seen != 1) fail(); state=2; next }
        $0 == fixture { fixtures++; next }
        END { if (bad || state != 3 || test_seen != 1 || fixtures != 1) exit 1 }
    ' "$archive/raw/$name.log"
    expected_failure=false
    [[ $prefix != corrected || $index != 2 ]] || expected_failure=true
    jq -Rsc --argjson expected_failure "$expected_failure" '[split("\n")[] | fromjson? | objects | select(has("card1")) | .card1] |
        length == 2 and all(.[]; .["Unique ID"] == "0xab83d2ffef0d3cdf" and .["PCI Bus"] == "0000:26:00.0") and
        (.[0]["GPU use (%)"] | tonumber) == 0 and
        (.[0]["VRAM Total Used Memory (B)"] | tonumber) < 536870912 and
        (if $expected_failure then
            [.[0]["VRAM Total Used Memory (B)"], .[1]["VRAM Total Used Memory (B)"]] == ["298647552", "918499328"]
        else .[0]["VRAM Total Used Memory (B)"] == .[1]["VRAM Total Used Memory (B)"] end)' \
        "$archive/raw/$name.log" | rg -qx true
done
for name in "$prefix-native-c4-cold" "$prefix-native-c4-bootstrap"; do
    rg -Fq 'full_roster=true, primary/AUX/rebound, 12 DATA disposals' "$archive/raw/$name.log"
done
ordered=(native-source-before native-binary remote-create remote-binary-upload remote-guard-upload remote-bytes remote-inventory-final
    native-c4-cold native-c4-bootstrap native-i2-cold native-i2-bootstrap native-binary-after native-source-after
    remote-remove-files remote-remove-directory remote-absence)
previous=
for name in "${ordered[@]}"; do
    success "$prefix-$name"
    [[ -z $previous || $(< "$archive/raw/$prefix-$previous.finished") < $(< "$archive/raw/$prefix-$name.started") ]]
    previous=$name
done
}
# The first corrected batch is explicitly unsuccessful accounting history.
# Only the separate rerun must meet exact equality for every native probe.
audit_native corrected
audit_native corrected-rerun
printf 'PASS: corrected 13-file source; GNU/musl each 1403 KFD + 896 runtime + 45 service-host passes; 17 native ignores; four separately qualified native reruns; owned cleanup. Initial manifest failures and first-batch native accounting failure remain historical.\n'
