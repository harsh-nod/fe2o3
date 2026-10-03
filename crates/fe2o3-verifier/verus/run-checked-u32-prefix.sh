#!/bin/sh
set -eu

: "${VERUS:?Set VERUS to the pinned Verus executable}"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
temp_parent=$(CDPATH= cd -- "${RUNNER_TEMP:-/tmp}" && pwd)
base=$(mktemp -d "$temp_parent/fe2o3-checked-u32-prefix.XXXXXXXX")
cleanup() {
    rm -rf -- "$base"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

python3 -I -B "$here/checked_u32_prefix_check.py" --verus "$VERUS" --output "$base/proof"
printf '%s\n' 'PASS: shared checked-u32 argument basis and prefix fold; 18 verified obligations, 16 logical mutants, 8 runner controls; no full normalization or application authority claim.'
