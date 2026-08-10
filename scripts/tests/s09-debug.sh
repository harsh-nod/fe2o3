#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECKER="${ROOT}/scripts/s09-debug-check.py"
readonly RUNNER="${ROOT}/scripts/s09-rocgdb-profile.sh"
readonly FIXTURES="${ROOT}/scripts/tests/fixtures/s09-debug"
readonly IDENTITY_FIXTURE="${ROOT}/scripts/tests/s09-debug-identity-fixture.py"
readonly FIXTURE_DWARFDUMP="${FIXTURES}/llvm-dwarfdump"
readonly FIXTURE_READOBJ="${FIXTURES}/llvm-readobj"
readonly WRONG_FIXTURE_READOBJ="${FIXTURES}/llvm-readobj-wrong-artifact"
readonly FLOOD_FIXTURE="${FIXTURES}/llvm-output-flood"
readonly ROCGDB_HSACO_SHA256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
readonly HARDWARE_SHA256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
readonly HARDWARE_BUILD_ID=cccccccccccccccccccccccccccccccccccccccc
readonly RUN_NONCE=dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
TMP="$(mktemp -d)"
readonly TMP
trap 'rm -rf -- "${TMP}"' EXIT
readonly ROCGDB_FIXTURE="${TMP}/rocgdb.pass.txt"
cp -- "${FIXTURES}/rocgdb.pass.txt" "${ROCGDB_FIXTURE}"

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
    --hsaco-sha256 "${ROCGDB_HSACO_SHA256}" \
    --hardware-sha256 "${HARDWARE_SHA256}" \
    --hardware-build-id "${HARDWARE_BUILD_ID}"
}

"${CHECKER}" check-dwarf --input "${FIXTURES}/dwarf.pass.txt"
check_rocgdb "${ROCGDB_FIXTURE}"
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
  --input "${ROCGDB_FIXTURE}" --output "${TMP}/rocgdb.one"
"${CHECKER}" normalize-rocgdb \
  --input "${ROCGDB_FIXTURE}" --output "${TMP}/rocgdb.two"
cmp "${TMP}/rocgdb.one" "${TMP}/rocgdb.two"
rg -q "AMDGPU Wave <WAVE>.*alpha.*crates/.+main\.rs:68" "${TMP}/rocgdb.one"
rg -q 'memory://<PID>#offset=0x<ADDR>&size=<SIZE>' "${TMP}/rocgdb.one"
rg -q '0x<ADDR>' "${TMP}/rocgdb.one"
rg -q '^HOST_THREAD_FRAME$' "${TMP}/rocgdb.one"
rg -q "^run_nonce = ${RUN_NONCE}$" "${TMP}/rocgdb.one"
expect_fail rg -q 'sysdeps|\.\./' "${TMP}/rocgdb.one"

readonly MANIFEST="${TMP}/protected-manifest.tsv"
readonly DEBUG_ARCHIVE_MANIFEST="${TMP}/debug-archive-manifest.txt"
readonly HSACO="${TMP}/identity.hsaco"
readonly HOST_EXECUTABLE="${TMP}/host-executable"
readonly IDENTITY_FIELDS="${TMP}/identity-fields.tsv"
"${IDENTITY_FIXTURE}" --output "${HSACO}"
cp -- /bin/true "${HOST_EXECUTABLE}"
chmod 400 "${HOST_EXECUTABLE}"
"${CHECKER}" identity-fields --hsaco "${HSACO}" --output "${IDENTITY_FIELDS}"
HSACO_SHA256="$(sha256sum "${HSACO}" | cut -d ' ' -f 1)"
HOST_EXECUTABLE_SHA256="$(sha256sum "${HOST_EXECUTABLE}" | cut -d ' ' -f 1)"
readonly HSACO_SHA256 HOST_EXECUTABLE_SHA256
"${CHECKER}" hardware-facts \
  --input "${FIXTURES}/hardware.pass.txt" \
  --sha256 "${HOST_EXECUTABLE_SHA256}" \
  --output "${TMP}/bundle.hardware.facts"
sed \
  -e "s/${ROCGDB_HSACO_SHA256}/${HSACO_SHA256}/g" \
  -e "s/${HARDWARE_SHA256}/${HOST_EXECUTABLE_SHA256}/g" \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb.bundle.raw"
"${CHECKER}" normalize-rocgdb \
  --input "${TMP}/rocgdb.bundle.raw" --output "${TMP}/rocgdb.bundle"
artifact_sha256="$(sha256sum "${TMP}/artifact.facts" | cut -d ' ' -f 1)"
hardware_facts_sha256="$(sha256sum "${TMP}/bundle.hardware.facts" | cut -d ' ' -f 1)"
dwarf_sha256="$(sha256sum "${TMP}/dwarf.one" | cut -d ' ' -f 1)"
rocgdb_sha256="$(sha256sum "${TMP}/rocgdb.bundle" | cut -d ' ' -f 1)"
readonly artifact_sha256 hardware_facts_sha256 dwarf_sha256 rocgdb_sha256
checker_sha256="$(sha256sum "${CHECKER}" | cut -d ' ' -f 1)"
rocgdb_raw_sha256="$(sha256sum "${TMP}/rocgdb.bundle.raw" | cut -d ' ' -f 1)"
readonly checker_sha256 rocgdb_raw_sha256
{
  printf 'format=fe2o3-s09-debug-archive-v2\n'
  printf 'profile=s09-alpha-gfx942-o0-v1\n'
  printf 'result=passed\n'
  printf 'target=gfx942:xnack-\n'
  printf 'optimization=O0\n'
  printf 'rocgdb=/opt/rocm/bin/rocgdb-py_3.12\n'
  printf 'llvm_dwarfdump=/opt/rocm/llvm/bin/llvm-dwarfdump\n'
  printf 'llvm_readobj=/opt/rocm/llvm/bin/llvm-readobj\n'
  printf 'llvm_readelf=/opt/rocm/llvm/bin/llvm-readobj --elf-output-style=GNU\n'
  printf 'hsaco_sha256=%s\n' "${HSACO_SHA256}"
  printf 'hardware_test_sha256=%s\n' "${HOST_EXECUTABLE_SHA256}"
  printf 'hardware_test_build_id=%s\n' "${HARDWARE_BUILD_ID}"
  printf 'run_nonce=%s\n' "${RUN_NONCE}"
  printf 'checker_sha256=%s\n' "${checker_sha256}"
  printf 'artifact_facts_sha256=%s\n' "${artifact_sha256}"
  printf 'hardware_facts_sha256=%s\n' "${hardware_facts_sha256}"
  printf 'dwarf_normalized_sha256=%s\n' "${dwarf_sha256}"
  printf 'rocgdb_normalized_sha256=%s\n' "${rocgdb_sha256}"
  printf 'rocgdb_raw_sha256=%s\n' "${rocgdb_raw_sha256}"
  printf 'rocgdb_raw_retained=false\n'
  for status in \
    dwarf_verify_status dwarf_dump_status dwarf_normalize_status \
    dwarf_check_status artifact_read_status artifact_check_status \
    hardware_read_status hardware_check_status rocgdb_status \
    rocgdb_normalize_status rocgdb_check_status; do
    printf '%s=0\n' "${status}"
  done
} >"${DEBUG_ARCHIVE_MANIFEST}"
debug_archive_manifest_sha256="$(sha256sum "${DEBUG_ARCHIVE_MANIFEST}" | cut -d ' ' -f 1)"
readonly debug_archive_manifest_sha256
{
  printf 'manifest_schema\tfe2o3-s09-protected-manifest-v2\n'
  printf 'trust_domain\ttest-fixture-v2\n'
  printf 'claim\tsource-debug-evidence-v2\n'
  cat -- "${IDENTITY_FIELDS}"
  printf 'source_commit\t1111111111111111111111111111111111111111\n'
  printf 'source_tree\t2222222222222222222222222222222222222222\n'
  printf 'hsaco_sha256\t%s\n' "${HSACO_SHA256}"
  printf 'host_executable_sha256\t%s\n' "${HOST_EXECUTABLE_SHA256}"
  printf 'host_executable_build_id\t%s\n' "${HARDWARE_BUILD_ID}"
  printf 'debug_archive_manifest_sha256\t%s\n' "${debug_archive_manifest_sha256}"
  printf 'artifact_facts_sha256\t%s\n' "${artifact_sha256}"
  printf 'hardware_facts_sha256\t%s\n' "${hardware_facts_sha256}"
  printf 'dwarf_normalized_sha256\t%s\n' "${dwarf_sha256}"
  printf 'rocgdb_normalized_sha256\t%s\n' "${rocgdb_sha256}"
} >"${MANIFEST}"
manifest_sha256="$(sha256sum "${MANIFEST}" | cut -d ' ' -f 1)"
readonly manifest_sha256
fixture_args=(
  "${CHECKER}" check-fixture
  --manifest "${MANIFEST}"
  --expected-manifest-sha256 "${manifest_sha256}"
  --hsaco "${HSACO}"
  --host-executable "${HOST_EXECUTABLE}"
  --debug-archive-manifest "${DEBUG_ARCHIVE_MANIFEST}"
  --artifact-facts "${TMP}/artifact.facts"
  --hardware-facts "${TMP}/bundle.hardware.facts"
  --dwarf "${TMP}/dwarf.one"
  --rocgdb "${TMP}/rocgdb.bundle"
  --fixture-llvm-dwarfdump "${FIXTURE_DWARFDUMP}"
  --fixture-llvm-readobj "${FIXTURE_READOBJ}"
)
evidence_args=(
  --hsaco "${HSACO}"
  --host-executable "${HOST_EXECUTABLE}"
  --debug-archive-manifest "${DEBUG_ARCHIVE_MANIFEST}"
  --artifact-facts "${TMP}/artifact.facts"
  --hardware-facts "${TMP}/bundle.hardware.facts"
  --dwarf "${TMP}/dwarf.one"
  --rocgdb "${TMP}/rocgdb.bundle"
)
fixture_tool_args=(
  --fixture-llvm-dwarfdump "${FIXTURE_DWARFDUMP}"
  --fixture-llvm-readobj "${FIXTURE_READOBJ}"
)
"${fixture_args[@]}" >"${TMP}/fixture.out"
rg -q '^S09 non-authoritative fixture checker passed$' "${TMP}/fixture.out"
expect_fail rg -qi 'production.*accepted|authoritative.*accepted' "${TMP}/fixture.out"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  "${evidence_args[@]}" \
  --fixture-llvm-dwarfdump "${FIXTURE_DWARFDUMP}" \
  --fixture-llvm-readobj "${WRONG_FIXTURE_READOBJ}"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  "${evidence_args[@]}" \
  --fixture-llvm-dwarfdump "${FLOOD_FIXTURE}" \
  --fixture-llvm-readobj "${FIXTURE_READOBJ}"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  "${evidence_args[@]}" \
  --fixture-llvm-dwarfdump "${FIXTURE_DWARFDUMP}" \
  --fixture-llvm-readobj "${FLOOD_FIXTURE}"

check_debug_manifest_mutation() {
  local name="$1"
  local mutated_debug="${TMP}/debug-${name}.txt"
  local mutated_manifest="${TMP}/manifest-debug-${name}.tsv"
  shift
  "$@" "${DEBUG_ARCHIVE_MANIFEST}" >"${mutated_debug}"
  local mutated_debug_sha256
  mutated_debug_sha256="$(sha256sum "${mutated_debug}" | cut -d ' ' -f 1)"
  awk -F '\t' -v digest="${mutated_debug_sha256}" '
    BEGIN { OFS = "\t" }
    $1 == "debug_archive_manifest_sha256" { $2 = digest }
    { print }
  ' "${MANIFEST}" >"${mutated_manifest}"
  local mutated_manifest_sha256
  mutated_manifest_sha256="$(sha256sum "${mutated_manifest}" | cut -d ' ' -f 1)"
  expect_fail "${CHECKER}" check-fixture \
    --manifest "${mutated_manifest}" \
    --expected-manifest-sha256 "${mutated_manifest_sha256}" \
    --hsaco "${HSACO}" \
    --host-executable "${HOST_EXECUTABLE}" \
    --debug-archive-manifest "${mutated_debug}" \
    --artifact-facts "${TMP}/artifact.facts" \
    --hardware-facts "${TMP}/bundle.hardware.facts" \
    --dwarf "${TMP}/dwarf.one" \
    --rocgdb "${TMP}/rocgdb.bundle" \
    "${fixture_tool_args[@]}"
}

check_debug_manifest_mutation missing sed '$d'
check_debug_manifest_mutation duplicate awk '{ print; if ($0 ~ /^profile=/) print }'
check_debug_manifest_mutation failed-status sed 's/^rocgdb_status=0$/rocgdb_status=1/'
check_debug_manifest_mutation wrong-nonce sed \
  's/^run_nonce=.*/run_nonce=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/'

cp -- "${HOST_EXECUTABLE}" "${TMP}/host-substitute"
chmod 600 "${TMP}/host-substitute"
printf X | dd of="${TMP}/host-substitute" bs=1 seek=64 count=1 conv=notrunc status=none
chmod 400 "${TMP}/host-substitute"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  --hsaco "${HSACO}" \
  --host-executable "${TMP}/host-substitute" \
  --debug-archive-manifest "${DEBUG_ARCHIVE_MANIFEST}" \
  --artifact-facts "${TMP}/artifact.facts" \
  --hardware-facts "${TMP}/bundle.hardware.facts" \
  --dwarf "${TMP}/dwarf.one" \
  --rocgdb "${TMP}/rocgdb.bundle" \
  "${fixture_tool_args[@]}"

check_identity_substitution() {
  local field replacement substituted_manifest substituted_sha256
  field="$1"
  replacement="$2"
  substituted_manifest="${TMP}/${field}-substitution.tsv"
  awk -F '\t' -v key="${field}" -v value="${replacement}" '
    BEGIN { OFS = "\t" }
    $1 == key { $2 = value; found += 1 }
    { print }
    END { if (found != 1) exit 2 }
  ' "${MANIFEST}" >"${substituted_manifest}"
  substituted_sha256="$(sha256sum "${substituted_manifest}" | cut -d ' ' -f 1)"
  expect_fail "${CHECKER}" check-fixture \
    --manifest "${substituted_manifest}" \
    --expected-manifest-sha256 "${substituted_sha256}" \
    "${evidence_args[@]}" \
    "${fixture_tool_args[@]}"
}

check_identity_substitution \
  build_pinned_cargo_image_sha256 \
  dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
check_identity_substitution build_observed_parent_pid 4243
check_identity_substitution build_observed_parent_start_time_ticks 9002

sed \
  -e 's/^trust_domain\ttest-fixture-v2$/trust_domain\tlocal-capability-v2/' \
  "${MANIFEST}" >"${TMP}/capability-manifest.tsv"
capability_manifest_sha256="$(sha256sum "${TMP}/capability-manifest.tsv" | cut -d ' ' -f 1)"
"${CHECKER}" check-capability \
  --manifest "${TMP}/capability-manifest.tsv" \
  --expected-manifest-sha256 "${capability_manifest_sha256}" \
  "${evidence_args[@]}" \
  "${fixture_tool_args[@]}" >"${TMP}/capability.out"
rg -q '^S09 non-authoritative capability manifest V2 accepted$' "${TMP}/capability.out"
expect_fail rg -q '^S09 production trust policy accepted' "${TMP}/capability.out"
"${CHECKER}" check-production --help >"${TMP}/production-help"
expect_fail rg -q -- '--manifest|--expected-manifest-sha256' "${TMP}/production-help"
rg -q -- '--debug-archive-manifest' "${TMP}/production-help"
production_evidence_args=(
  "${CHECKER}" check-production
  "${evidence_args[@]}"
)
expect_fail "${production_evidence_args[@]}"
expect_fail "${CHECKER}" check-production \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  "${evidence_args[@]}"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${TMP}/absent-manifest.tsv" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  "${evidence_args[@]}" \
  "${fixture_tool_args[@]}"
cp "${TMP}/artifact.facts" "${TMP}/artifact-mutated.facts"
printf 'mutation=true\n' >>"${TMP}/artifact-mutated.facts"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${MANIFEST}" \
  --expected-manifest-sha256 "${manifest_sha256}" \
  --hsaco "${HSACO}" \
  --host-executable "${HOST_EXECUTABLE}" \
  --artifact-facts "${TMP}/artifact-mutated.facts" \
  --hardware-facts "${TMP}/bundle.hardware.facts" \
  --dwarf "${TMP}/dwarf.one" \
  --rocgdb "${TMP}/rocgdb.bundle" \
  "${fixture_tool_args[@]}"
sed 's/^trust_domain\ttest-fixture-v2$/trust_domain\tproduction-v2/' \
  "${MANIFEST}" >"${TMP}/production-domain-manifest.tsv"
production_domain_sha256="$(sha256sum "${TMP}/production-domain-manifest.tsv" | cut -d ' ' -f 1)"
expect_fail "${CHECKER}" check-fixture \
  --manifest "${TMP}/production-domain-manifest.tsv" \
  --expected-manifest-sha256 "${production_domain_sha256}" \
  "${evidence_args[@]}" \
  "${fixture_tool_args[@]}"
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
sed '/DW_TAG_compile_unit/a private_source = file:///home/harsh/private/build.rs' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-file-uri"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-file-uri"
sed '/DW_TAG_compile_unit/a private_source = https://host/etc/passwd' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-uri-absolute"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-uri-absolute"
sed '/DW_TAG_compile_unit/a private_source = file:%2Fhome%2Fharsh%2Fprivate.rs' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-percent-absolute"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-percent-absolute"
sed '/DW_TAG_compile_unit/a private_source](/home/harsh/private.rs)' \
  "${FIXTURES}/dwarf.pass.txt" >"${TMP}/dwarf-delimiter-absolute"
expect_fail "${CHECKER}" check-dwarf --input "${TMP}/dwarf-delimiter-absolute"

sed '/^i = /d' "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-missing-local"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-local"
sed '/hit Breakpoint 2/d' "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-missing-bp2-hit"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp2-hit"
sed '/hit Breakpoint 3/d' "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-missing-bp3-hit"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp3-hit"
sed '/AMDGPU Wave .*main.rs:69/d' "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-missing-bp2-wave"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp2-wave"
sed '/AMDGPU Wave .*main.rs:70/d' "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-missing-bp3-wave"
expect_fail check_rocgdb "${TMP}/rocgdb-missing-bp3-wave"
sed 's/scale = 1.5/scale = <optimized out>/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-optimized"
expect_fail check_rocgdb "${TMP}/rocgdb-optimized"
sed 's/input_len = 1/input_len = 2/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-wrong-length"
expect_fail check_rocgdb "${TMP}/rocgdb-wrong-length"
sed 's/AMDGPU Wave/CPU Thread/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-host-alpha"
expect_fail check_rocgdb "${TMP}/rocgdb-host-alpha"
sed '/Breakpoint 1 (alpha) pending[.]/d' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-no-kernel-load"
expect_fail check_rocgdb "${TMP}/rocgdb-no-kernel-load"
sed '/FE2O3_S09_HARNESS_RESULT_V1/d' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-no-hardware-pass"
expect_fail check_rocgdb "${TMP}/rocgdb-no-hardware-pass"
sed '/^test .* \.\.\. ok$/d; /^test result: /d' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-no-cargo-status"
check_rocgdb "${TMP}/rocgdb-no-cargo-status"
sed '/FE2O3_S09_HARNESS_RESULT_V1/s/result=passed/result=failed/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-forged-result"
expect_fail check_rocgdb "${TMP}/rocgdb-forged-result"
sed '/FE2O3_S09_HARNESS_RESULT_V1/s/run_nonce=[0-9a-f]*/run_nonce=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-forged-nonce"
expect_fail check_rocgdb "${TMP}/rocgdb-forged-nonce"
sed '/FE2O3_S09_HARNESS_RESULT_V1/s/hsaco_sha256=[0-9a-f]*/hsaco_sha256=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-forged-result-hsaco"
expect_fail check_rocgdb "${TMP}/rocgdb-forged-result-hsaco"
sed '/FE2O3_S09_ROCGDB_EXIT_STATUS = 0/d' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-no-exit-status"
expect_fail check_rocgdb "${TMP}/rocgdb-no-exit-status"
sed "s/${ROCGDB_HSACO_SHA256}/dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd/" \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-substitute-hsaco"
expect_fail check_rocgdb "${TMP}/rocgdb-substitute-hsaco"
sed "s/${HARDWARE_BUILD_ID}/dddddddddddddddddddddddddddddddddddddddd/" \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-substitute-host"
expect_fail check_rocgdb "${TMP}/rocgdb-substitute-host"
sed 's#crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#/other-root/crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs#' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-cross-root"
expect_fail check_rocgdb "${TMP}/rocgdb-cross-root"
sed '/FE2O3_S09_BINDING/a leak = /home/harsh/private/build.rs' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-posix-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-posix-leak"
sed '/FE2O3_S09_BINDING/a leak = C:\\private\\build.rs' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-windows-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-windows-leak"
sed '/FE2O3_S09_BINDING/a leak = file:\/\/\/C:\\private\\build.rs' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-windows-file-uri"
expect_fail check_rocgdb "${TMP}/rocgdb-windows-file-uri"
sed '/FE2O3_S09_BINDING/a leak = \\\\server\\share\\build.rs' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-unc-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-unc-leak"
sed '/FE2O3_S09_BINDING/a leak = \\\\?\\C:\\private\\build.rs' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-device-leak"
expect_fail check_rocgdb "${TMP}/rocgdb-device-leak"
sed '/FE2O3_S09_BEGIN/a FE2O3_S09_BEGIN' \
  "${ROCGDB_FIXTURE}" >"${TMP}/rocgdb-duplicate-marker"
expect_fail check_rocgdb "${TMP}/rocgdb-duplicate-marker"
ln -s "${ROCGDB_FIXTURE}" "${TMP}/rocgdb-link"
expect_fail check_rocgdb "${TMP}/rocgdb-link"

sed "s/gfx942:xnack-/gfx90a:xnack-/" \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-wrong-target"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-wrong-target" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/wrong-target.facts"
sed 's/ABIVersion: 4/ABIVersion: 3/' \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-wrong-cov"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-wrong-cov" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/wrong-cov.facts"
sed 's/\.kernarg_segment_size: 296/.kernarg_segment_size: 295/' \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-wrong-kernarg"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-wrong-kernarg" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/wrong-kernarg.facts"
sed '/    \.symbol:         alpha\.kd/a\
  - .name:           zeta\
    .symbol:         zeta.kd' \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-extra-zeta"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-extra-zeta" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/extra-zeta.facts"
sed 's/\.symbol:         alpha\.kd/.symbol:         wrong.kd/' \
  "${FIXTURES}/artifact.pass.txt" >"${TMP}/artifact-wrong-symbol"
expect_fail "${CHECKER}" artifact-facts \
  --metadata "${TMP}/artifact-wrong-symbol" \
  --dwarf "${FIXTURES}/dwarf.pass.txt" \
  --output "${TMP}/wrong-symbol.facts"
sed '/Build ID:/d' "${FIXTURES}/hardware.pass.txt" >"${TMP}/hardware-no-build-id"
expect_fail "${CHECKER}" hardware-facts \
  --input "${TMP}/hardware-no-build-id" \
  --sha256 "${HARDWARE_SHA256}" \
  --output "${TMP}/no-build-id.facts"

rg -q '^readonly ROCGDB=/opt/rocm/bin/rocgdb-py_3\.12$' "${RUNNER}"
rg -q '^readonly READOBJ=/opt/rocm/llvm/bin/llvm-readobj$' "${RUNNER}"
rg -Fq -- '--file-headers --notes "${HSACO}"' "${RUNNER}"
rg -q '^readonly READELF=/opt/rocm/llvm/bin/llvm-readobj$' "${RUNNER}"
rg -Fq '/etc/fe2o3/s09-trust-v2.tsv' "${CHECKER}"
rg -q 'FS_IMMUTABLE_FL' "${CHECKER}"
rg -q 'O_NOFOLLOW' "${CHECKER}"
rg -q -- '--batch --nx --nh' "${RUNNER}"
rg -q -- '--return-child-result' "${RUNNER}"
rg -q 'FE2O3_S09_HARDWARE_PASS' "${RUNNER}"
rg -q 'FE2O3_S09_HARNESS_RESULT_V1' "${RUNNER}"
expect_fail rg -Fq -- "-ex 'if \$_exitcode" "${RUNNER}"
expect_fail rg -q 'FE2O3_S09_HARNESS_RESULT_V1' \
  "${ROOT}/crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs"
rg -q 's09_gfx942_cov6_alpha_only_controller' "${RUNNER}"
rg -q 'FE2O3_S09_GFX942_ALPHA_SHA256' "${RUNNER}"
rg -q 'FE2O3_S09_GFX942_ALPHA_HSACO_DEVICE' "${RUNNER}"
rg -q 'FE2O3_S09_GFX942_ALPHA_HSACO_OWNER_START_TIME_TICKS' "${RUNNER}"
rg -q 'FE2O3_S09_GFX942_ALPHA_FACTS_INODE' "${RUNNER}"
rg -q 'FE2O3_S09_GFX942_ALPHA_FACTS_SHA256' "${RUNNER}"
expect_fail rg -q 'FE2O3_GFX942_ALPHA_ZETA' "${RUNNER}"
rg -q 'FE2O3_S09_RUN_NONCE' "${RUNNER}"
rg -q 'FE2O3_S09_ROCGDB_EXIT_STATUS' "${RUNNER}"
rg -q 'FE2O3_S09_BP2_ARMED' "${RUNNER}"
rg -q 'FE2O3_S09_BP3_STOP' "${RUNNER}"
rg -Fq "info sharedlibrary memory://" "${RUNNER}"
expect_fail rg -Fq -- "-ex 'info sharedlibrary'" "${RUNNER}"
rg -Fq 's09_install_raw_transcript_guard "${ROCGDB_RAW}"' "${RUNNER}"
rg -Fq 's09_delete_raw_transcript' "${RUNNER}"
rg -Fq '/usr/bin/prlimit' "${RUNNER}"
rg -Fq -- '--fsize="${TOOL_OUTPUT_LIMIT_BYTES}:${TOOL_OUTPUT_LIMIT_BYTES}"' "${RUNNER}"
expect_fail rg -q -- '--command|command-file|FE2O3_.*DEBUG.*COMMAND' "${RUNNER}"
expect_fail "${RUNNER}"
PYTHONDONTWRITEBYTECODE=1 python3 "${ROOT}/scripts/tests/s09-raw-transcript-guard.py"
python3 "${ROOT}/scripts/tests/s09-debug-policy.py"

printf 'S09 debug checker tests passed\n'
