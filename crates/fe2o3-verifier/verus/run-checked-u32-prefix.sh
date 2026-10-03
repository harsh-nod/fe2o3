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
printf '%s\n' 'PASS: shared checked-u32 source assembly, normalization, basis and fold; 71 verified obligations, 48 logical mutants, 12 runner controls; terminal AST, KIR and application authority remain separate.'
