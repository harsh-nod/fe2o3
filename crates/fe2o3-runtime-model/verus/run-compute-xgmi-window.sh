#!/bin/sh
set -eu

: "${VERUS:?Set VERUS to the pinned Verus executable}"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
temp_parent=$(CDPATH= cd -- "${RUNNER_TEMP:-/tmp}" && pwd)
base=$(mktemp -d "$temp_parent/fe2o3-compute-xgmi-window.XXXXXXXX")
cleanup() {
    rm -rf -- "$base"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

python3 -I -B "$here/compute_xgmi_window_check.py" \
    --verus "$VERUS" --output "$base/proof"
printf '%s\n' 'PASS: 3 logical-window arithmetic functions, 11 rejected logical mutants, 7 runner controls; no adapter or DMA refinement claim.'
