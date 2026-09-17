#!/usr/bin/env bash
set -euo pipefail
expected=${1:?binary SHA256}
test_name=${2:?exact native test}
case "$test_name" in
    kfd_backend::generated_adoption::tests::native::generated_native_cold_primary_auxiliary_rebound_abort_and_shutdown|\
    kfd_backend::generated_adoption::tests::native::generated_native_bootstrap_primary_auxiliary_rebound_abort_and_shutdown|\
    kfd_backend::generated_adoption::tests::native::generated_native_cold_issue_complete_readback_retire|\
    kfd_backend::generated_adoption::tests::native::generated_native_bootstrap_issue_complete_readback_retire) ;;
    *) exit 64 ;;
esac
scratch=$(cd -- "$(dirname -- "$0")" && pwd)
[[ $scratch == /tmp/fe2o3-i2-harsh-20260917.* && -O $scratch && ! -L $scratch ]]
printf '%s  %s\n' "$expected" "$scratch/runtime" | sha256sum --check
date -u +%FT%T.%NZ
df -B1 /tmp
status=$(timeout 20s rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --json)
printf '%s\n' "$status"
jq -e '.card1 | .["Unique ID"] == "0xab83d2ffef0d3cdf" and .["PCI Bus"] == "0000:26:00.0" and (.["GPU use (%)"] | tonumber) == 0 and (.["VRAM Total Used Memory (B)"] | tonumber) < 536870912' <<< "$status" > /dev/null
pids=$(timeout 20s rocm-smi --showpidgpus)
printf '%s\n' "$pids"
# rocm-smi emits no JSON for --showpidgpus. Accept only its explicit PID
# header/device-list grammar; unknown text or any target-GPU process fails closed.
awk '
    /^PID [0-9]+ is using [0-9]+ DRM device\(s\)/ {
        count=$5; seen++;
        if (count > 0) {
            if (getline <= 0 || NF != count) { bad=1; exit }
            for (i=1; i<=NF; i++) if ($i !~ /^[0-9]+$/ || $i == 1) { bad=1; exit }
        }
        next
    }
    /^=|^[[:space:]]*$/ { next }
    { bad=1; exit }
    END { if (bad || seen == 0) exit 1 }
' <<< "$pids"
printf 'admitted UID=0xab83d2ffef0d3cdf BDF=0000:26:00.0 exact_test=%s\n' "$test_name"
timeout --signal=TERM 45s env FE2O3_TEST_NATIVE_ISOLATED=1 \
    FE2O3_TEST_NATIVE_UNIQUE_ID=0xab83d2ffef0d3cdf \
    prlimit --core=0:0 -- "$scratch/runtime" --ignored --exact "$test_name" --nocapture --test-threads=1
date -u +%FT%T.%NZ
timeout 20s rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --json
