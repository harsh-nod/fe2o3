#!/usr/bin/env bash
set -euo pipefail
readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
source "$repo/scripts/qualify-two-gpu-mounts.sh"
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
