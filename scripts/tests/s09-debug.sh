#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECKER="${ROOT}/scripts/s09-debug-check.py"
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly FIXTURES="${ROOT}/scripts/tests/fixtures/s09-debug"
TMP="$(mktemp -d)"
readonly TMP
trap 'rm -rf -- "${TMP}"' EXIT

expect_fail() {
  if "$@" >/dev/null 2>&1; then
    printf 'command unexpectedly succeeded:' >&2
    printf ' %q' "$@" >&2
    printf '\n' >&2
    exit 1
  fi
}

"${CHECKER}" check-dwarf --input "${FIXTURES}/dwarf.pass.txt"
"${CHECKER}" check-rocgdb --input "${FIXTURES}/rocgdb.pass.txt"
"${CHECKER}" normalize-dwarf \
  --input "${FIXTURES}/dwarf.pass.txt" --output "${TMP}/dwarf.one"
"${CHECKER}" normalize-dwarf \
  --input "${FIXTURES}/dwarf.pass.txt" --output "${TMP}/dwarf.two"
cmp "${TMP}/dwarf.one" "${TMP}/dwarf.two"
"${CHECKER}" normalize-rocgdb \
  --input "${FIXTURES}/rocgdb.pass.txt" --output "${TMP}/rocgdb.one"
"${CHECKER}" normalize-rocgdb \
  --input "${FIXTURES}/rocgdb.pass.txt" --output "${TMP}/rocgdb.two"
cmp "${TMP}/rocgdb.one" "${TMP}/rocgdb.two"
rg -q '^Breakpoint .+\$REPO/crates/.+main\.rs:68$' "${TMP}/rocgdb.one"
rg -q '0x<ADDR>' "${TMP}/rocgdb.one"
expect_fail "${CHECKER}" normalize-dwarf \
  --input "${FIXTURES}/dwarf.pass.txt" --output "${TMP}/dwarf.one"

sed 's/DW_AT_name ("i")/DW_AT_name ("j")/' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-missing-local"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-missing-local"
sed 's/DW_AT_location (DW_OP_regx VGPR0)/DW_AT_const_value (0)/' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-missing-location"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-missing-location"
sed 's/ 70 13 / 71 13 /' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-missing-line"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-missing-line"

sed '/^i = /d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-local"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-missing-local"
sed 's/scale = 1.5/scale = <optimized out>/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-optimized"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-optimized"
sed 's/scale = 1.5/scale = Could not find the frame base/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-frame-base"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-frame-base"
sed 's/input_len = 1/input_len = 2/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-wrong-length"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-wrong-length"
sed '/FE2O3_S09_BEGIN/a FE2O3_S09_BEGIN' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-duplicate-marker"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-duplicate-marker"
ln -s "${FIXTURES}/rocgdb.pass.txt" "${TMP}/rocgdb-link"
expect_fail "${CHECKER}" check-rocgdb --input "${TMP}/rocgdb-link"

rg -q '^readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3\.12$' "${RUNNER}"
rg -q -- "--batch --nx --nh" "${RUNNER}"
expect_fail rg -q -- '--command|command-file|FE2O3_.*DEBUG.*COMMAND' "${RUNNER}"
expect_fail "${RUNNER}"

printf 'S09 debug checker tests passed\n'
