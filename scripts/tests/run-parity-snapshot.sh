#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${TEST_SCRIPT_DIR}/../.." && pwd)"
readonly RUNNER="${REPO_ROOT}/scripts/run-parity-snapshot.sh"
readonly TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEST_ROOT}"' EXIT

OUTPUT=""
STATUS=0

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

expect_failure() {
  set +e
  OUTPUT="$("$@" 2>&1)"
  STATUS=$?
  set -e
  ((STATUS != 0)) || fail "command unexpectedly passed: $*"
}

assert_contains() {
  local needle="$1"
  [[ "${OUTPUT}" == *"${needle}"* ]] ||
    fail "output did not contain '${needle}': ${OUTPUT}"
}

hex_encode() {
  printf '%s' "$1" | od -An -v -tx1 | tr -d ' \n'
}

write_fake_repo() {
  local repo="$1"
  mkdir -p -- "${repo}/scripts/tests"

  cat >"${repo}/scripts/ci-local.sh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
mkdir -p -- "${FE2O3_EVIDENCE_OUTPUT_DIR}"
printf 'ci-local\t%s\t%s\t%s\n' "$1" "${CARGO_TARGET_DIR}" "${TMPDIR}" \
  >>"${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
case "$1" in
  workspace-test | rocm-compile) ;;
  verus)
    printf 'verus-environment\t%s\t%s\n' \
      "${FE2O3_RUNTIME_MODEL_VERUS:-}" "${VERUS:-}" \
      >>"${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
    ;;
  *) exit 90 ;;
esac
EOF

  local script
  for script in \
    scripts/parity-matrix.sh \
    scripts/tests/parity-matrix.sh \
    scripts/parity-dashboard.sh \
    scripts/tests/parity-dashboard.sh \
    scripts/tests/parity-evidence.sh \
    scripts/tests/differential-conformance.sh
  do
    cat >"${repo}/${script}" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
mkdir -p -- "${FE2O3_EVIDENCE_OUTPUT_DIR}"
printf 'script\t%s\t%s\n' "$0" "${CARGO_TARGET_DIR}" \
  >>"${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
EOF
  done
  chmod 755 "${repo}/scripts/ci-local.sh" \
    "${repo}/scripts/parity-matrix.sh" \
    "${repo}/scripts/tests/parity-matrix.sh" \
    "${repo}/scripts/parity-dashboard.sh" \
    "${repo}/scripts/tests/parity-dashboard.sh" \
    "${repo}/scripts/tests/parity-evidence.sh" \
    "${repo}/scripts/tests/differential-conformance.sh"

  git -C "${repo}" init -q
  git -C "${repo}" config user.email parity-test@example.invalid
  git -C "${repo}" config user.name 'Parity Test'
  git -C "${repo}" add scripts
  git -C "${repo}" commit -qm fixture
}

write_fake_tools() {
  local directory="$1"
  local exit_status="$2"
  mkdir -p -- "${directory}"
  cat >"${directory}/cargo" <<EOF
#!/usr/bin/env bash
set -Eeuo pipefail
mkdir -p -- "\${FE2O3_EVIDENCE_OUTPUT_DIR}"
printf 'cargo\\t%s\\t%s\\n' "\${CARGO_TARGET_DIR}" "\$*" \\
  >>"\${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
exit ${exit_status}
EOF
  cat >"${directory}/verus" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  cat >"${directory}/runtime-model-verus" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod 755 "${directory}/cargo" "${directory}/verus" \
    "${directory}/runtime-model-verus"
}

readonly FIXTURE_REPO="${TEST_ROOT}/repo"
readonly ATTACHED_REPO="${TEST_ROOT}/attached"
readonly PASS_BIN="${TEST_ROOT}/pass-bin"
readonly FAIL_BIN="${TEST_ROOT}/fail-bin"
readonly ARCHIVE="${TEST_ROOT}/archive"
readonly FAILURE_ARCHIVE="${TEST_ROOT}/failure-archive"
readonly COLLISION_ARCHIVE="${TEST_ROOT}/collision-archive"
readonly ATTACHED_ARCHIVE="${TEST_ROOT}/attached-archive"
readonly COMPILE_ARCHIVE="${TEST_ROOT}/compile-archive"
readonly TEST_HOME="${TEST_ROOT}/home"
readonly RECORDED_PATH="${PASS_BIN}:/usr/bin:/bin"
readonly FAIL_PATH="${FAIL_BIN}:/usr/bin:/bin"

mkdir -p -- "${FIXTURE_REPO}" "${ATTACHED_REPO}" "${ARCHIVE}" \
  "${FAILURE_ARCHIVE}" "${COLLISION_ARCHIVE}" "${ATTACHED_ARCHIVE}" \
  "${COMPILE_ARCHIVE}" \
  "${TEST_HOME}/.cargo" \
  "${TEST_HOME}/.rustup"
write_fake_repo "${FIXTURE_REPO}"
write_fake_repo "${ATTACHED_REPO}"
write_fake_tools "${PASS_BIN}" 0
write_fake_tools "${FAIL_BIN}" 41
git -C "${FIXTURE_REPO}" checkout --detach -q

OUTPUT="$(${RUNNER} list)"
assert_contains $'Q1\tcore'
assert_contains $'GFX942-HARDWARE\tunavailable'

common_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --timeout-seconds 30
)

first_plan="$(${RUNNER} dry-run "${common_args[@]}" --shard Q2 --shard Q3)"
second_plan="$(${RUNNER} dry-run "${common_args[@]}" --shard Q2 --shard Q3)"
[[ "${first_plan}" == "${second_plan}" ]] || fail 'dry-run plan is not deterministic'
OUTPUT="${first_plan}"
assert_contains $'snapshot_plan_schema_version\t1'
assert_contains $'shard\tQ2\trecords/q2.tsv\tlogs/q2.log'
assert_contains $'shard\tQ3\trecords/q3.tsv\tlogs/q3.log'
assert_contains "${ARCHIVE}/work/q2/target"
assert_contains "${ARCHIVE}/work/q3/target"
assert_contains $'environment\tQ2\tLC_ALL\t43'
assert_contains $'argv\tQ2\t0000\t'

"${RUNNER}" run "${common_args[@]}" --shard Q2 --shard Q3 >/dev/null
[[ -f "${ARCHIVE}/records/q2.tsv" ]] || fail 'Q2 result record is missing'
[[ -f "${ARCHIVE}/records/q3.tsv" ]] || fail 'Q3 result record is missing'
[[ -f "${ARCHIVE}/logs/q2.log" ]] || fail 'Q2 log is missing'
[[ -f "${ARCHIVE}/logs/q3.log" ]] || fail 'Q3 log is missing'
[[ -f "${ARCHIVE}/work/q2/output/invocations.tsv" ]] || fail 'Q2 output is missing'
[[ -f "${ARCHIVE}/work/q3/output/invocations.tsv" ]] || fail 'Q3 output is missing'
[[ "$(grep -c $'^cargo\t' "${ARCHIVE}/work/q2/output/invocations.tsv")" -eq 1 ]] ||
  fail 'Q2 must execute the fixed rustc-codegen library test command exactly once'
grep -Fq 'test -p rustc-codegen-fe2o3 --locked --lib' \
  "${ARCHIVE}/work/q2/output/invocations.tsv" ||
  fail 'Q2 did not execute the fixed rustc-codegen library test command'
grep -Fq "${ARCHIVE}/work/q2/target" \
  "${ARCHIVE}/work/q2/output/invocations.tsv" || fail 'Q2 did not use its target directory'
grep -Fq "${ARCHIVE}/work/q3/target" \
  "${ARCHIVE}/work/q3/output/invocations.tsv" || fail 'Q3 did not use its target directory'
if grep -Fq "${ARCHIVE}/work/q3/target" "${ARCHIVE}/work/q2/output/invocations.tsv"; then
  fail 'Q2 output observed Q3 target state'
fi
"${RUNNER}" verify-only "${common_args[@]}" --shard Q2 --shard Q3 >/dev/null
"${RUNNER}" run "${common_args[@]}" --shard Q1 >/dev/null
"${RUNNER}" verify-only "${common_args[@]}" --shard Q1 >/dev/null

mkdir -p -- "${COLLISION_ARCHIVE}/work/q3"
collision_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${COLLISION_ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
)
expect_failure "${RUNNER}" run "${collision_args[@]}" --shard Q2 --shard Q3
assert_contains 'shard work directory already exists: work/q3'
[[ ! -e "${COLLISION_ARCHIVE}/work/q2" ]] || fail 'Q2 started before complete preflight'

failure_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${FAILURE_ARCHIVE}"
  --path "${FAIL_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --timeout-seconds 30
)
expect_failure "${RUNNER}" run "${failure_args[@]}" --shard Q2 --shard Q3
((STATUS == 41)) || fail "failed shard status was not preserved: ${STATUS}"
assert_contains 'command failed with exit status 41'
[[ -f "${FAILURE_ARCHIVE}/records/q2.tsv" ]] || fail 'failed Q2 record was not retained'
[[ ! -e "${FAILURE_ARCHIVE}/records/q3.tsv" ]] || fail 'Q3 ran after Q2 failed'
grep -Fq $'exit_status\t41' "${FAILURE_ARCHIVE}/records/q2.tsv" ||
  fail 'failed Q2 record does not contain its exit status'

attached_args=(
  --repo "${ATTACHED_REPO}"
  --archive-root "${ATTACHED_ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --shard Q2
)
expect_failure "${RUNNER}" dry-run "${attached_args[@]}"
assert_contains 'repository must be detached'
git -C "${ATTACHED_REPO}" checkout --detach -q
printf 'dirty\n' >"${ATTACHED_REPO}/untracked"
expect_failure "${RUNNER}" dry-run "${attached_args[@]}"
assert_contains 'repository must be clean'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-hardware
assert_contains 'gfx942 hardware shard is unavailable'

compile_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${COMPILE_ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --timeout-seconds 30
  --shard Q2
  --gfx942-compile
)

first_plan="$(FE2O3_LLVM_LINK_WORKER=/ambient/worker \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID=ambient-worker-id \
  FE2O3_LLVM_BUILD_ID=ambient-llvm-id \
  FE2O3_LLVM_AS=/ambient/llvm-as \
  "${RUNNER}" dry-run "${compile_args[@]}")"
second_plan="$(${RUNNER} dry-run "${compile_args[@]}")"
[[ "${first_plan}" == "${second_plan}" ]] ||
  fail 'gfx942 compile dry-run plan depends on ambient Worker V2 values'
OUTPUT="${first_plan}"
assert_contains $'shard\tGFX942-COMPILE'
assert_contains $'environment\tGFX942-COMPILE\tFE2O3_TARGET\t'"$(hex_encode gfx942)"
[[ "${OUTPUT}" != *FE2O3_LLVM_LINK_WORKER* ]] ||
  fail 'gfx942 compile plan retained Worker V2 environment'
[[ "${OUTPUT}" != *kernel_ir_codegen* ]] ||
  fail 'gfx942 compile plan retained the retired integration target'

FE2O3_LLVM_LINK_WORKER=/ambient/worker \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID=ambient-worker-id \
  FE2O3_LLVM_BUILD_ID=ambient-llvm-id \
  FE2O3_LLVM_AS=/ambient/llvm-as \
  "${RUNNER}" run "${compile_args[@]}" >/dev/null
readonly COMPILE_INVOCATIONS="${COMPILE_ARCHIVE}/work/gfx942-compile/output/invocations.tsv"
readonly COMPILE_RECORD="${COMPILE_ARCHIVE}/records/gfx942-compile.tsv"
[[ -f "${COMPILE_INVOCATIONS}" ]] || fail 'gfx942 compile invocation output is missing'
[[ -f "${COMPILE_RECORD}" ]] || fail 'gfx942 compile result record is missing'
grep -Fq $'ci-local\trocm-compile\t' "${COMPILE_INVOCATIONS}" ||
  fail 'gfx942 compile did not invoke the production ROCm lane'
if grep -Fq ambient "${COMPILE_INVOCATIONS}" || grep -Fq ambient "${COMPILE_RECORD}"; then
  fail 'ambient Worker V2 values entered execution or its result record'
fi

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-compile --llvm-link-worker /retired/worker
assert_contains 'unknown option: --llvm-link-worker'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q7 \
  --verus "${PASS_BIN}/verus"
assert_contains 'Q7 requires --runtime-model-verus with an absolute executable path'

OUTPUT="$(${RUNNER} dry-run "${common_args[@]}" --shard Q7 \
  --runtime-model-verus "${PASS_BIN}/runtime-model-verus" \
  --verus "${PASS_BIN}/verus")"
assert_contains $'environment\tQ7\tFE2O3_RUNTIME_MODEL_VERUS\t'"$(hex_encode "${PASS_BIN}/runtime-model-verus")"
assert_contains $'environment\tQ7\tVERUS\t'"$(hex_encode "${PASS_BIN}/verus")"
assert_contains $'tool\tQ7\truntime-model-verus'
assert_contains $'tool\tQ7\tverus'

"${RUNNER}" run "${common_args[@]}" --shard Q7 \
  --runtime-model-verus "${PASS_BIN}/runtime-model-verus" \
  --verus "${PASS_BIN}/verus" >/dev/null
readonly Q7_INVOCATIONS="${ARCHIVE}/work/q7/output/invocations.tsv"
readonly Q7_RECORD="${ARCHIVE}/records/q7.tsv"
grep -Fq $'verus-environment\t'"${PASS_BIN}/runtime-model-verus"$'\t'"${PASS_BIN}/verus" \
  "${Q7_INVOCATIONS}" || fail 'Q7 did not execute with both exact Verus paths'
grep -Fq $'environment\tFE2O3_RUNTIME_MODEL_VERUS\t' \
  "${Q7_RECORD}" || fail 'Q7 record omitted the runtime-model Verus environment'
grep -Fq $'environment\tVERUS\t' \
  "${Q7_RECORD}" || fail 'Q7 record omitted the MIR/PLIRON Verus environment'
grep -Fq $'tool\truntime-model-verus\t' \
  "${Q7_RECORD}" || fail 'Q7 record omitted the runtime-model Verus tool binding'
grep -Fq $'tool\tverus\t' \
  "${Q7_RECORD}" || fail 'Q7 record omitted the MIR/PLIRON Verus tool binding'

printf 'PASS: parity snapshot orchestration is isolated, deterministic, and fail closed\n'
