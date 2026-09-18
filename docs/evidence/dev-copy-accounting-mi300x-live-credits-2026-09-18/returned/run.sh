#!/usr/bin/env bash
set -uo pipefail
owned=/tmp/fe2o3-copy-accounting-live-credits-20260918.UYReiiVM
marker=fe2o3-copy-accounting-f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe
[[ -d "$owned" && ! -L "$owned" && $(cat "$owned/owner") == "$marker" ]] || exit 90
cd "$owned" || exit 91
mkdir results || exit 92
unset HIP_VISIBLE_DEVICES ROCR_VISIBLE_DEVICES CUDA_VISIBLE_DEVICES GPU_DEVICE_ORDINAL
record() {
    local name=$1 status
    shift
    [[ ! -e results/$name.command ]] || return 93
    printf '%q ' "$@" > "results/$name.command"
    printf '\n' >> "results/$name.command"
    date -u +%FT%T.%NZ > "results/$name.started"
    "$@" > "results/$name.stdout" 2> "results/$name.stderr"
    status=$?
    date -u +%FT%T.%NZ > "results/$name.finished"
    printf '%s\n' "$status" > "results/$name.exit"
    printf '%s exit=%s\n' "$name" "$status"
    return "$status"
}
observe=(python3 -B "$owned/copy-host-observe.py" --gpu-index 4 --pci-bdf 0000:85:00.0 --unique-id 0x54f88318ca05093d --samples 1)
record inspect-before bash "$owned/inspect.sh" || exit $?
if ! record preflight "${observe[@]}"; then
    printf 'native_launched=false reason=preflight-refused no_retry=true\n' > results/outcome.txt
    record inspect-after bash "$owned/inspect.sh"
    exit 10
fi
record native env -u HIP_VISIBLE_DEVICES -u ROCR_VISIBLE_DEVICES -u CUDA_VISIBLE_DEVICES -u GPU_DEVICE_ORDINAL FE2O3_TEST_NATIVE_ACCOUNTING=1 FE2O3_TEST_NATIVE_UNIQUE_ID=0x54f88318ca05093d prlimit --core=0:0 -- timeout --signal=TERM --kill-after=5s 180s numactl --physcpubind=48-95 --membind=1 "$owned/runtime-test" kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing --ignored --exact --nocapture --test-threads=1
native_status=$?
record post-immediate "${observe[@]}"
immediate_status=$?
record settle-delay sleep 20
delay_status=$?
record post-delayed "${observe[@]}"
delayed_status=$?
record inspect-after bash "$owned/inspect.sh"
inspect_status=$?
printf 'native_launched=true native_exit=%s immediate_exit=%s delay_exit=%s delayed_exit=%s inspect_exit=%s no_retry=true\n' "$native_status" "$immediate_status" "$delay_status" "$delayed_status" "$inspect_status" > results/outcome.txt
[[ $native_status == 0 && $immediate_status == 0 && $delay_status == 0 && $delayed_status == 0 && $inspect_status == 0 ]]
