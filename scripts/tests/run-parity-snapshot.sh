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
  workspace-test | verus | rocm-compile) ;;
  *) exit 90 ;;
esac
if [[ "$1" == rocm-compile ]]; then
  printf 'worker-env\t%s\t%s\t%s\t%s\n' \
    "${FE2O3_LLVM_LINK_WORKER}" \
    "${FE2O3_LLVM_LINK_WORKER_BUILD_ID}" \
    "${FE2O3_LLVM_BUILD_ID}" \
    "${FE2O3_LLVM_AS}" \
    >>"${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
fi
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
printf 'cargo-worker-env\\t%s\\t%s\\t%s\\t%s\\n' \\
  "\${FE2O3_LLVM_LINK_WORKER-<unset>}" \\
  "\${FE2O3_LLVM_LINK_WORKER_BUILD_ID-<unset>}" \\
  "\${FE2O3_LLVM_BUILD_ID-<unset>}" \\
  "\${FE2O3_LLVM_AS-<unset>}" \\
  >>"\${FE2O3_EVIDENCE_OUTPUT_DIR}/invocations.tsv"
exit ${exit_status}
EOF
  cat >"${directory}/verus" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  cat >"${directory}/llvm-link-worker" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  cat >"${directory}/llvm-as" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod 755 "${directory}/cargo" "${directory}/verus" \
    "${directory}/llvm-link-worker" "${directory}/llvm-as"
}

readonly FIXTURE_REPO="${TEST_ROOT}/repo"
readonly ATTACHED_REPO="${TEST_ROOT}/attached"
readonly PASS_BIN="${TEST_ROOT}/pass-bin"
readonly FAIL_BIN="${TEST_ROOT}/fail-bin"
readonly ARCHIVE="${TEST_ROOT}/archive"
readonly FAILURE_ARCHIVE="${TEST_ROOT}/failure-archive"
readonly COLLISION_ARCHIVE="${TEST_ROOT}/collision-archive"
readonly ATTACHED_ARCHIVE="${TEST_ROOT}/attached-archive"
readonly WORKER_ARCHIVE="${TEST_ROOT}/worker-archive"
readonly ALPHA_ZETA_ARCHIVE="${TEST_ROOT}/alpha-zeta-archive"
readonly TEST_HOME="${TEST_ROOT}/home"
readonly RECORDED_PATH="${PASS_BIN}:/usr/bin:/bin"
readonly FAIL_PATH="${FAIL_BIN}:/usr/bin:/bin"
readonly LLVM_LINK_WORKER="${PASS_BIN}/llvm-link-worker"
readonly LLVM_AS="${PASS_BIN}/llvm-as"
readonly LLVM_LINK_WORKER_BUILD_ID='fe2o3-worker-v1-sha256-0123456789abcdef'
readonly LLVM_BUILD_ID='7.2.4'
readonly ALPHA_ZETA_SHA256='3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4'

mkdir -p -- "${FIXTURE_REPO}" "${ATTACHED_REPO}" "${ARCHIVE}" \
  "${FAILURE_ARCHIVE}" "${COLLISION_ARCHIVE}" "${ATTACHED_ARCHIVE}" \
  "${WORKER_ARCHIVE}" "${ALPHA_ZETA_ARCHIVE}" \
  "${TEST_HOME}/.cargo" \
  "${TEST_HOME}/.rustup"
write_fake_repo "${FIXTURE_REPO}"
write_fake_repo "${ATTACHED_REPO}"
write_fake_tools "${PASS_BIN}" 0
write_fake_tools "${FAIL_BIN}" 41
git -C "${FIXTURE_REPO}" checkout --detach -q

OUTPUT="$(${RUNNER} list)"
assert_contains $'Q1\tcore'
assert_contains $'GFX942-HARDWARE\toptional'
assert_contains $'GFX942-ALPHA-ZETA-HARDWARE\toptional'

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

printf 'fake hsaco\n' >"${ARCHIVE}/artifacts-vecadd.hsaco"
OUTPUT="$(${RUNNER} dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-hardware --vecadd-hsaco artifacts-vecadd.hsaco)"
assert_contains $'shard\tGFX942-HARDWARE'
assert_contains 'FE2O3_GFX942_VECADD_HSACO'

printf 'fake alpha zeta hsaco\n' >"${ALPHA_ZETA_ARCHIVE}/artifacts-alpha-zeta.hsaco"
alpha_zeta_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${ALPHA_ZETA_ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --timeout-seconds 30
  --shard Q2
  --gfx942-alpha-zeta-hardware
  --alpha-zeta-hsaco artifacts-alpha-zeta.hsaco
  --alpha-zeta-sha256 "${ALPHA_ZETA_SHA256}"
)
OUTPUT="$(${RUNNER} dry-run "${alpha_zeta_args[@]}")"
assert_contains $'shard\tGFX942-ALPHA-ZETA-HARDWARE'
assert_contains 'FE2O3_RUN_GFX942_TWO_KERNEL'
assert_contains 'FE2O3_GFX942_ALPHA_ZETA_HSACO'
assert_contains 'FE2O3_GFX942_ALPHA_ZETA_SHA256'
assert_contains "$(hex_encode gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator)"
"${RUNNER}" run "${alpha_zeta_args[@]}" >/dev/null
readonly ALPHA_ZETA_RECORD="${ALPHA_ZETA_ARCHIVE}/records/gfx942-alpha-zeta-hardware.tsv"
readonly ALPHA_ZETA_INVOCATIONS="${ALPHA_ZETA_ARCHIVE}/work/gfx942-alpha-zeta-hardware/output/invocations.tsv"
grep -Fq $'environment\tFE2O3_RUN_GFX942_TWO_KERNEL\t31' "${ALPHA_ZETA_RECORD}" ||
  fail 'alpha/zeta record omitted its explicit opt-in'
grep -Fq $'environment\tFE2O3_GFX942_ALPHA_ZETA_SHA256\t'"$(hex_encode "${ALPHA_ZETA_SHA256}")" \
  "${ALPHA_ZETA_RECORD}" || fail 'alpha/zeta record omitted the expected digest'
grep -Fq $'artifact\talpha-zeta-hsaco\tartifacts-alpha-zeta.hsaco\t' \
  "${ALPHA_ZETA_RECORD}" || fail 'alpha/zeta record omitted the pinned artifact'
grep -Fq 'gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator' \
  "${ALPHA_ZETA_INVOCATIONS}" || fail 'alpha/zeta shard invoked the wrong test'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-alpha-zeta-hardware \
  --alpha-zeta-hsaco artifacts-alpha-zeta.hsaco
assert_contains 'requires a lowercase --alpha-zeta-sha256 digest'
expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-alpha-zeta-hardware \
  --alpha-zeta-hsaco artifacts-alpha-zeta.hsaco \
  --alpha-zeta-sha256 "${ALPHA_ZETA_SHA256^^}"
assert_contains 'requires a lowercase --alpha-zeta-sha256 digest'

worker_args=(
  --repo "${FIXTURE_REPO}"
  --archive-root "${WORKER_ARCHIVE}"
  --path "${RECORDED_PATH}"
  --home "${TEST_HOME}"
  --cargo-home "${TEST_HOME}/.cargo"
  --rustup-home "${TEST_HOME}/.rustup"
  --timeout-seconds 30
  --shard Q2
  --gfx942-compile
  --llvm-link-worker "${LLVM_LINK_WORKER}"
  --llvm-link-worker-build-id "${LLVM_LINK_WORKER_BUILD_ID}"
  --llvm-build-id "${LLVM_BUILD_ID}"
  --llvm-as "${LLVM_AS}"
)

first_plan="$(FE2O3_LLVM_LINK_WORKER=/ambient/worker \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID=ambient-worker-id \
  FE2O3_LLVM_BUILD_ID=ambient-llvm-id \
  FE2O3_LLVM_AS=/ambient/llvm-as \
  "${RUNNER}" dry-run "${worker_args[@]}")"
second_plan="$(${RUNNER} dry-run "${worker_args[@]}")"
[[ "${first_plan}" == "${second_plan}" ]] ||
  fail 'gfx942 compile dry-run plan depends on ambient Worker V2 values'
OUTPUT="${first_plan}"
assert_contains $'shard\tGFX942-COMPILE'
assert_contains $'environment\tGFX942-COMPILE\tFE2O3_LLVM_LINK_WORKER\t'"$(hex_encode "${LLVM_LINK_WORKER}")"
assert_contains $'environment\tGFX942-COMPILE\tFE2O3_LLVM_LINK_WORKER_BUILD_ID\t'"$(hex_encode "${LLVM_LINK_WORKER_BUILD_ID}")"
assert_contains $'environment\tGFX942-COMPILE\tFE2O3_LLVM_BUILD_ID\t'"$(hex_encode "${LLVM_BUILD_ID}")"
assert_contains $'environment\tGFX942-COMPILE\tFE2O3_LLVM_AS\t'"$(hex_encode "${LLVM_AS}")"
assert_contains $'tool\tGFX942-COMPILE\tllvm-link-worker\t'"${LLVM_LINK_WORKER}"
assert_contains $'tool\tGFX942-COMPILE\tllvm-as\t'"${LLVM_AS}"

FE2O3_LLVM_LINK_WORKER=/ambient/worker \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID=ambient-worker-id \
  FE2O3_LLVM_BUILD_ID=ambient-llvm-id \
  FE2O3_LLVM_AS=/ambient/llvm-as \
  "${RUNNER}" run "${worker_args[@]}" >/dev/null
readonly WORKER_INVOCATIONS="${WORKER_ARCHIVE}/work/gfx942-compile/output/invocations.tsv"
readonly WORKER_RECORD="${WORKER_ARCHIVE}/records/gfx942-compile.tsv"
[[ -f "${WORKER_INVOCATIONS}" ]] || fail 'gfx942 compile invocation output is missing'
[[ -f "${WORKER_RECORD}" ]] || fail 'gfx942 compile result record is missing'
expected_worker_env="${LLVM_LINK_WORKER}"$'\t'"${LLVM_LINK_WORKER_BUILD_ID}"$'\t'"${LLVM_BUILD_ID}"$'\t'"${LLVM_AS}"
grep -Fq $'worker-env\t'"${expected_worker_env}" "${WORKER_INVOCATIONS}" ||
  fail 'rocm compile did not receive the exact pinned Worker V2 environment'
grep -Fq $'cargo-worker-env\t'"${expected_worker_env}" "${WORKER_INVOCATIONS}" ||
  fail 'Worker V2 rustc test did not receive the exact pinned environment'
grep -Fq $'environment\tFE2O3_LLVM_LINK_WORKER\t'"$(hex_encode "${LLVM_LINK_WORKER}")" "${WORKER_RECORD}" ||
  fail 'result record omitted the Worker V2 executable environment'
grep -Fq $'environment\tFE2O3_LLVM_LINK_WORKER_BUILD_ID\t'"$(hex_encode "${LLVM_LINK_WORKER_BUILD_ID}")" "${WORKER_RECORD}" ||
  fail 'result record omitted the Worker V2 build ID'
grep -Fq $'environment\tFE2O3_LLVM_BUILD_ID\t'"$(hex_encode "${LLVM_BUILD_ID}")" "${WORKER_RECORD}" ||
  fail 'result record omitted the LLVM build ID'
grep -Fq $'environment\tFE2O3_LLVM_AS\t'"$(hex_encode "${LLVM_AS}")" "${WORKER_RECORD}" ||
  fail 'result record omitted the llvm-as environment'
grep -Fq $'tool\tllvm-link-worker\t'"${LLVM_LINK_WORKER}"$'\t' "${WORKER_RECORD}" ||
  fail 'result record omitted the Worker V2 executable identity'
grep -Fq $'tool\tllvm-as\t'"${LLVM_AS}"$'\t' "${WORKER_RECORD}" ||
  fail 'result record omitted the llvm-as executable identity'
if grep -Fq ambient "${WORKER_INVOCATIONS}" || grep -Fq ambient "${WORKER_RECORD}"; then
  fail 'ambient Worker V2 values entered execution or its result record'
fi

expect_failure env \
  FE2O3_LLVM_LINK_WORKER=/ambient/worker \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID=ambient-worker-id \
  FE2O3_LLVM_BUILD_ID=ambient-llvm-id \
  FE2O3_LLVM_AS=/ambient/llvm-as \
  "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 --gfx942-compile
assert_contains 'requires --llvm-link-worker'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-compile \
  --llvm-link-worker "${LLVM_LINK_WORKER}" \
  --llvm-link-worker-build-id "${LLVM_LINK_WORKER_BUILD_ID}" \
  --llvm-build-id "${LLVM_BUILD_ID}"
assert_contains 'requires --llvm-as'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-compile \
  --llvm-link-worker relative/worker \
  --llvm-link-worker-build-id "${LLVM_LINK_WORKER_BUILD_ID}" \
  --llvm-build-id "${LLVM_BUILD_ID}" \
  --llvm-as "${LLVM_AS}"
assert_contains 'requires --llvm-link-worker with an absolute executable path'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-compile \
  --llvm-link-worker "${PASS_BIN}/llvm-link-worker;literal" \
  --llvm-link-worker-build-id "${LLVM_LINK_WORKER_BUILD_ID}" \
  --llvm-build-id "${LLVM_BUILD_ID}" \
  --llvm-as "${LLVM_AS}"
assert_contains 'requires --llvm-link-worker with an absolute executable path'

expect_failure "${RUNNER}" dry-run "${common_args[@]}" --shard Q2 \
  --gfx942-compile \
  --llvm-link-worker "${LLVM_LINK_WORKER}" \
  --llvm-link-worker-build-id $'bad\tid' \
  --llvm-build-id "${LLVM_BUILD_ID}" \
  --llvm-as "${LLVM_AS}"
assert_contains 'bounded safe --llvm-link-worker-build-id'

OUTPUT="$(${RUNNER} dry-run "${common_args[@]}" --shard Q7 \
  --verus "${PASS_BIN}/verus")"
assert_contains $'tool\tQ7\tverus'

printf 'PASS: parity snapshot orchestration is isolated, deterministic, and fail closed\n'
