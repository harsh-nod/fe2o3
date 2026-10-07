#!/usr/bin/env bash
set -euo pipefail
readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
source "$repo/scripts/qualify-device-roster-mounts.sh"
source "$repo/scripts/qualify-host-link.sh"
unset FE2O3_GENUINE_GPU_UID0 FE2O3_GENUINE_GPU_UID1 FE2O3_GENUINE_TWO_GPU_CASE FE2O3_GENUINE_GPU_ROSTER
campaign=genuine
validate_device_roster_case
campaign=genuine-device-roster
if validate_device_roster_case 2>/dev/null; then exit 1; fi
FE2O3_GENUINE_GPU_ROSTER='["0x1","0x2"]'
validate_device_roster_case
set_genuine_campaign_command
[[ ${FE2O3_GENUINE_COMMAND[6]} == provisioning::tests::genuine_application::root_genuine_device_roster_application_campaign ]]
for name in FE2O3_GENUINE_GPU_UID0 FE2O3_GENUINE_GPU_UID1 FE2O3_GENUINE_TWO_GPU_CASE; do
  printf -v "$name" '%s' 'unexpected'
  if validate_device_roster_case 2>/dev/null; then exit 1; fi
  unset "$name"
done
for campaign in resources genuine genuine-two-gpu; do
  if validate_device_roster_case 2>/dev/null; then exit 1; fi
done
campaign=genuine-device-roster
FE2O3_GENUINE_JQ=${FE2O3_GENUINE_JQ:-$(command -v jq)}
readonly good='{"schema":"fe2o3.genuine-gpu-roster-selection.v1","devices":["0x0000000000000002","0x0000000000000001"],"render_minors":[129,128],"observed_host_devices":["0x0000000000000001","0x0000000000000002","0x0000000000000003"],"excluded_unselected_devices":["0x0000000000000003"],"occupancy":"not-measured","scope":"selected-admitted-context-only"}'
FE2O3_GENUINE_GPU_ROSTER_SELECTION=$(printf 'preamble\nFE2O3_GPU_ROSTER_SELECTION_V1=%s\n' "$good" | read_device_roster_selection)
configure_device_roster_mounts
expected=(--dev-bind /dev/kfd /dev/kfd --dir /dev/dri \
  --dev-bind /dev/dri/renderD129 /dev/dri/renderD129 \
  --dev-bind /dev/dri/renderD128 /dev/dri/renderD128)
[[ ${#FE2O3_GPU_MOUNTS[@]} == ${#expected[@]} ]]
for index in "${!expected[@]}"; do [[ ${FE2O3_GPU_MOUNTS[$index]} == "${expected[$index]}" ]]; done
for mutation in \
  '.schema="fe2o3.genuine-gpu-selection.v1"' '.extra=true' '.devices=[]' \
  '.devices[0]="0x0000000000000000"' '.devices[1]=.devices[0]' \
  '.devices[0]="0x1;id"' '.devices[0]=18446744073709551615' \
  '.devices[0]="0x0000000000000002\n"' '.render_minors=[128,128]' \
  '.render_minors=[127,129]' '.render_minors=[128,256]' '.render_minors=[128,129.5]' \
  '.render_minors=["128",129]' '.render_minors=[128]' '.render_minors=[128,"/dev/dri/renderD129"]' \
  '.observed_host_devices=[]' '.observed_host_devices|=reverse' \
  '.observed_host_devices += [.observed_host_devices[0]]' '.excluded_unselected_devices=[]' \
  '.occupancy="idle"' '.scope="all-host-qualified"'; do
  FE2O3_GENUINE_GPU_ROSTER_SELECTION=$("$FE2O3_GENUINE_JQ" -c "$mutation" <<< "$good")
  if configure_device_roster_mounts 2>/dev/null; then printf 'accepted invalid roster: %s\n' "$mutation" >&2; exit 1; fi
  [[ ${#FE2O3_GPU_MOUNTS[@]} == 0 ]]
done
for count in 1 2 8 9; do
  FE2O3_GENUINE_GPU_ROSTER_SELECTION=$("$FE2O3_GENUINE_JQ" -cn --argjson n "$count" '
    [range(1;$n+1) | "0x000000000000000\(.)"] as $ids |
    {schema:"fe2o3.genuine-gpu-roster-selection.v1", devices:$ids,
      render_minors:[range(128;128+$n)], observed_host_devices:$ids,
      excluded_unselected_devices:[], occupancy:"not-measured",scope:"selected-admitted-context-only"}')
  if [[ $count == 2 || $count == 8 ]]; then
    configure_device_roster_mounts
    [[ ${#FE2O3_GPU_MOUNTS[@]} == $((5 + 3 * count)) ]]
  elif configure_device_roster_mounts 2>/dev/null; then exit 1; fi
done
for record in '' 'FE2O3_GPU_ROSTER_SELECTION_V1=bad' \
  "FE2O3_GPU_ROSTER_SELECTION_V1=$good"$'\n'"FE2O3_GPU_ROSTER_SELECTION_V1=$good"; do
  if printf '%s\n' "$record" | read_device_roster_selection >/dev/null 2>&1; then exit 1; fi
done
printf 'bounded device-roster mount selection and campaign isolation checks passed\n'
