#!/usr/bin/env bash
set -euo pipefail

archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
out="$archive/native"
mkdir -p -- "$out"
cd -- "$root"
expected_source=967a62dfffb94b4ca679ba3d4ae5ed60fc82eba1
[[ $(git rev-parse HEAD) == "$expected_source" ]]
git diff --exit-code -- crates
git diff --cached --exit-code -- crates
binary="$root/target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-1e67dbc4ddf4b5aa"
receipt="$root/docs/evidence/dev-r126-auxiliary-release-2026-09-16/final/musl-runtime-binary.sha256"
uid=0xab83d2ffef0d3cdf

run() {
    local name=$1 status
    shift
    [[ ! -e "$out/$name.command" ]]
    printf '%q ' "$@" > "$out/$name.command"
    printf '\n' >> "$out/$name.command"
    date -u +%FT%T.%NZ > "$out/$name.started"
    if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%T.%NZ > "$out/$name.finished"
    printf '%s\n' "$status" > "$out/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}

availability() {
    local name=$1
    run "$name-use" ssh mi300x /opt/rocm/bin/rocm-smi --showuse --showmeminfo vram --json
    run "$name-identity" ssh mi300x /opt/rocm/bin/rocm-smi --showuniqueid --showbus --json
    run "$name-pids" ssh mi300x /opt/rocm/bin/rocm-smi --showpidgpus
    run "$name-admit" python3 "$archive/check-availability.py" \
        "$out/$name-use.log" "$out/$name-identity.log" "$out/$name-pids.log"
}

run source git rev-parse HEAD
run binary-before sha256sum --check "$receipt"
availability initial
run create-scratch ssh mi300x mktemp -d /tmp/fe2o3-r126-native-967a62df.XXXXXXXX
remote=$(< "$out/create-scratch.log")
[[ $remote =~ ^/tmp/fe2o3-r126-native-967a62df\.[[:alnum:]]{8}$ ]]
run upload scp "$binary" "mi300x:$remote/runtime-tests"
run remote-hash-before ssh mi300x sha256sum "$remote/runtime-tests"
[[ $(awk '{print $1}' "$out/remote-hash-before.log") == $(awk '{print $1}' "$receipt") ]]
run remote-tools ssh mi300x bash -c "'command -v timeout && command -v prlimit && command -v fuser'"

names=(
    cold_allocation::native_runtime_cold_device_capacity_refunds_context_and_retries
    cold_allocation::native_runtime_cold_host_capacity_refunds_context_and_retries
    native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection
    native_runtime_allocates_while_primary_compute_is_pending
    native_runtime_allocates_while_primary_and_auxiliary_compute_are_pending
    native_runtime_allocation_shutdown_selects_retained_directional_release
    native_runtime_device_promotion_roundtrip_and_retained_shutdown
    native_runtime_zero_capacity_recycle_disposes_before_trim
    native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary
    native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds
    native_runtime_auxiliary_budget_failure_retains_initialized_prefix
)
for index in "${!names[@]}"; do
    printf -v name 'probe-%02d' "$index"
    availability "$name-before"
    run "$name" ssh mi300x env FE2O3_TEST_NATIVE_UNIQUE_ID="$uid" \
        FE2O3_TEST_NATIVE_ISOLATED=1 FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=2 \
        prlimit --core=0:0 -- timeout --signal=TERM --kill-after=10s 180s \
        "$remote/runtime-tests" "kfd_backend::retained_release_tests::${names[index]}" \
        --exact --ignored --nocapture --test-threads=1
    grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored;' "$out/$name.log"
done
for prefix in 0 1; do
    name="prefix-$prefix"
    availability "$name-before"
    run "$name" ssh mi300x env FE2O3_TEST_NATIVE_UNIQUE_ID="$uid" \
        FE2O3_TEST_NATIVE_ISOLATED=1 FE2O3_TEST_NATIVE_INITIALIZED_PREFIX="$prefix" \
        prlimit --core=0:0 -- timeout --signal=TERM --kill-after=10s 180s \
        "$remote/runtime-tests" \
        kfd_backend::retained_release_tests::native_runtime_auxiliary_budget_failure_retains_initialized_prefix \
        --exact --ignored --nocapture --test-threads=1
    grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored;' "$out/$name.log"
done
availability final
run remote-hash-after ssh mi300x sha256sum "$remote/runtime-tests"
cmp "$out/remote-hash-before.log" "$out/remote-hash-after.log"
run binary-after sha256sum --check "$receipt"
run source-after git rev-parse HEAD
cmp "$out/source.log" "$out/source-after.log"
git diff --exit-code -- crates
git diff --cached --exit-code -- crates
printf 'Native probes complete. Inspect process closure before removing only %s/runtime-tests and its directory.\n' "$remote"
