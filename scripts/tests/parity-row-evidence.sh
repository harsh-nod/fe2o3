#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly TOOL="${ROOT}/scripts/parity-row-evidence.sh"
readonly ATTESTOR_PRIVATE="${TEST_DIR}/fixtures/evidence-test-attestor-private.pem"
readonly REVIEWER_PRIVATE="${TEST_DIR}/fixtures/evidence-test-reviewer-private.pem"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT
readonly REPO="${TEST_ROOT}/repo"
readonly ARCHIVE="${TEST_ROOT}/archive"
readonly TRUSTED="${TEST_ROOT}/trusted"

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'negative evidence test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'wrong evidence diagnostic for %s\n' "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    return 1
  }
}

hex_text() {
  printf '%s' "$1" | od -An -tx1 | tr -d ' \n'
}

file_size() {
  stat -c %s -- "$1"
}

file_sha() {
  sha256sum -- "$1" | awk '{ print $1 }'
}

sign_test() {
  local role="$1"
  local input="$2"
  local output="$3"
  local key
  local key_id
  if [[ "${role}" == attestor ]]; then
    key="${ATTESTOR_PRIVATE}"
    key_id=test-attestor
  else
    key="${REVIEWER_PRIVATE}"
    key_id=test-reviewer
  fi
  "${TOOL}" sign --repo "${REPO}" --private-key "${key}" --key-id "${key_id}" --domain test --role "${role}" --test-mode "${input}" "${output}"
}

write_status() {
  local file="$1"
  local commit="$2"
  local status="$3"
  {
    printf 'schema_version\t1\n'
    printf 'fe2o3_commit\t%s\n' "${commit}"
    printf 'kind\tid\tstatus\n'
    printf 'normative\t04\t%s\n' "${status}"
  } >"${file}"
}

write_policy() {
  local file="$1"
  local partial="$2"
  local complete="$3"
  {
    printf 'row_evidence_policy_schema_version\t2\n'
    printf 'row_count\t1\n'
    printf 'row\t0000\t04\tgfx942\t%s\t%s\treviewer\n' "${partial}" "${complete}"
  } >"${file}"
}

write_result() {
  local class="$1"
  local to_status="$2"
  local identity_seed="${3:-${to_status}-${class}}"
  local source="${4:-${SOURCE}}"
  local queue_path="${5:--}"
  local queue_digest="${6:--}"
  local lower="${to_status,,}"
  local result_id
  local log_relative="logs/${lower}-${class}.log"
  local artifact_relative="artifacts/${lower}-${class}.bin"
  local unsigned="${ARCHIVE}/work/${lower}-${class}-${identity_seed}.unsigned.tsv"
  local output="${ARCHIVE}/results/${lower}-${class}-${identity_seed}.tsv"
  local command="bash scripts/evidence-class-fixture.sh ${class}"
  local artifact_count=0
  local queue_id=-
  local timeout=0
  local from_status=Missing
  if [[ "${to_status}" == Complete ]]; then
    from_status=Partial
  fi
  result_id="$(printf '%s' "${identity_seed}" | sha256sum | awk '{ print $1 }')"
  mkdir -p "${ARCHIVE}/logs" "${ARCHIVE}/artifacts" "${ARCHIVE}/results" "${ARCHIVE}/work"
  (
    cd "${REPO}"
    FE2O3_FIXTURE_ARCHIVE="${ARCHIVE}" \
      FE2O3_FIXTURE_OUTPUT="${artifact_relative}" \
      bash scripts/evidence-class-fixture.sh "${class}"
  ) >"${ARCHIVE}/${log_relative}" 2>&1
  if [[ "${class}" == compile || "${class}" == hardware ]]; then
    artifact_count=1
  fi
  if [[ "${class}" == hardware ]]; then
    queue_id="$(printf '%s' "queue-${identity_seed}" | sha256sum | awk '{ print $1 }')"
    timeout=30
  fi
  {
    printf 'signed_result_schema_version\t2\n'
    printf 'result_id\t%s\n' "${result_id}"
    printf 'row_id\t04\n'
    printf 'from_status\t%s\n' "${from_status}"
    printf 'to_status\t%s\n' "${to_status}"
    printf 'baseline_commit\t%s\n' "${BASELINE}"
    printf 'source_commit\t%s\n' "${source}"
    printf 'source_tree\t%s\n' "${SOURCE_TREE}"
    printf 'evidence_class\t%s\n' "${class}"
    printf 'target\tgfx942\n'
    printf 'hardware_lane\tmi300x-gfx942-test\n'
    printf 'execution_mode\ttest\n'
    printf 'queue_manifest_path\t%s\n' "${queue_path}"
    printf 'queue_manifest_sha256\t%s\n' "${queue_digest}"
    printf 'queue_id\t%s\n' "${queue_id}"
    printf 'timeout_seconds\t%s\n' "${timeout}"
    printf 'toolchain_count\t1\n'
    printf 'toolchain\t0000\tbash\ttoolchains/bash.tsv\t%s\t%s\n' "$(file_size "${ARCHIVE}/toolchains/bash.tsv")" "$(file_sha "${ARCHIVE}/toolchains/bash.tsv")"
    printf 'command_count\t1\n'
    printf 'command\t0000\t%s\t0\n' "$(hex_text "${command}")"
    printf 'log\t0000\t%s\t%s\t%s\n' "${log_relative}" "$(file_size "${ARCHIVE}/${log_relative}")" "$(file_sha "${ARCHIVE}/${log_relative}")"
    printf 'artifact_count\t%s\n' "${artifact_count}"
    if ((artifact_count)); then
      printf 'artifact\t0000\tbinary\t%s\t%s\t%s\n' "${artifact_relative}" "$(file_size "${ARCHIVE}/${artifact_relative}")" "$(file_sha "${ARCHIVE}/${artifact_relative}")"
    fi
  } >"${unsigned}"
  sign_test attestor "${unsigned}" "${output}"
  printf '%s' "results/${lower}-${class}-${identity_seed}.tsv"
}

write_manifest_prefix() {
  local file="$1"
  local to_status="$2"
  shift 2
  local class
  local path
  local index=0
  local from_status=Missing
  if [[ "${to_status}" == Complete ]]; then
    from_status=Partial
  fi
  {
    printf 'promotion_manifest_schema_version\t2\n'
    printf 'baseline_commit\t%s\n' "${BASELINE}"
    printf 'source_commit\t%s\n' "${SOURCE}"
    printf 'source_tree\t%s\n' "${SOURCE_TREE}"
    printf 'target\tgfx942\n'
    printf 'hardware_lane\tmi300x-gfx942-test\n'
    printf 'result_count\t%s\n' "$#"
    for class in "$@"; do
      path="results/${to_status,,}-${class}-${to_status}-${class}.tsv"
      printf 'result\t%04d\t04\t%s\t%s\t%s\t%s\t%s\t%s\n' "${index}" "${from_status}" "${to_status}" "${class}" "${path}" "$(file_sha "${ARCHIVE}/${path}")" "$(awk -F '\t' '$1 == "result_id" { print $2 }' "${ARCHIVE}/${path}")"
      ((index += 1))
    done
  } >"${file}"
}

finish_manifest() {
  local file="$1"
  local authorization="${2:-}"
  local digest
  digest="$(file_sha "${file}")"
  printf 'evidence_set_sha256\t%s\n' "${digest}" >>"${file}"
  if [[ -z "${authorization}" ]]; then
    printf 'authorization_count\t0\n' >>"${file}"
  else
    printf 'authorization_count\t1\n' >>"${file}"
    printf 'authorization\t0000\t04\t%s\t%s\n' "${authorization}" "$(file_sha "${ARCHIVE}/${authorization}")" >>"${file}"
  fi
}

write_authorization() {
  local evidence_set="$1"
  local unsigned="${ARCHIVE}/work/review.unsigned.tsv"
  local output="${ARCHIVE}/authorizations/review.tsv"
  mkdir -p "${ARCHIVE}/authorizations"
  {
    printf 'review_authorization_schema_version\t1\n'
    printf 'authorization_id\t%s\n' "$(printf '%s' "review-${evidence_set}" | sha256sum | awk '{ print $1 }')"
    printf 'row_id\t04\n'
    printf 'from_status\tPartial\n'
    printf 'baseline_commit\t%s\n' "${BASELINE}"
    printf 'source_commit\t%s\n' "${SOURCE}"
    printf 'source_tree\t%s\n' "${SOURCE_TREE}"
    printf 'to_status\tComplete\n'
    printf 'target\tgfx942\n'
    printf 'hardware_lane\tmi300x-gfx942-test\n'
    printf 'evidence_set_sha256\t%s\n' "${evidence_set}"
    printf 'reviewer_identity\ttest-reviewer\n'
    printf 'execution_mode\ttest\n'
  } >"${unsigned}"
  sign_test reviewer "${unsigned}" "${output}"
}

gate_args=()
set_gate_args() {
  local manifest="$1"
  local candidate="$2"
  local baseline_status="${3:-${TEST_ROOT}/baseline.tsv}"
  gate_args=(
    gate
    --repo "${REPO}"
    --archive-root "${ARCHIVE}"
    --trusted-root "${TRUSTED}"
    --trust-policy "${TRUSTED}/trust.tsv"
    --manifest "${manifest}"
    --trusted-policy "${TEST_ROOT}/policy.tsv"
    --candidate-policy "${TEST_ROOT}/candidate-policy.tsv"
    --baseline-status "${baseline_status}"
    --candidate-status "${candidate}"
    --allow-test-fixtures
  )
}

git init -q "${REPO}"
git -C "${REPO}" config user.email evidence@example.invalid
git -C "${REPO}" config user.name 'Evidence Fixture'
mkdir -p "${REPO}/scripts" "${REPO}/tests"
cat >"${REPO}/scripts/evidence-class-fixture.sh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
case "$1" in
  unit)
    [[ "$((20 22))" == 42 ]]
    ;;
  ui)
    if bash -n tests/invalid-ui.sh 2>/dev/null; then
      exit 1
    fi
    ;;
  ir)
    awk '$1 == "block" && $2 == "entry" { found = 1 } END { exit !found }' tests/kernel.ir
    ;;
  compile)
    bash -n scripts/compile-input.sh
    mkdir -p "$(dirname -- "${FE2O3_FIXTURE_ARCHIVE}/${FE2O3_FIXTURE_OUTPUT}")"
    cp scripts/compile-input.sh "${FE2O3_FIXTURE_ARCHIVE}/${FE2O3_FIXTURE_OUTPUT}"
    ;;
  hardware)
    mkdir -p "$(dirname -- "${FE2O3_FIXTURE_ARCHIVE}/${FE2O3_FIXTURE_OUTPUT}")"
    printf 'test-only queue bypass artifact\n' >"${FE2O3_FIXTURE_ARCHIVE}/${FE2O3_FIXTURE_OUTPUT}"
    ;;
  *)
    exit 2
    ;;
esac
printf '%s fixture completed\n' "$1"
EOF
cat >"${REPO}/scripts/compile-input.sh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
printf 'compiled fixture\n'
EOF
cat >"${REPO}/tests/invalid-ui.sh" <<'EOF'
if then
EOF
cat >"${REPO}/tests/kernel.ir" <<'EOF'
block entry
return
EOF
chmod 755 "${REPO}/scripts/evidence-class-fixture.sh" "${REPO}/scripts/compile-input.sh"
git -C "${REPO}" add scripts tests
git -C "${REPO}" commit -qm baseline
BASELINE="$(git -C "${REPO}" rev-parse HEAD)"
readonly BASELINE
printf 'source marker\n' >>"${REPO}/tests/kernel.ir"
git -C "${REPO}" commit -qam source
SOURCE="$(git -C "${REPO}" rev-parse HEAD)"
readonly SOURCE
SOURCE_TREE="$(git -C "${REPO}" rev-parse 'HEAD^{tree}')"
readonly SOURCE_TREE
git -C "${REPO}" checkout -q --detach

PROTECTED_BASE_REPO="${TEST_ROOT}/protected-base-repo"
CANDIDATE_HEAD_REPO="${TEST_ROOT}/candidate-head-repo"
git clone -q "${REPO}" "${PROTECTED_BASE_REPO}"
git clone -q "${REPO}" "${CANDIDATE_HEAD_REPO}"
git -C "${PROTECTED_BASE_REPO}" checkout -q --detach "${SOURCE}"
git -C "${CANDIDATE_HEAD_REPO}" checkout -q --detach "${SOURCE}"
protected_base_args=(
  check-protected-base
  --protected-repo "${PROTECTED_BASE_REPO}"
  --candidate-repo "${CANDIDATE_HEAD_REPO}"
  --protected-base "${SOURCE}"
  --default-tip "${SOURCE}"
  --candidate-head "${SOURCE}"
)
"${TOOL}" "${protected_base_args[@]}"
expect_failure stale_default_tip 'pull request base SHA is not current default tip' \
  "${TOOL}" "${protected_base_args[@]:0:8}" "${BASELINE}" \
  "${protected_base_args[@]:9}"
expect_failure zero_default_tip 'malformed or zero current default tip commit' \
  "${TOOL}" "${protected_base_args[@]:0:8}" "$(printf '0%.0s' {1..40})" \
  "${protected_base_args[@]:9}"
expect_failure protected_checkout_substitution \
  'protected checkout does not match event base SHA' \
  "${TOOL}" check-protected-base \
  --protected-repo "${PROTECTED_BASE_REPO}" \
  --candidate-repo "${CANDIDATE_HEAD_REPO}" \
  --protected-base "${BASELINE}" \
  --default-tip "${BASELINE}" \
  --candidate-head "${SOURCE}"
expect_failure candidate_checkout_substitution \
  'candidate checkout does not match event head SHA' \
  "${TOOL}" "${protected_base_args[@]:0:10}" "${BASELINE}"
git -C "${CANDIDATE_HEAD_REPO}" checkout -q --detach "${BASELINE}"
expect_failure stale_candidate_ancestry \
  'candidate head does not contain current protected default tip' \
  "${TOOL}" check-protected-base \
  --protected-repo "${PROTECTED_BASE_REPO}" \
  --candidate-repo "${CANDIDATE_HEAD_REPO}" \
  --protected-base "${SOURCE}" \
  --default-tip "${SOURCE}" \
  --candidate-head "${BASELINE}"
git -C "${CANDIDATE_HEAD_REPO}" checkout -q --detach "${SOURCE}"

mkdir -p "${ARCHIVE}/toolchains" "${TRUSTED}/keys"
{
  printf 'toolchain_closure_schema_version\t1\n'
  printf 'executable\tbash\t%s\t%s\n' "$(command -v bash)" "$(file_sha "$(command -v bash)")"
  printf 'version_sha256\t%s\n' "$(bash --version | file_sha /dev/stdin)"
} >"${ARCHIVE}/toolchains/bash.tsv"
cp "${TEST_DIR}/fixtures/evidence-test-attestor-public.pem" "${TRUSTED}/keys/attestor.pem"
cp "${TEST_DIR}/fixtures/evidence-test-reviewer-public.pem" "${TRUSTED}/keys/reviewer.pem"
{
  printf 'parity_trust_policy_schema_version\t2\n'
  printf 'trust_domain\ttest\n'
  printf 'metadata_path_count\t0\n'
  printf 'key_count\t2\n'
  printf 'key\t0000\tattestor\ttest-attestor\tkeys/attestor.pem\t%s\ted25519\n' "$(file_sha "${TRUSTED}/keys/attestor.pem")"
  printf 'key\t0001\treviewer\ttest-reviewer\tkeys/reviewer.pem\t%s\ted25519\n' "$(file_sha "${TRUSTED}/keys/reviewer.pem")"
} >"${TRUSTED}/trust.tsv"

write_policy "${TEST_ROOT}/policy.tsv" unit,ui unit,ui,ir,compile
cp "${TEST_ROOT}/policy.tsv" "${TEST_ROOT}/candidate-policy.tsv"
write_status "${TEST_ROOT}/baseline.tsv" "${BASELINE}" Missing
write_status "${TEST_ROOT}/baseline-partial.tsv" "${BASELINE}" Partial
write_status "${TEST_ROOT}/partial.tsv" "${SOURCE}" Partial
write_status "${TEST_ROOT}/complete.tsv" "${SOURCE}" Complete

for class in unit ui; do
  write_result "${class}" Partial >/dev/null
done
write_manifest_prefix "${ARCHIVE}/partial.tsv" Partial unit ui
finish_manifest "${ARCHIVE}/partial.tsv"

common_validate=(
  --repo "${REPO}"
  --archive-root "${ARCHIVE}"
  --trusted-root "${TRUSTED}"
  --trust-policy "${TRUSTED}/trust.tsv"
)
"${TOOL}" validate-result "${common_validate[@]}" results/partial-unit-Partial-unit.tsv
"${TOOL}" validate-manifest "${common_validate[@]}" partial.tsv
"${TOOL}" validate-shard "${common_validate[@]}" --manifest partial.tsv --row 04
set_gate_args partial.tsv "${TEST_ROOT}/partial.tsv"
"${TOOL}" "${gate_args[@]}"
expect_failure test_domain 'production promotion requires a production trust domain' "${TOOL}" "${gate_args[@]:0:${#gate_args[@]}-1}"

for class in unit ui ir compile; do
  write_result "${class}" Complete >/dev/null
done
write_manifest_prefix "${ARCHIVE}/complete-prefix.tsv" Complete unit ui ir compile
EVIDENCE_SET="$(file_sha "${ARCHIVE}/complete-prefix.tsv")"
readonly EVIDENCE_SET
write_authorization "${EVIDENCE_SET}"
mv "${ARCHIVE}/complete-prefix.tsv" "${ARCHIVE}/complete.tsv"
finish_manifest "${ARCHIVE}/complete.tsv" authorizations/review.tsv
set_gate_args complete.tsv "${TEST_ROOT}/complete.tsv" "${TEST_ROOT}/baseline-partial.tsv"
"${TOOL}" "${gate_args[@]}"

# Signature mutation.
cp "${ARCHIVE}/results/partial-unit-Partial-unit.tsv" "${ARCHIVE}/results/signature-mutated.tsv"
signature_before="$(awk -F '\t' '$1 == "signature_base64" { print $2 }' "${ARCHIVE}/results/signature-mutated.tsv")"
awk -F '\t' 'BEGIN { OFS = FS } $1 == "signature_base64" {
  replacement = substr($2, 1, 1) == "A" ? "B" : "A"
  $2 = replacement substr($2, 2)
} { print }' "${ARCHIVE}/results/signature-mutated.tsv" >"${ARCHIVE}/results/signature-mutated.tmp"
mv "${ARCHIVE}/results/signature-mutated.tmp" "${ARCHIVE}/results/signature-mutated.tsv"
[[ "$(awk -F '\t' '$1 == "signature_base64" { print $2 }' "${ARCHIVE}/results/signature-mutated.tsv")" != "${signature_before}" ]]
expect_failure signature_mutation 'signature verification failed' "${TOOL}" validate-result "${common_validate[@]}" results/signature-mutated.tsv

# Signature context fields are canonical and authenticated.
cp "${ARCHIVE}/results/partial-unit-Partial-unit.tsv" "${ARCHIVE}/results/domain-mutated.tsv"
sed -i 's/^signature_domain\ttest$/signature_domain\tproduction/' "${ARCHIVE}/results/domain-mutated.tsv"
expect_failure domain_mutation 'non-canonical signed signature context' "${TOOL}" validate-result "${common_validate[@]}" results/domain-mutated.tsv
cp "${ARCHIVE}/results/partial-unit-Partial-unit.tsv" "${ARCHIVE}/results/role-mutated.tsv"
sed -i 's/^signature_role\tattestor$/signature_role\treviewer/' "${ARCHIVE}/results/role-mutated.tsv"
expect_failure role_mutation 'non-canonical signed signature context' "${TOOL}" validate-result "${common_validate[@]}" results/role-mutated.tsv
cp "${ARCHIVE}/results/partial-unit-Partial-unit.tsv" "${ARCHIVE}/results/algorithm-mutated.tsv"
sed -i 's/^signature_algorithm\ted25519$/signature_algorithm\tunknown/' "${ARCHIVE}/results/algorithm-mutated.tsv"
expect_failure algorithm_mutation 'non-canonical signed signature context' "${TOOL}" validate-result "${common_validate[@]}" results/algorithm-mutated.tsv

# A different trusted key ID still invalidates the signature bytes.
mkdir -p "${TEST_ROOT}/alternate-trust/keys"
cp "${TRUSTED}/keys/attestor.pem" "${TEST_ROOT}/alternate-trust/keys/attestor.pem"
cp "${TRUSTED}/keys/reviewer.pem" "${TEST_ROOT}/alternate-trust/keys/alternate.pem"
{
  printf 'parity_trust_policy_schema_version\t2\n'
  printf 'trust_domain\ttest\n'
  printf 'metadata_path_count\t0\n'
  printf 'key_count\t2\n'
  printf 'key\t0000\tattestor\ttest-attestor\tkeys/attestor.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/alternate-trust/keys/attestor.pem")"
  printf 'key\t0001\tattestor\ttest-attestor-alt\tkeys/alternate.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/alternate-trust/keys/alternate.pem")"
} >"${TEST_ROOT}/alternate-trust/trust.tsv"
cp "${ARCHIVE}/results/partial-unit-Partial-unit.tsv" "${ARCHIVE}/results/key-id-mutated.tsv"
sed -i 's/^signing_key_id\ttest-attestor$/signing_key_id\ttest-attestor-alt/' "${ARCHIVE}/results/key-id-mutated.tsv"
expect_failure key_id_mutation 'signature verification failed' "${TOOL}" validate-result --repo "${REPO}" --archive-root "${ARCHIVE}" --trusted-root "${TEST_ROOT}/alternate-trust" --trust-policy "${TEST_ROOT}/alternate-trust/trust.tsv" results/key-id-mutated.tsv

# Distinct trust identities cannot alias one public-key fingerprint.
mkdir -p "${TEST_ROOT}/duplicate-fingerprint/keys"
cp "${TRUSTED}/keys/attestor.pem" "${TEST_ROOT}/duplicate-fingerprint/keys/one.pem"
cp "${TRUSTED}/keys/attestor.pem" "${TEST_ROOT}/duplicate-fingerprint/keys/two.pem"
{
  printf 'parity_trust_policy_schema_version\t2\n'
  printf 'trust_domain\ttest\n'
  printf 'metadata_path_count\t0\n'
  printf 'key_count\t2\n'
  printf 'key\t0000\tattestor\tone\tkeys/one.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/duplicate-fingerprint/keys/one.pem")"
  printf 'key\t0001\tattestor\ttwo\tkeys/two.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/duplicate-fingerprint/keys/two.pem")"
} >"${TEST_ROOT}/duplicate-fingerprint/trust.tsv"
expect_failure duplicate_fingerprint 'duplicate trusted public-key fingerprint' "${TOOL}" validate-result --repo "${REPO}" --archive-root "${ARCHIVE}" --trusted-root "${TEST_ROOT}/duplicate-fingerprint" --trust-policy "${TEST_ROOT}/duplicate-fingerprint/trust.tsv" results/partial-unit-Partial-unit.tsv

# PEM byte differences cannot disguise reuse of the same Ed25519 key material.
mkdir -p "${TEST_ROOT}/canonical-fingerprint/keys"
cp "${TRUSTED}/keys/attestor.pem" "${TEST_ROOT}/canonical-fingerprint/keys/one.pem"
cp "${TRUSTED}/keys/attestor.pem" "${TEST_ROOT}/canonical-fingerprint/keys/two.pem"
printf '\n' >>"${TEST_ROOT}/canonical-fingerprint/keys/two.pem"
[[ "$(file_sha "${TEST_ROOT}/canonical-fingerprint/keys/one.pem")" != "$(file_sha "${TEST_ROOT}/canonical-fingerprint/keys/two.pem")" ]]
{
  printf 'parity_trust_policy_schema_version\t2\n'
  printf 'trust_domain\ttest\n'
  printf 'metadata_path_count\t0\n'
  printf 'key_count\t2\n'
  printf 'key\t0000\tattestor\tone\tkeys/one.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/canonical-fingerprint/keys/one.pem")"
  printf 'key\t0001\treviewer\ttwo\tkeys/two.pem\t%s\ted25519\n' "$(file_sha "${TEST_ROOT}/canonical-fingerprint/keys/two.pem")"
} >"${TEST_ROOT}/canonical-fingerprint/trust.tsv"
expect_failure canonical_fingerprint 'duplicate trusted public-key fingerprint' "${TOOL}" validate-result --repo "${REPO}" --archive-root "${ARCHIVE}" --trusted-root "${TEST_ROOT}/canonical-fingerprint" --trust-policy "${TEST_ROOT}/canonical-fingerprint/trust.tsv" results/partial-unit-Partial-unit.tsv

# Protected production trust requirements are monotonic without break-glass.
TRUST_OLD="${TEST_ROOT}/trust-old"
TRUST_CANDIDATE="${TEST_ROOT}/trust-candidate"
PRODUCTION_ATTESTOR_PRIVATE="${TEST_ROOT}/production-attestor-private.pem"
PRODUCTION_REVIEWER_PRIVATE="${TEST_ROOT}/production-reviewer-private.pem"
PRODUCTION_ATTESTOR_PUBLIC="${TEST_ROOT}/production-attestor-public.pem"
PRODUCTION_REVIEWER_PUBLIC="${TEST_ROOT}/production-reviewer-public.pem"
openssl genpkey -algorithm Ed25519 -out "${PRODUCTION_ATTESTOR_PRIVATE}" 2>/dev/null
openssl genpkey -algorithm Ed25519 -out "${PRODUCTION_REVIEWER_PRIVATE}" 2>/dev/null
openssl pkey -in "${PRODUCTION_ATTESTOR_PRIVATE}" -pubout \
  -out "${PRODUCTION_ATTESTOR_PUBLIC}" 2>/dev/null
openssl pkey -in "${PRODUCTION_REVIEWER_PRIVATE}" -pubout \
  -out "${PRODUCTION_REVIEWER_PUBLIC}" 2>/dev/null

"${TOOL}" bootstrap-production-trust \
  --output-root "${TRUST_OLD}" \
  --attestor-public-key "${PRODUCTION_ATTESTOR_PUBLIC}" \
  --attestor-key-id production-attestor \
  --reviewer-public-key "${PRODUCTION_REVIEWER_PUBLIC}" \
  --reviewer-key-id production-reviewer
"${TOOL}" validate-production-trust \
  --trusted-root "${TRUST_OLD}" \
  --trust-policy "${TRUST_OLD}/docs/parity-evidence/trust-policy-v2.tsv"
[[ "$(stat -c %a "${TRUST_OLD}/docs/parity-evidence/trust-policy-v2.tsv")" == 444 ]]
[[ "$(stat -c %a "${TRUST_OLD}/docs/parity-evidence/trusted-keys/production-attestor.pem")" == 444 ]]
if rg -F -- 'PRIVATE KEY' "${TRUST_OLD}" >/dev/null; then
  printf 'production bootstrap copied private key material\n' >&2
  exit 1
fi

expect_failure bootstrap_existing 'production trust bootstrap output already exists' \
  "${TOOL}" bootstrap-production-trust \
  --output-root "${TRUST_OLD}" \
  --attestor-public-key "${PRODUCTION_ATTESTOR_PUBLIC}" \
  --attestor-key-id production-attestor \
  --reviewer-public-key "${PRODUCTION_REVIEWER_PUBLIC}" \
  --reviewer-key-id production-reviewer
expect_failure bootstrap_private_input 'public key is not public Ed25519 material' \
  "${TOOL}" bootstrap-production-trust \
  --output-root "${TEST_ROOT}/private-input-bootstrap" \
  --attestor-public-key "${PRODUCTION_ATTESTOR_PRIVATE}" \
  --attestor-key-id production-attestor \
  --reviewer-public-key "${PRODUCTION_REVIEWER_PUBLIC}" \
  --reviewer-key-id production-reviewer
expect_failure bootstrap_duplicate_key 'attestor and reviewer must use distinct Ed25519 public keys' \
  "${TOOL}" bootstrap-production-trust \
  --output-root "${TEST_ROOT}/duplicate-key-bootstrap" \
  --attestor-public-key "${PRODUCTION_ATTESTOR_PUBLIC}" \
  --attestor-key-id production-attestor \
  --reviewer-public-key "${PRODUCTION_ATTESTOR_PUBLIC}" \
  --reviewer-key-id production-reviewer

{
  printf 'row_evidence_policy_schema_version\t2\n'
  printf 'row_count\t2\n'
  printf 'row\t0000\t04\tgfx942\tunit,ui\tunit,ui,ir,compile\treviewer\n'
  printf 'row\t0001\t05\tgfx950\tunit,ui\tunit,ui,ir,compile\treviewer\n'
} >"${TRUST_OLD}/row-policy.tsv"

TRUST_EMPTY="${TEST_ROOT}/trust-empty"
mkdir -p "${TRUST_EMPTY}/docs/parity-evidence"
cp "${TRUST_OLD}/row-policy.tsv" "${TRUST_EMPTY}/row-policy.tsv"
"${TOOL}" check-trust-update \
  --protected-root "${TRUST_EMPTY}" \
  --protected-policy "${TRUST_EMPTY}/docs/parity-evidence/trust-policy-v2.tsv" \
  --protected-row-policy "${TRUST_EMPTY}/row-policy.tsv" \
  --candidate-root "${TRUST_OLD}" \
  --candidate-policy "${TRUST_OLD}/docs/parity-evidence/trust-policy-v2.tsv" \
  --candidate-row-policy "${TRUST_OLD}/row-policy.tsv"

cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"
trust_update_args=(
  check-trust-update
  --protected-root "${TRUST_OLD}"
  --protected-policy "${TRUST_OLD}/docs/parity-evidence/trust-policy-v2.tsv"
  --protected-row-policy "${TRUST_OLD}/row-policy.tsv"
  --candidate-root "${TRUST_CANDIDATE}"
  --candidate-policy "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
  --candidate-row-policy "${TRUST_CANDIDATE}/row-policy.tsv"
)
"${TOOL}" "${trust_update_args[@]}"

sed -i 's/^row_count\t2$/row_count\t1/' "${TRUST_CANDIDATE}/row-policy.tsv"
sed -i '/^row\t0001\t05\t/d' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure row_policy_exact_removal 'row policy row set cannot change without break-glass' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/04\tgfx942\tunit,ui\t/04\tgfx942\tunit\t/' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure partial_policy_downgrade 'Partial evidence requirements cannot be removed for row 04' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/unit,ui,ir,compile\treviewer/unit,ui,ir\treviewer/' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure complete_policy_downgrade 'Complete evidence requirements cannot be removed for row 04' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/05\tgfx950/05\tgfx941/' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure row_policy_target_set 'row policy target set cannot change without break-glass' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/04\tgfx942/04\tgfx950/; s/05\tgfx950/05\tgfx942/' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure row_policy_target_identity 'row policy target identity cannot change for row 04' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i '0,/\treviewer$/s//\tattestor/' "${TRUST_CANDIDATE}/row-policy.tsv"
expect_failure row_policy_reviewer_role 'row policy reviewer role cannot change for row 04' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/04\tgfx942\tunit,ui\tunit,ui,ir,compile/04\tgfx942\tunit,ui,ir\tunit,ui,ir,compile,verus/' "${TRUST_CANDIDATE}/row-policy.tsv"
"${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's#prefix\tdocs/parity-evidence/archive/#prefix\tdocs/#' "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure metadata_allowlist_downgrade 'non-canonical metadata allowlist' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

sed -i 's/^trust_domain\tproduction$/trust_domain\ttest/' "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure trust_domain_downgrade 'must use the production domain' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

chmod u+w "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem"
printf '\n' >>"${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem"
sed -i "s#trusted-keys/production-attestor.pem\t[0-9a-f]*#trusted-keys/production-attestor.pem\t$(file_sha "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem")#" "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure noncanonical_public_key 'production public key is not canonical PEM' \
  "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

mv "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem" \
  "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/attestor-other.pem"
sed -i 's#trusted-keys/production-attestor.pem#trusted-keys/attestor-other.pem#' \
  "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure noncanonical_public_key_path 'production public key is missing' \
  "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

openssl genpkey -algorithm Ed25519 -out "${TEST_ROOT}/replacement-private.pem" 2>/dev/null
chmod u+w "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem"
openssl pkey -in "${TEST_ROOT}/replacement-private.pem" -pubout -out "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem" 2>/dev/null
sed -i "s#trusted-keys/production-attestor.pem\t[0-9a-f]*#trusted-keys/production-attestor.pem\t$(file_sha "${TRUST_CANDIDATE}/docs/parity-evidence/trusted-keys/production-attestor.pem")#" "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure signing_authority_replacement 'cannot replace signing authority without break-glass' "${TOOL}" "${trust_update_args[@]}"
rm -rf "${TRUST_CANDIDATE}"
cp -a "${TRUST_OLD}" "${TRUST_CANDIDATE}"

rm "${TRUST_CANDIDATE}/docs/parity-evidence/trust-policy-v2.tsv"
expect_failure trust_policy_removal 'active trust policy cannot be removed without break-glass' "${TOOL}" "${trust_update_args[@]}"

# Production signing cannot omit the repository trust boundary.
expect_failure production_repo_required '--repo is required outside explicit test mode' "${TOOL}" sign --private-key "${ATTESTOR_PRIVATE}" --key-id production-attestor --domain production --role attestor "${ARCHIVE}/work/complete-unit-Complete-unit.unsigned.tsv" "${ARCHIVE}/results/no-repo.tsv"

# Protected key substitution.
cp -a "${TRUSTED}" "${TEST_ROOT}/substituted-trust"
cp "${TRUSTED}/keys/reviewer.pem" "${TEST_ROOT}/substituted-trust/keys/attestor.pem"
expect_failure key_substitution 'trusted public key digest mismatch' "${TOOL}" validate-result --repo "${REPO}" --archive-root "${ARCHIVE}" --trusted-root "${TEST_ROOT}/substituted-trust" --trust-policy "${TEST_ROOT}/substituted-trust/trust.tsv" results/partial-unit-Partial-unit.tsv

# Relabeling cannot change signed row, class, or target.
cp "${ARCHIVE}/partial.tsv" "${ARCHIVE}/class-relabel.tsv"
sed -i 's/\tunit\tresults\/partial-unit/\tui\tresults\/partial-unit/' "${ARCHIVE}/class-relabel.tsv"
expect_failure class_relabel 'manifest result relabeling' "${TOOL}" validate-manifest "${common_validate[@]}" class-relabel.tsv
cp "${ARCHIVE}/partial.tsv" "${ARCHIVE}/row-relabel.tsv"
sed -i 's/result\t0000\t04/result\t0000\t05/' "${ARCHIVE}/row-relabel.tsv"
expect_failure row_relabel 'manifest result relabeling' "${TOOL}" validate-manifest "${common_validate[@]}" row-relabel.tsv
cp "${ARCHIVE}/partial.tsv" "${ARCHIVE}/target-relabel.tsv"
sed -i 's/target\tgfx942/target\tgfx950/' "${ARCHIVE}/target-relabel.tsv"
expect_failure target_relabel 'manifest result relabeling' "${TOOL}" validate-manifest "${common_validate[@]}" target-relabel.tsv

# The signed source status prevents cross-transition evidence replay.
cp "${ARCHIVE}/complete.tsv" "${ARCHIVE}/transition-relabel.tsv"
sed -i 's/\t04\tPartial\tComplete\t/\t04\tMissing\tComplete\t/' "${ARCHIVE}/transition-relabel.tsv"
expect_failure transition_relabel 'manifest result relabeling' "${TOOL}" validate-manifest "${common_validate[@]}" transition-relabel.tsv

cp "${ARCHIVE}/work/review.unsigned.tsv" "${ARCHIVE}/work/review-wrong-transition.unsigned.tsv"
sed -i 's/^from_status\tPartial$/from_status\tMissing/' "${ARCHIVE}/work/review-wrong-transition.unsigned.tsv"
sign_test reviewer "${ARCHIVE}/work/review-wrong-transition.unsigned.tsv" "${ARCHIVE}/authorizations/review-wrong-transition.tsv"
wrong_review_digest="$(file_sha "${ARCHIVE}/authorizations/review-wrong-transition.tsv")"
cp "${ARCHIVE}/complete.tsv" "${ARCHIVE}/review-transition-mismatch.tsv"
awk -F '\t' -v OFS='\t' -v digest="${wrong_review_digest}" '$1 == "authorization" { $4 = "authorizations/review-wrong-transition.tsv"; $5 = digest } { print }' "${ARCHIVE}/review-transition-mismatch.tsv" >"${ARCHIVE}/review-transition-mismatch.tmp"
mv "${ARCHIVE}/review-transition-mismatch.tmp" "${ARCHIVE}/review-transition-mismatch.tsv"
set_gate_args review-transition-mismatch.tsv "${TEST_ROOT}/complete.tsv" "${TEST_ROOT}/baseline-partial.tsv"
expect_failure review_transition_mismatch 'review authorization transition mismatch' "${TOOL}" "${gate_args[@]}"


# A signed result naming an unknown source is stale.
stale_path="$(write_result unit Partial stale-source aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)"
expect_failure stale_source 'stale result commit' "${TOOL}" validate-result "${common_validate[@]}" "${stale_path}"

# Hardware cannot bypass a signed queue manifest.
bypass_path="$(write_result hardware Complete queue-bypass "${SOURCE}" queues/missing.tsv aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)"
expect_failure queue_bypass 'hardware result has no signed queue binding' "${TOOL}" validate-result "${common_validate[@]}" "${bypass_path}"

# A result cannot be replayed against a different baseline.
cp "${ARCHIVE}/partial.tsv" "${ARCHIVE}/result-replay.tsv"
sed -i "s/baseline_commit\t${BASELINE}/baseline_commit\t${SOURCE}/" "${ARCHIVE}/result-replay.tsv"
expect_failure result_replay 'manifest result relabeling or source mismatch' "${TOOL}" validate-manifest "${common_validate[@]}" result-replay.tsv

# Result identities are globally unique and cannot be shared across classes.
unit_id="$(awk -F '\t' '$1 == "result_id" { print $2 }' "${ARCHIVE}/results/complete-unit-Complete-unit.tsv")"
cp "${ARCHIVE}/work/complete-ui-Complete-ui.unsigned.tsv" "${ARCHIVE}/work/duplicate-id.unsigned.tsv"
sed -i "s/^result_id\t.*/result_id\t${unit_id}/" "${ARCHIVE}/work/duplicate-id.unsigned.tsv"
sign_test attestor "${ARCHIVE}/work/duplicate-id.unsigned.tsv" "${ARCHIVE}/results/complete-ui-duplicate-id.tsv"
duplicate_digest="$(file_sha "${ARCHIVE}/results/complete-ui-duplicate-id.tsv")"
cp "${ARCHIVE}/complete.tsv" "${ARCHIVE}/duplicate-result.tsv"
awk -F '\t' -v OFS='\t' -v path=results/complete-ui-duplicate-id.tsv -v digest="${duplicate_digest}" -v identity="${unit_id}" '$1 == "result" && $6 == "ui" { $7 = path; $8 = digest; $9 = identity } { print }' "${ARCHIVE}/duplicate-result.tsv" >"${ARCHIVE}/duplicate-result.tmp"
mv "${ARCHIVE}/duplicate-result.tmp" "${ARCHIVE}/duplicate-result.tsv"
expect_failure duplicate_result 'duplicate result identity, path, or digest' "${TOOL}" validate-manifest "${common_validate[@]}" duplicate-result.tsv

# Partial policy is exact: missing a required class is insufficient.
write_manifest_prefix "${ARCHIVE}/insufficient.tsv" Partial unit
finish_manifest "${ARCHIVE}/insufficient.tsv"
set_gate_args insufficient.tsv "${TEST_ROOT}/partial.tsv"
expect_failure insufficient_evidence 'insufficient or extra evidence for row 04' "${TOOL}" "${gate_args[@]}"

# Complete requires an independently signed review over the exact evidence set.
write_manifest_prefix "${ARCHIVE}/complete-no-review.tsv" Complete unit ui ir compile
finish_manifest "${ARCHIVE}/complete-no-review.tsv"
set_gate_args complete-no-review.tsv "${TEST_ROOT}/complete.tsv" "${TEST_ROOT}/baseline-partial.tsv"
expect_failure missing_review 'Complete promotion lacks reviewed authorization' "${TOOL}" "${gate_args[@]}"

# Candidate policy cannot remove a Complete requirement.
cp "${TEST_ROOT}/policy.tsv" "${TEST_ROOT}/downgraded-policy.tsv"
sed -i 's/unit,ui,ir,compile/unit,ui,ir/' "${TEST_ROOT}/downgraded-policy.tsv"
cp "${TEST_ROOT}/downgraded-policy.tsv" "${TEST_ROOT}/candidate-policy.tsv"
set_gate_args complete.tsv "${TEST_ROOT}/complete.tsv" "${TEST_ROOT}/baseline-partial.tsv"
expect_failure complete_downgrade 'candidate row policy differs from protected baseline policy' "${TOOL}" "${gate_args[@]}"
cp "${TEST_ROOT}/policy.tsv" "${TEST_ROOT}/candidate-policy.tsv"

# Exact implementation tree permits no unlisted source delta.
git -C "${REPO}" switch -q -c post-attestation
printf 'implementation delta\n' >>"${REPO}/scripts/compile-input.sh"
git -C "${REPO}" commit -qam 'implementation delta'
set_gate_args partial.tsv "${TEST_ROOT}/partial.tsv"
expect_failure implementation_delta 'implementation changed after attestation' "${TOOL}" "${gate_args[@]}"
git -C "${REPO}" checkout -q --detach "${SOURCE}"

# An explicit protected metadata allowlist permits only that post-attestation delta.
git -C "${REPO}" switch -q -c metadata-only
mkdir -p "${REPO}/docs"
printf 'promotion metadata\n' >"${REPO}/docs/evidence-metadata.tsv"
git -C "${REPO}" add docs/evidence-metadata.tsv
git -C "${REPO}" commit -qm 'metadata-only evidence'
awk 'BEGIN { FS = OFS = "\t" } $1 == "metadata_path_count" { $2 = 1; print; print "metadata_path", "0000", "exact", "docs/evidence-metadata.tsv"; next } { print }' "${TRUSTED}/trust.tsv" >"${TRUSTED}/trust-metadata.tsv"
"${TOOL}" gate --repo "${REPO}" --archive-root "${ARCHIVE}" --trusted-root "${TRUSTED}" --trust-policy "${TRUSTED}/trust-metadata.tsv" --manifest partial.tsv --trusted-policy "${TEST_ROOT}/policy.tsv" --candidate-policy "${TEST_ROOT}/candidate-policy.tsv" --baseline-status "${TEST_ROOT}/baseline.tsv" --candidate-status "${TEST_ROOT}/partial.tsv" --allow-test-fixtures
git -C "${REPO}" checkout -q --detach "${SOURCE}"

# Persistent policy covers the full row universe across status transitions.
[[ "$(awk -F '\t' '$1 == "row" { count += 1 } END { print count }' "${ROOT}/docs/parity-row-evidence-policy-v2.tsv")" == 109 ]]
[[ "$(awk -F '\t' '$1 == "row" { print $3 }' "${ROOT}/docs/parity-row-evidence-policy-v2.tsv" | sort -u | wc -l)" == 109 ]]
[[ ! -e "${ROOT}/docs/parity-row-evidence-policy-v1.tsv" ]]
strong_rows=(04 05 06 11 13 14 15 16 18 19 21 22 23 29 30 31 34 47 50 52 56 62 68 69 72 73 82 88 S08 S12 S13)
for row in "${strong_rows[@]}"; do
  read -r partial complete < <(awk -F '\t' -v row="${row}" '$1 == "row" && $3 == row { print $5, $6 }' "${ROOT}/docs/parity-row-evidence-policy-v2.tsv")
  [[ "${partial}" == unit,ui,ir,compile,verus,hardware ]]
  [[ "${complete}" == unit,ui,ir,compile,verus,hardware,debug ]]
done
for row in 46 75 76 77; do
  read -r partial complete < <(awk -F '\t' -v row="${row}" '$1 == "row" && $3 == row { print $5, $6 }' "${ROOT}/docs/parity-row-evidence-policy-v2.tsv")
  [[ "${partial}" == unit,ui,ir,compile,hardware,debug ]]
  [[ "${complete}" == unit,ui,ir,compile,verus,hardware,debug ]]
done
read -r partial complete < <(awk -F '\t' '$1 == "row" && $3 == "S09" { print $5, $6 }' "${ROOT}/docs/parity-row-evidence-policy-v2.tsv")
[[ "${partial}" == unit,ui,compile,debug ]]
[[ "${complete}" == unit,ui,compile,verus,debug ]]


bash -n "${TOOL}" "${BASH_SOURCE[0]}"
python3 -m py_compile "${ROOT}/scripts/parity-signed-evidence.py"
shellcheck "${TOOL}" "${BASH_SOURCE[0]}"
printf 'signed parity row evidence tests passed\n'
