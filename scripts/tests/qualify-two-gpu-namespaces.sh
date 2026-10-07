#!/usr/bin/env bash
# Device-node carriage only. Null/zero aliases are never passed to GPU admission.
set -euo pipefail
export PATH=/usr/bin:/bin
export FE2O3_GENUINE_JQ=/usr/bin/jq
[[ -x $FE2O3_GENUINE_JQ ]] || { printf 'namespace test requires /usr/bin/jq\n' >&2; exit 1; }
[[ $EUID == 0 ]] || { printf 'namespace test requires real root\n' >&2; exit 1; }
readonly repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
source "$repo/scripts/qualify-two-gpu-mounts.sh"
export FE2O3_GENUINE_GPU_SELECTION='{"schema":"fe2o3.genuine-gpu-selection.v1","devices":["0x0000000000000001","0x0000000000000002"],"render_minors":[128,129]}'
configure_two_gpu_mounts
case "${1:-}" in
  '')
    # All aliases exist only in this initial private namespace. The following
    # four layers use the exact campaign mount-array builder unchanged.
    exec timeout --kill-after=2s 15s bwrap --die-with-parent --unshare-pid \
      --ro-bind / / --proc /proc --dev /dev --dir /dev/dri \
      --dev-bind /dev/null /dev/kfd \
      --dev-bind /dev/null /dev/dri/renderD128 \
      --dev-bind /dev/zero /dev/dri/renderD129 \
      /bin/bash "$0" layer 4
    ;;
  layer)
    [[ ${2:-} =~ ^[0-4]$ ]]
    if [[ $2 != 0 ]]; then
      exec bwrap --die-with-parent --ro-bind / / --dev /dev \
        "${FE2O3_GPU_MOUNTS[@]}" /bin/bash "$0" layer "$(( $2 - 1 ))"
    fi
    [[ -c /dev/kfd && -c /dev/dri/renderD128 && -c /dev/dri/renderD129 ]]
    [[ $(stat -c '%t:%T' /dev/kfd) == 1:3 ]]
    [[ $(stat -c '%t:%T' /dev/dri/renderD128) == 1:3 ]]
    [[ $(stat -c '%t:%T' /dev/dri/renderD129) == 1:5 ]]
    nodes=(/dev/dri/*)
    [[ ${#nodes[@]} == 2 ]]
    [[ ${nodes[0]} == /dev/dri/renderD128 && ${nodes[1]} == /dev/dri/renderD129 ]]
    printf 'four-layer synthetic character-node carriage passed; no GPU admission or execution\n'
    ;;
  *) exit 2 ;;
esac
