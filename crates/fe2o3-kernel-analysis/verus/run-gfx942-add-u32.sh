#!/bin/sh
set -eu

: "${VERUS:?Set VERUS to the pinned Verus executable}"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
temp_parent=$(CDPATH= cd -- "${RUNNER_TEMP:-/tmp}" && pwd)
base=$(mktemp -d "$temp_parent/fe2o3-gfx942-add-u32.XXXXXXXX")
cleanup() {
    rm -rf -- "$base"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

python3 -I -B "$here/gfx942_add_u32_test.py"
python3 -I -B "$here/gfx942_add_u32_check.py" \
    --verus "$VERUS" --output "$base/proof"
printf '%s\n' 'PASS: shared arithmetic and conditional 102-SGPR/SCC transition, 3 arithmetic + 5 state logical mutants, 12 runner controls; no decoder, ISA or application authority claim.'
