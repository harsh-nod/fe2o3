#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
runner="$script_dir/run.sh"

cpu_output=$($runner cpu-source-model)
[[ $cpu_output == *"CPU_SOURCE_MODEL bidirectional device FFI oracle"* ]]

source_output=$($runner source-check)
[[ $source_output == *"SOURCE_CHECK"* ]]
[[ $source_output == *"no GPU compilation, link, load, or execution occurred"* ]]

set +e
llvm_output=$($runner llvm-verify 2>&1)
llvm_status=$?
set -e
if [[ $llvm_status -eq 0 ]]; then
  [[ $llvm_output == *"LLVM_VERIFIED"* ]]
elif [[ $llvm_status -eq 77 ]]; then
  [[ $llvm_output == "UNAVAILABLE llvm-verify:"* ]]
else
  printf '%s\n' "$llvm_output" >&2
  exit "$llvm_status"
fi

set +e
unavailable_llvm_output=$(FE2O3_LLVM_AS=/fe2o3-missing/llvm-as \
  FE2O3_OPT=/fe2o3-missing/opt "$runner" llvm-verify 2>&1)
unavailable_llvm_status=$?
set -e
[[ $unavailable_llvm_status -eq 77 ]]
[[ $unavailable_llvm_output == "UNAVAILABLE llvm-verify:"* ]]

set +e
hardware_output=$($runner hardware 2>&1)
hardware_status=$?
set -e
[[ $hardware_status -eq 77 ]]
[[ $hardware_output == "UNAVAILABLE hardware:"* ]]

set +e
aggregate_output=$($runner all 2>&1)
aggregate_status=$?
set -e
[[ $aggregate_status -eq 77 ]]
[[ $aggregate_output == *"CPU_SOURCE_MODEL bidirectional device FFI oracle"* ]]
[[ $aggregate_output == *"SOURCE_CHECK"* ]]
[[ $aggregate_output == *"UNAVAILABLE hardware:"* ]]

set +e
unavailable_aggregate_output=$(FE2O3_LLVM_AS=/fe2o3-missing/llvm-as \
  FE2O3_OPT=/fe2o3-missing/opt "$runner" all 2>&1)
unavailable_aggregate_status=$?
set -e
[[ $unavailable_aggregate_status -eq 77 ]]
[[ $unavailable_aggregate_output == *"UNAVAILABLE llvm-verify:"* ]]
[[ $unavailable_aggregate_output == *"UNAVAILABLE hardware:"* ]]

set +e
removed_mode_output=$($runner compile 2>&1)
removed_mode_status=$?
set -e
[[ $removed_mode_status -eq 64 ]]
[[ $removed_mode_output == usage:* ]]

set +e
missing_toolchain_output=$(FE2O3_RUSTUP=/fe2o3-missing/rustup \
  "$runner" source-check 2>&1)
missing_toolchain_status=$?
set -e
[[ $missing_toolchain_status -eq 69 ]]
[[ $missing_toolchain_output == "ERROR source-check: rustup executable not found:"* ]]

printf '%s\n' 'device-link source-evidence classification tests passed'
