#!/usr/bin/env bash
set -euo pipefail
readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
source "$repo/scripts/qualify-two-gpu-mounts.sh"
source "$repo/scripts/qualify-host-link.sh"
unset FE2O3_GENUINE_TWO_GPU_CASE
campaign=resources
validate_two_gpu_case
for campaign in resources genuine; do
  FE2O3_GENUINE_TWO_GPU_CASE=positive
  if validate_two_gpu_case 2>/dev/null; then exit 1; fi
done
campaign=genuine-two-gpu
for FE2O3_GENUINE_TWO_GPU_CASE in positive second-coverage-reject peer-deadline-before-submit; do
  validate_two_gpu_case
  set_genuine_campaign_command
  case "$FE2O3_GENUINE_TWO_GPU_CASE" in
    positive) expected_test=root_genuine_two_gpu_application_campaign ;;
    second-coverage-reject) expected_test=root_genuine_two_gpu_second_coverage_control ;;
    peer-deadline-before-submit) expected_test=root_genuine_two_gpu_peer_deadline_control ;;
  esac
  [[ ${FE2O3_GENUINE_COMMAND[6]} == "provisioning::tests::genuine_application::$expected_test" ]]
done
for FE2O3_GENUINE_TWO_GPU_CASE in '' POSITIVE peer-timeout 'second-coverage-reject ' $'positive\n'; do
  if validate_two_gpu_case 2>/dev/null; then exit 1; fi
  if set_genuine_campaign_command 2>/dev/null; then exit 1; fi
done
unset FE2O3_GENUINE_TWO_GPU_CASE
set_genuine_campaign_command
[[ ${FE2O3_GENUINE_COMMAND[6]} == provisioning::tests::genuine_application::root_genuine_two_gpu_application_campaign ]]
FE2O3_GENUINE_JQ=${FE2O3_GENUINE_JQ:-$(command -v jq)}
readonly good='{"schema":"fe2o3.genuine-gpu-selection.v1","devices":["0xffffffffffffffff","0x0000000000000001"],"render_minors":[255,128]}'
selection=$(printf 'libtest preamble\nFE2O3_GPU_SELECTION_V1=%s\nlibtest epilogue\n' "$good" | read_two_gpu_selection)
[[ "$("$FE2O3_GENUINE_JQ" -cS . <<< "$selection")" == "$("$FE2O3_GENUINE_JQ" -cS . <<< "$good")" ]]
for records in '' 'FE2O3_GPU_SELECTION_V1=malformed' \
  "FE2O3_GPU_SELECTION_V1=$good"$'\n'"FE2O3_GPU_SELECTION_V1=$good"; do
  if printf '%s\n' "$records" | read_two_gpu_selection >/dev/null 2>&1; then
    printf 'accepted invalid selection transcript\n' >&2
    exit 1
  fi
done
FE2O3_GENUINE_GPU_SELECTION=$good
configure_two_gpu_mounts
expected=(--dev-bind /dev/kfd /dev/kfd --dir /dev/dri \
  --dev-bind /dev/dri/renderD255 /dev/dri/renderD255 \
  --dev-bind /dev/dri/renderD128 /dev/dri/renderD128)
[[ ${#FE2O3_GPU_MOUNTS[@]} == ${#expected[@]} ]]
for index in "${!expected[@]}"; do
  [[ ${FE2O3_GPU_MOUNTS[$index]} == "${expected[$index]}" ]]
done
for mutation in \
  '.schema="wrong"' '.extra=true' '.devices=[]' \
  '.devices[0]="0x0000000000000000"' '.devices[1]=.devices[0]' \
  '.devices[0]=18446744073709551615' '.devices[0]="0x1;id"' \
  '.devices[0]="0x0000000000000002\n"' \
  '.render_minors=[128,128]' '.render_minors=[127,129]' \
  '.render_minors=[128,256]' '.render_minors=[128,129.5]' \
  '.render_minors=["128",129]' '.render_minors=[128]' \
  '.render_minors=[128,129,130]' '.render_minors=[128,"129;id"]' \
  '.render_minors=[128,"/dev/dri/renderD129"]'; do
  FE2O3_GENUINE_GPU_SELECTION=$("$FE2O3_GENUINE_JQ" -c "$mutation" <<< "$good")
  if configure_two_gpu_mounts 2>/dev/null; then
    printf 'accepted invalid selection: %s\n' "$mutation" >&2
    exit 1
  fi
  [[ ${#FE2O3_GPU_MOUNTS[@]} == 0 ]]
done
FE2O3_GENUINE_GPU_SELECTION='malformed'
if configure_two_gpu_mounts 2>/dev/null; then exit 1; fi
printf 'two-GPU fixed mount selection checks passed\n'
