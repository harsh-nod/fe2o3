#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECKER="${ROOT}/scripts/s09-debug-check.py"
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly FIXTURES="${ROOT}/scripts/tests/fixtures/s09-debug"
readonly HSACO_SHA256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
readonly HARDWARE_SHA256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
readonly HARDWARE_BUILD_ID=cccccccccccccccccccccccccccccccccccccccc
readonly HARDWARE_TEST=gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator
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

check_rocgdb() {
  "${CHECKER}" check-rocgdb \
    --input "$1" \
    --hsaco-sha256 "${HSACO_SHA256}" \
    --hardware-sha256 "${HARDWARE_SHA256}" \
    --hardware-build-id "${HARDWARE_BUILD_ID}"
}

"${CHECKER}" check-dwarf --input "${FIXTURES}/dwarf.pass.txt"
check_rocgdb "${FIXTURES}/rocgdb.pass.txt"
"${CHECKER}" artifact-facts \
  --metadata "${FIXTURES}/artifact.pass.txt" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/artifact.facts"
"${CHECKER}" hardware-facts \
  --input "${FIXTURES}/hardware.pass.txt" \
  --sha256 "${HARDWARE_SHA256}" \
  --output "${TMP}/hardware.facts"
rg -q '^target=gfx942:xnack-$' "${TMP}/artifact.facts"
rg -q '^optimization=O0$' "${TMP}/artifact.facts"
rg -q "^build_id=${HARDWARE_BUILD_ID}$" "${TMP}/hardware.facts"

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
rg -q "AMDGPU Wave <WAVE>.*alpha.*crates/.+main\.rs:68" "${TMP}/rocgdb.one"
rg -q 'memory://<PID>#offset=0x<ADDR>&size=<SIZE>' "${TMP}/rocgdb.one"
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
sed 's#crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#/checkout/crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-absolute-source"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-absolute-source"
sed 's#crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/../src/main.rs#' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-dotdot-source"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-dotdot-source"
sed '/DW_TAG_compile_unit/a private_source = /home/harsh/private/build.rs' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-unrelated-absolute"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-unrelated-absolute"

sed '/^i = /d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-local"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-local"
sed '/hit Breakpoint 2/d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-bp2-hit"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp2-hit"
sed '/hit Breakpoint 3/d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-bp3-hit"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp3-hit"
sed '/AMDGPU Wave .*main.rs:69/d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-bp2-wave"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp2-wave"
sed '/AMDGPU Wave .*main.rs:70/d' "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-missing-bp3-wave"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp3-wave"
sed 's/scale = 1.5/scale = <optimized out>/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-optimized"
expect_fail check_rocgdb "${TMP}/rocgdb-optimized"
sed 's/input_len = 1/input_len = 2/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-wrong-length"
expect_fail check_rocgdb "${TMP}/rocgdb-wrong-length"
sed 's/AMDGPU Wave/CPU Thread/' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-host-alpha"
expect_fail check_rocgdb "${TMP}/rocgdb-host-alpha"
sed '/Breakpoint 1 (alpha) pending[.]/d' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-no-kernel-load"
expect_fail check_rocgdb "${TMP}/rocgdb-no-kernel-load"
sed "/test ${HARDWARE_TEST} ... ok/d" \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-no-hardware-pass"
expect_fail check_rocgdb "${TMP}/rocgdb-no-hardware-pass"
sed "s/${HSACO_SHA256}/dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd/" \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-substitute-hsaco"
expect_fail check_rocgdb "${TMP}/rocgdb-substitute-hsaco"
sed "s/${HARDWARE_BUILD_ID}/dddddddddddddddddddddddddddddddddddddddd/" \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-substitute-host"
expect_fail check_rocgdb "${TMP}/rocgdb-substitute-host"
sed 's#crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#/other-root/crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-cross-root"
expect_fail check_rocgdb "${TMP}/rocgdb-cross-root"
sed '/FE2O3_S09_BINDING/a leak = /home/harsh/private/build.rs' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-posix-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-posix-leak"
sed '/FE2O3_S09_BINDING/a leak = C:\\private\\build.rs' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-windows-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-windows-leak"
sed '/FE2O3_S09_BEGIN/a FE2O3_S09_BEGIN' \
  "${FIXTURES}/rocgdb.pass.txt" >"${TMP}/rocgdb-duplicate-marker"
expect_fail check_rocgdb "${TMP}/rocgdb-duplicate-marker"
ln -s "${FIXTURES}/rocgdb.pass.txt" "${TMP}/rocgdb-link"
expect_fail check_rocgdb "${TMP}/rocgdb-link"

sed "s/gfx942:xnack-/gfx90a:xnack-/" \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-wrong-target"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-wrong-target" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/wrong-target.facts"
sed '/Build ID:/d' "${FIXTURES}/hardware.pass.txt" >"${TMP}/hardware-no-build-id"
expect_fail "${CHECKER}" hardware-facts \
  --input "${TMP}/hardware-no-build-id" \
  --sha256 "${HARDWARE_SHA256}" \
  --output "${TMP}/no-build-id.facts"

rg -q '^readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3\.12$' "${RUNNER}"
rg -q '^readonly READOBJ=/opt/rocm/llvm/bin/llvm-readobj$' "${RUNNER}"
rg -q '^readonly READELF=/opt/rocm/llvm/bin/llvm-readobj$' "${RUNNER}"
rg -q -- '--batch --nx --nh' "${RUNNER}"
rg -q 'FE2O3_S09_HARDWARE_PASS' "${RUNNER}"
rg -q 'FE2O3_S09_BP2_ARMED' "${RUNNER}"
rg -q 'FE2O3_S09_BP3_STOP' "${RUNNER}"
rg -Fq 'rm -f -- "${ROCGDB_RAW}"' "${RUNNER}"
expect_fail rg -q -- '--command|command-file|FE2O3_.*DEBUG.*COMMAND' "${RUNNER}"
expect_fail "${RUNNER}"

printf 'S09 debug checker tests passed\n'
