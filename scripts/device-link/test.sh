#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
runner="$script_dir/run.sh"

cpu_output=$($runner cpu)
[[ $cpu_output == *"CPU_ONLY bidirectional device FFI oracle"* ]]

compile_output=$($runner compile)
[[ $compile_output == *"COMPILE_ONLY"* ]]

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
[[ $aggregate_output == *"CPU_ONLY bidirectional device FFI oracle"* ]]
[[ $aggregate_output == *"COMPILE_ONLY"* ]]
[[ $aggregate_output == *"UNAVAILABLE hardware:"* ]]

printf '%s\n' 'device-link evidence classification tests passed'
