#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly QUEUE="${ROOT}/scripts/mi300x-evidence-queue.sh"
readonly TOOL="${ROOT}/scripts/parity-row-evidence.sh"
readonly PRIVATE_KEY="${TEST_DIR}/fixtures/evidence-test-attestor-private.pem"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT
readonly REPO="${TEST_ROOT}/repo"
readonly TRUSTED="${TEST_ROOT}/trusted"
readonly LOCK_ROOT="${TEST_ROOT}/lock"

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'negative queue test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'wrong queue diagnostic for %s\n' "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    return 1
  }
}

file_sha() {
  sha256sum -- "$1" | awk '{ print $1 }'
}

file_size() {
  stat -c %s -- "$1"
}

hex_text() {
  printf '%s' "$1" | od -An -tx1 | tr -d ' \n'
}

sign_queue() {
  local archive="$1"
  local unsigned="$2"
  local output="$3"
  "${TOOL}" sign --repo "${REPO}" --private-key "${PRIVATE_KEY}" --key-id test-attestor --domain test --role attestor --test-mode "${archive}/${unsigned}" "${archive}/${output}"
}

prepare_archive() {
  local archive="$1"
  local suffix="$2"
  local source="${3:-${SOURCE}}"
  local target="${4:-gfx942}"
  local job_count="${5:-1}"
  local bash_executor
  local result_id
  local script_digest
  local sleep_executor
  local timeout_executor
  local unsigned=queues/queue.unsigned.tsv
  result_id="$(printf '%s' "hardware-${suffix}" | sha256sum | awk '{ print $1 }')"
  script_digest="$(file_sha "${REPO}/scripts/hardware-queue-fixture.sh")"
  mkdir -p "${archive}/queues" "${archive}/toolchains"
  bash_executor="$(readlink -f "$(command -v bash)")"
  sleep_executor="$(readlink -f "$(command -v sleep)")"
  timeout_executor="$(readlink -f "$(command -v timeout)")"
  {
    printf 'toolchain_closure_schema_version\t1\n'
    printf 'executable\tbash\t%s\t%s\n' "$(command -v bash)" "$(file_sha "$(command -v bash)")"
    printf 'version\t%s\n' "$(bash --version | head -1)"
    printf 'executable\ttimeout\t%s\t%s\n' "$(command -v timeout)" "$(file_sha "$(command -v timeout)")"
    printf 'timeout_version\t%s\n' "$(timeout --version | head -1)"
  } >"${archive}/toolchains/bash.tsv"
  {
    printf 'signed_queue_schema_version\t3\n'
    printf 'queue_id\t%s\n' "$(printf '%s' "queue-${suffix}" | sha256sum | awk '{ print $1 }')"
    printf 'baseline_commit\t%s\n' "${BASELINE}"
    printf 'source_commit\t%s\n' "${source}"
    printf 'source_tree\t%s\n' "${SOURCE_TREE}"
    printf 'target\t%s\n' "${target}"
    printf 'hardware_lane\tmi300x-gfx942-test\n'
    printf 'execution_mode\ttest\n'
    printf 'execution_closure\tinert\n'
    printf 'archive_root\t%s\n' "$(realpath -e "${archive}")"
    printf 'executor_count\t3\n'
    printf 'executor\t0000\tbash\t%s\t%s\t%s\n' "${bash_executor}" "$(file_size "${bash_executor}")" "$(file_sha "${bash_executor}")"
    printf 'executor\t0001\tsleep\t%s\t%s\t%s\n' "${sleep_executor}" "$(file_size "${sleep_executor}")" "$(file_sha "${sleep_executor}")"
    printf 'executor\t0002\ttimeout\t%s\t%s\t%s\n' "${timeout_executor}" "$(file_size "${timeout_executor}")" "$(file_sha "${timeout_executor}")"
    printf 'environment_count\t3\n'
    printf 'environment\t0000\tHOME\t%s\n' "$(hex_text /nonexistent)"
    printf 'environment\t0001\tLC_ALL\t%s\n' "$(hex_text C)"
    printf 'environment\t0002\tPATH\t%s\n' "$(hex_text /nonexistent)"
    printf 'toolchain_count\t1\n'
    printf 'toolchain\t0000\tbash\ttoolchains/bash.tsv\t%s\t%s\n' "$(file_size "${archive}/toolchains/bash.tsv")" "$(file_sha "${archive}/toolchains/bash.tsv")"
    printf 'job_count\t%s\n' "${job_count}"
    if [[ "${job_count}" == 1 ]]; then
      printf 'job\t0000\thardware-%s\t%s\t04\tMissing\tPartial\t30\t%s\t%s\t%s\t%s\t%s\thardware\n' "${suffix}" "${result_id}" scripts/hardware-queue-fixture.sh "${script_digest}" results/hardware.tsv logs/hardware.log binary=artifacts/hardware.bin
    elif [[ "${job_count}" == 2 ]]; then
      printf 'job\t0000\thardware-%s-one\t%s\t04\tMissing\tPartial\t30\t%s\t%s\t%s\t%s\t%s\thardware\n' "${suffix}" "$(printf '%s' "hardware-${suffix}-one" | sha256sum | awk '{ print $1 }')" scripts/hardware-queue-fixture.sh "${script_digest}" results/hardware-one.tsv logs/hardware-one.log binary=artifacts/hardware-one.bin
      printf 'job\t0001\thardware-%s-two\t%s\t05\tMissing\tPartial\t30\t%s\t%s\t%s\t%s\t%s\thardware\n' "${suffix}" "$(printf '%s' "hardware-${suffix}-two" | sha256sum | awk '{ print $1 }')" scripts/hardware-queue-fixture.sh "${script_digest}" results/hardware-two.tsv logs/hardware-two.log binary=artifacts/hardware-two.bin
    else
      return 2
    fi
  } >"${archive}/${unsigned}"
  sign_queue "${archive}" "${unsigned}" queues/queue.tsv
  rm "${archive}/${unsigned}"
}

common_args() {
  local archive="$1"
  printf '%s\n' --repo "${REPO}" --archive-root "${archive}" --trusted-root "${TRUSTED}" --trust-policy "${TRUSTED}/trust.tsv"
}

run_queue() {
  local archive="$1"
  local -a args=()
  mapfile -t args < <(common_args "${archive}")
  "${QUEUE}" run "${args[@]}" --manifest queues/queue.tsv --signing-key "${PRIVATE_KEY}" --key-id test-attestor --test-mode --lock-root "${LOCK_ROOT}"
}

mutate_hardware_result() {
  local name="$1"
  local old="$2"
  local replacement="$3"
  local expected="${4:-hardware result does not match signed queue job}"
  local archive="${ARCHIVE}"
  local backup="${TEST_ROOT}/${name}-hardware.tsv"
  local unsigned=work/mutated-result.tsv
  local -a args=()
  cp "${archive}/results/hardware.tsv" "${backup}"
  mkdir -p "${archive}/work"
  head -n -6 "${archive}/results/hardware.tsv" >"${archive}/${unsigned}"
  sed -i "s|${old}|${replacement}|" "${archive}/${unsigned}"
  case "${name}" in
    log_path)
      cp "${archive}/logs/hardware.log" "${archive}/logs/alternate.log"
      ;;
    artifact_path)
      cp "${archive}/artifacts/hardware.bin" "${archive}/artifacts/alternate.bin"
      ;;
  esac
  rm "${archive}/results/hardware.tsv"
  sign_queue "${archive}" "${unsigned}" results/hardware.tsv
  mapfile -t args < <(common_args "${archive}")
  expect_failure "${name}" "${expected}" \
    "${TOOL}" validate-result "${args[@]}" results/hardware.tsv
  mv "${backup}" "${archive}/results/hardware.tsv"
  rm -f "${archive}/work/mutated-result.tsv" \
    "${archive}/logs/alternate.log" "${archive}/artifacts/alternate.bin"
}


git init -q "${REPO}"
git -C "${REPO}" config user.email queue@example.invalid
git -C "${REPO}" config user.name 'Queue Fixture'
printf 'baseline\n' >"${REPO}/README"
git -C "${REPO}" add README
git -C "${REPO}" commit -qm baseline
BASELINE="$(git -C "${REPO}" rev-parse HEAD)"
readonly BASELINE
mkdir -p "${REPO}/scripts"
cat >"${REPO}/scripts/hardware-queue-fixture.sh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
"${FE2O3_EVIDENCE_SLEEP}" 0.25
artifact="${FE2O3_EVIDENCE_ARTIFACTS#*=}"
{
  printf 'test-only serialized hardware fixture\n'
  printf 'row=%s\n' "${FE2O3_EVIDENCE_ROW}"
  printf 'target=%s\n' "${FE2O3_EVIDENCE_TARGET}"
  printf 'lane=%s\n' "${FE2O3_EVIDENCE_HARDWARE_LANE}"
  printf 'home=%s\n' "${HOME}"
  printf 'path=%s\n' "${PATH}"
} >"${FE2O3_EVIDENCE_ARCHIVE_ROOT}/${artifact}"
printf 'hardware queue fixture executed\n'
EOF
chmod 755 "${REPO}/scripts/hardware-queue-fixture.sh"
git -C "${REPO}" add scripts/hardware-queue-fixture.sh
git -C "${REPO}" commit -qm source
SOURCE="$(git -C "${REPO}" rev-parse HEAD)"
readonly SOURCE
SOURCE_TREE="$(git -C "${REPO}" rev-parse 'HEAD^{tree}')"
readonly SOURCE_TREE
git -C "${REPO}" checkout -q --detach

mkdir -p "${TRUSTED}/keys" "${LOCK_ROOT}"
chmod 700 "${LOCK_ROOT}"
: >"${LOCK_ROOT}/mi300x-gfx942-evidence.lock"
chmod 600 "${LOCK_ROOT}/mi300x-gfx942-evidence.lock"
cp "${TEST_DIR}/fixtures/evidence-test-attestor-public.pem" "${TRUSTED}/keys/attestor.pem"
{
  printf 'parity_trust_policy_schema_version\t2\n'
  printf 'trust_domain\ttest\n'
  printf 'metadata_path_count\t0\n'
  printf 'key_count\t1\n'
  printf 'key\t0000\tattestor\ttest-attestor\tkeys/attestor.pem\t%s\ted25519\n' "$(file_sha "${TRUSTED}/keys/attestor.pem")"
} >"${TRUSTED}/trust.tsv"

ARCHIVE="${TEST_ROOT}/archive"
readonly ARCHIVE
mkdir -p "${ARCHIVE}"
prepare_archive "${ARCHIVE}" primary
mapfile -t validate_args < <(common_args "${ARCHIVE}")
"${QUEUE}" validate "${validate_args[@]}" queues/queue.tsv
run_queue "${ARCHIVE}"
"${TOOL}" validate-result "${validate_args[@]}" results/hardware.tsv
grep -F 'test-only serialized hardware fixture' "${ARCHIVE}/artifacts/hardware.bin" >/dev/null
grep -F $'queue_manifest_path\tqueues/queue.tsv' "${ARCHIVE}/results/hardware.tsv" >/dev/null
queue_id="$(awk -F '\t' '$1 == "queue_id" { print $2 }' "${ARCHIVE}/queues/queue.tsv")"
alternate_queue_id="$(printf '%s' alternate-queue | sha256sum | awk '{ print $1 }')"
bash_executor="$(readlink -f "$(command -v bash)")"
timeout_executor="$(readlink -f "$(command -v timeout)")"
grep -F $'execution_closure\tinert' "${ARCHIVE}/results/hardware.tsv" >/dev/null
grep -F 'home=/nonexistent' "${ARCHIVE}/artifacts/hardware.bin" >/dev/null
grep -F 'path=/nonexistent' "${ARCHIVE}/artifacts/hardware.bin" >/dev/null

expected_command="$(hex_text "${timeout_executor} --signal=TERM --kill-after=5s 30 ${bash_executor} scripts/hardware-queue-fixture.sh")"
alternate_command="$(hex_text "${timeout_executor} --signal=TERM --kill-after=5s 31 ${bash_executor} scripts/hardware-queue-fixture.sh")"
grep -F $'queue_id\t'"${queue_id}" "${ARCHIVE}/results/hardware.tsv" >/dev/null
grep -F $'timeout_seconds\t30' "${ARCHIVE}/results/hardware.tsv" >/dev/null
grep -F $'command\t0000\t'"${expected_command}"$'\t0' "${ARCHIVE}/results/hardware.tsv" >/dev/null

mutate_hardware_result closure $'execution_closure\tinert' \
  $'execution_closure\tverified' 'hardware result has no signed queue binding'
mutate_hardware_result environment \
  $'environment\t0008\tPATH\t2f6e6f6e6578697374656e74' \
  $'environment\t0008\tPATH\t2f746d70'
mutate_hardware_result executor_label $'executor\t0000\tbash\t' \
  $'executor\t0000\tshell\t'
# A shell queue result is explicitly inert and cannot promote a parity row.
{
  printf 'schema_version\t1\n'
  printf 'fe2o3_commit\t%s\n' "${BASELINE}"
  printf 'kind\tid\tstatus\n'
  printf 'normative\t04\tMissing\n'
} >"${TEST_ROOT}/baseline-status.tsv"
{
  printf 'schema_version\t1\n'
  printf 'fe2o3_commit\t%s\n' "${SOURCE}"
  printf 'kind\tid\tstatus\n'
  printf 'normative\t04\tPartial\n'
} >"${TEST_ROOT}/candidate-status.tsv"
{
  printf 'row_evidence_policy_schema_version\t2\n'
  printf 'row_count\t1\n'
  printf 'row\t0000\t04\tgfx942\thardware\thardware,debug\treviewer\n'
} >"${TEST_ROOT}/inert-policy.tsv"
result_id="$(awk -F '\t' '$1 == "result_id" { print $2 }' "${ARCHIVE}/results/hardware.tsv")"
{
  printf 'promotion_manifest_schema_version\t2\n'
  printf 'baseline_commit\t%s\n' "${BASELINE}"
  printf 'source_commit\t%s\n' "${SOURCE}"
  printf 'source_tree\t%s\n' "${SOURCE_TREE}"
  printf 'target\tgfx942\n'
  printf 'hardware_lane\tmi300x-gfx942-test\n'
  printf 'result_count\t1\n'
  printf 'result\t0000\t04\tMissing\tPartial\thardware\tresults/hardware.tsv\t%s\t%s\n' "$(file_sha "${ARCHIVE}/results/hardware.tsv")" "${result_id}"
} >"${ARCHIVE}/promotion-inert.tsv"
printf 'evidence_set_sha256\t%s\n' "$(file_sha "${ARCHIVE}/promotion-inert.tsv")" >>"${ARCHIVE}/promotion-inert.tsv"
printf 'authorization_count\t0\n' >>"${ARCHIVE}/promotion-inert.tsv"
expect_failure inert_hardware_promotion 'hardware result execution closure is inert and cannot promote parity' \
  "${TOOL}" gate "${validate_args[@]}" --manifest promotion-inert.tsv \
  --trusted-policy "${TEST_ROOT}/inert-policy.tsv" \
  --candidate-policy "${TEST_ROOT}/inert-policy.tsv" \
  --baseline-status "${TEST_ROOT}/baseline-status.tsv" \
  --candidate-status "${TEST_ROOT}/candidate-status.tsv" --allow-test-fixtures

# A referenced signed queue is an exact unit: every declared job must have one
# manifest hardware result. Omitting the second job fails before archive copy.
MULTI_ARCHIVE="${TEST_ROOT}/multi-job-archive"
mkdir -p "${MULTI_ARCHIVE}"
prepare_archive "${MULTI_ARCHIVE}" multi "${SOURCE}" gfx942 2
run_queue "${MULTI_ARCHIVE}"
rm -rf "${MULTI_ARCHIVE}/work"
multi_one_id="$(awk -F '\t' '$1 == "result_id" { print $2 }' \
  "${MULTI_ARCHIVE}/results/hardware-one.tsv")"
multi_two_id="$(awk -F '\t' '$1 == "result_id" { print $2 }' \
  "${MULTI_ARCHIVE}/results/hardware-two.tsv")"
{
  printf 'promotion_manifest_schema_version\t2\n'
  printf 'baseline_commit\t%s\n' "${BASELINE}"
  printf 'source_commit\t%s\n' "${SOURCE}"
  printf 'source_tree\t%s\n' "${SOURCE_TREE}"
  printf 'target\tgfx942\n'
  printf 'hardware_lane\tmi300x-gfx942-test\n'
  printf 'result_count\t1\n'
  printf 'result\t0000\t04\tMissing\tPartial\thardware\tresults/hardware-one.tsv\t%s\t%s\n' \
    "$(file_sha "${MULTI_ARCHIVE}/results/hardware-one.tsv")" "${multi_one_id}"
} >"${MULTI_ARCHIVE}/promotion.tsv"
printf 'evidence_set_sha256\t%s\n' "$(file_sha "${MULTI_ARCHIVE}/promotion.tsv")" \
  >>"${MULTI_ARCHIVE}/promotion.tsv"
printf 'authorization_count\t0\n' >>"${MULTI_ARCHIVE}/promotion.tsv"
expect_failure omitted_second_queue_job 'referenced queue job/result set is not exact' \
  "${TOOL}" ingest-archive \
  --repo "${REPO}" \
  --source-root "${MULTI_ARCHIVE}" \
  --destination-root "${TEST_ROOT}/multi-job-omitted-output" \
  --trusted-root "${TRUSTED}" \
  --trust-policy "${TRUSTED}/trust.tsv" \
  --manifest promotion.tsv \
  --expected-manifest-sha256 "$(file_sha "${MULTI_ARCHIVE}/promotion.tsv")" \
  --expected-baseline "${BASELINE}" \
  --expected-source "${SOURCE}" \
  --expected-tree "${SOURCE_TREE}" \
  --expected-target gfx942 \
  --expected-lane mi300x-gfx942-test \
  --allow-test-fixtures

{
  printf 'promotion_manifest_schema_version\t2\n'
  printf 'baseline_commit\t%s\n' "${BASELINE}"
  printf 'source_commit\t%s\n' "${SOURCE}"
  printf 'source_tree\t%s\n' "${SOURCE_TREE}"
  printf 'target\tgfx942\n'
  printf 'hardware_lane\tmi300x-gfx942-test\n'
  printf 'result_count\t2\n'
  printf 'result\t0000\t04\tMissing\tPartial\thardware\tresults/hardware-one.tsv\t%s\t%s\n' \
    "$(file_sha "${MULTI_ARCHIVE}/results/hardware-one.tsv")" "${multi_one_id}"
  printf 'result\t0001\t05\tMissing\tPartial\thardware\tresults/hardware-two.tsv\t%s\t%s\n' \
    "$(file_sha "${MULTI_ARCHIVE}/results/hardware-two.tsv")" "${multi_two_id}"
} >"${MULTI_ARCHIVE}/promotion.tsv"
printf 'evidence_set_sha256\t%s\n' "$(file_sha "${MULTI_ARCHIVE}/promotion.tsv")" \
  >>"${MULTI_ARCHIVE}/promotion.tsv"
printf 'authorization_count\t0\n' >>"${MULTI_ARCHIVE}/promotion.tsv"
"${TOOL}" ingest-archive \
  --repo "${REPO}" \
  --source-root "${MULTI_ARCHIVE}" \
  --destination-root "${TEST_ROOT}/multi-job-complete-output" \
  --trusted-root "${TRUSTED}" \
  --trust-policy "${TRUSTED}/trust.tsv" \
  --manifest promotion.tsv \
  --expected-manifest-sha256 "$(file_sha "${MULTI_ARCHIVE}/promotion.tsv")" \
  --expected-baseline "${BASELINE}" \
  --expected-source "${SOURCE}" \
  --expected-tree "${SOURCE_TREE}" \
  --expected-target gfx942 \
  --expected-lane mi300x-gfx942-test \
  --allow-test-fixtures
chmod -R u+w "${TEST_ROOT}/multi-job-complete-output"

# Every queue-declared execution field is checked after signature verification.
mutate_hardware_result queue_identity $'queue_id\t'"${queue_id}" $'queue_id\t'"${alternate_queue_id}"
mutate_hardware_result timeout $'timeout_seconds\t30' $'timeout_seconds\t31'
mutate_hardware_result toolchain $'toolchain\t0000\tbash\t' $'toolchain\t0000\tshell\t'
mutate_hardware_result command \
  $'command\t0000\t'"${expected_command}"$'\t0' \
  $'command\t0000\t'"${alternate_command}"$'\t0'
mutate_hardware_result log_path \
  $'log\t0000\tlogs/hardware.log\t' $'log\t0000\tlogs/alternate.log\t'
mutate_hardware_result artifact_label \
  $'artifact\t0000\tbinary\t' $'artifact\t0000\tobject\t'
mutate_hardware_result artifact_path \
  $'\tartifacts/hardware.bin\t' $'\tartifacts/alternate.bin\t'


# The canonical lock serializes concurrent invocations sharing the host lock.
ARCHIVE_A="${TEST_ROOT}/archive-a"
ARCHIVE_B="${TEST_ROOT}/archive-b"
mkdir -p "${ARCHIVE_A}" "${ARCHIVE_B}"
prepare_archive "${ARCHIVE_A}" concurrent-a
prepare_archive "${ARCHIVE_B}" concurrent-b
start_ns="$(date +%s%N)"
run_queue "${ARCHIVE_A}" >"${TEST_ROOT}/concurrent-a.out" 2>&1 &
pid_a=$!
run_queue "${ARCHIVE_B}" >"${TEST_ROOT}/concurrent-b.out" 2>&1 &
pid_b=$!
wait "${pid_a}"
wait "${pid_b}"
elapsed_ms="$((( $(date +%s%N) - start_ns ) / 1000000))"
((elapsed_ms >= 400)) || {
  printf 'MI300X queue executions overlapped: %sms\n' "${elapsed_ms}" >&2
  exit 1
}
[[ -f "${ARCHIVE_A}/results/hardware.tsv" ]]
[[ -f "${ARCHIVE_B}/results/hardware.tsv" ]]

# A modified queue signature is rejected.
cp "${ARCHIVE}/queues/queue.tsv" "${ARCHIVE}/queues/signature-mutated.tsv"
signature_before="$(awk -F '\t' '$1 == "signature_base64" { print $2 }' "${ARCHIVE}/queues/signature-mutated.tsv")"
awk -F '\t' 'BEGIN { OFS = FS } $1 == "signature_base64" {
  replacement = substr($2, 1, 1) == "A" ? "B" : "A"
  $2 = replacement substr($2, 2)
} { print }' "${ARCHIVE}/queues/signature-mutated.tsv" >"${ARCHIVE}/queues/signature-mutated.tmp"
mv "${ARCHIVE}/queues/signature-mutated.tmp" "${ARCHIVE}/queues/signature-mutated.tsv"
[[ "$(awk -F '\t' '$1 == "signature_base64" { print $2 }' "${ARCHIVE}/queues/signature-mutated.tsv")" != "${signature_before}" ]]
expect_failure signature_mutation 'signature verification failed' "${QUEUE}" validate "${validate_args[@]}" queues/signature-mutated.tsv

# Signed launcher digest and environment substitutions still fail.
head -n -6 "${ARCHIVE}/queues/queue.tsv" |
  awk -F '\t' -v OFS='\t' '$1 == "executor" && $3 == "bash" { $6 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" } { print }' \
    >"${ARCHIVE}/queues/executor-digest.unsigned.tsv"
sign_queue "${ARCHIVE}" queues/executor-digest.unsigned.tsv queues/executor-digest.tsv
expect_failure executor_digest 'bound executor digest or filesystem policy mismatch' \
  "${QUEUE}" validate "${validate_args[@]}" queues/executor-digest.tsv

head -n -6 "${ARCHIVE}/queues/queue.tsv" |
  awk -F '\t' -v OFS='\t' '$1 == "environment" && $3 == "PATH" { $4 = "2f746d70" } { print }' \
    >"${ARCHIVE}/queues/environment.unsigned.tsv"
sign_queue "${ARCHIVE}" queues/environment.unsigned.tsv queues/environment.tsv
expect_failure queue_environment 'signed queue environment is not the constrained baseline' \
  "${QUEUE}" validate "${validate_args[@]}" queues/environment.tsv

# Signed but stale or target-mismatched queues fail their semantic bindings.
STALE_ARCHIVE="${TEST_ROOT}/stale"
mkdir -p "${STALE_ARCHIVE}"
prepare_archive "${STALE_ARCHIVE}" stale aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
mapfile -t stale_args < <(common_args "${STALE_ARCHIVE}")
expect_failure stale_source 'stale queue commit' "${QUEUE}" validate "${stale_args[@]}" queues/queue.tsv
TARGET_ARCHIVE="${TEST_ROOT}/target"
mkdir -p "${TARGET_ARCHIVE}"
prepare_archive "${TARGET_ARCHIVE}" target "${SOURCE}" gfx950
mapfile -t target_args < <(common_args "${TARGET_ARCHIVE}")
expect_failure target_mismatch 'MI300X queue target/lane mismatch' "${QUEUE}" validate "${target_args[@]}" queues/queue.tsv

# Every queue script path component must be a real checkout directory or file.
mv "${REPO}/scripts" "${REPO}/scripts-real"
ln -s scripts-real "${REPO}/scripts"
expect_failure script_parent_symlink 'queue script path contains symlink' \
  "${QUEUE}" validate "${validate_args[@]}" queues/queue.tsv
rm "${REPO}/scripts"
mv "${REPO}/scripts-real" "${REPO}/scripts"
mv "${REPO}/scripts/hardware-queue-fixture.sh" "${REPO}/scripts/fixture-real.sh"
ln -s fixture-real.sh "${REPO}/scripts/hardware-queue-fixture.sh"
expect_failure script_file_symlink 'queue script path contains symlink' \
  "${QUEUE}" validate "${validate_args[@]}" queues/queue.tsv
rm "${REPO}/scripts/hardware-queue-fixture.sh"
mv "${REPO}/scripts/fixture-real.sh" "${REPO}/scripts/hardware-queue-fixture.sh"

cp "${REPO}/scripts/hardware-queue-fixture.sh" "${TEST_ROOT}/fixture.backup"
printf 'modified checkout\n' >>"${REPO}/scripts/hardware-queue-fixture.sh"
expect_failure script_checkout_mismatch \
  'queue checkout script differs from attested source' \
  "${QUEUE}" validate "${validate_args[@]}" queues/queue.tsv
mv "${TEST_ROOT}/fixture.backup" "${REPO}/scripts/hardware-queue-fixture.sh"


# Lock selection and filesystem surprises fail before queue preflight.
expect_failure alternate_lock 'alternate lock roots require explicit test mode' "${QUEUE}" run "${validate_args[@]}" --manifest queues/missing.tsv --signing-key "${PRIVATE_KEY}" --key-id test-attestor --lock-root "${LOCK_ROOT}"
expect_failure caller_lock_file 'unrecognized arguments: --lock-file' "${QUEUE}" run "${validate_args[@]}" --manifest queues/queue.tsv --signing-key "${PRIVATE_KEY}" --key-id test-attestor --test-mode --lock-root "${LOCK_ROOT}" --lock-file "${TEST_ROOT}/other"

ln "${LOCK_ROOT}/mi300x-gfx942-evidence.lock" "${TEST_ROOT}/hardlink"
expect_failure lock_hardlink 'unsafe canonical MI300X lock ownership, mode, or link count' run_queue "${ARCHIVE_A}"
rm "${TEST_ROOT}/hardlink"
rm "${LOCK_ROOT}/mi300x-gfx942-evidence.lock"
ln -s "${TEST_ROOT}/not-a-lock" "${LOCK_ROOT}/mi300x-gfx942-evidence.lock"
expect_failure lock_symlink 'unsafe canonical MI300X lock ownership, mode, or link count' run_queue "${ARCHIVE_A}"

bash -n "${QUEUE}" "${BASH_SOURCE[0]}"
shellcheck "${QUEUE}" "${BASH_SOURCE[0]}"
python3 -m py_compile "${ROOT}/scripts/parity-signed-evidence.py"
printf 'signed MI300X evidence queue tests passed\n'
