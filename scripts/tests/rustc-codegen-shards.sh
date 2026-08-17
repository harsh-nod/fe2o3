#!/usr/bin/env bash

set -Eeuo pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly POLICY="${ROOT}/scripts/rustc-codegen-shards.py"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

expect_policy_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if python3 "${POLICY}" "$@" \
    >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'shard policy negative test unexpectedly passed: %s\n' "${name}" >&2
    exit 1
  fi
  rg -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'shard policy negative test produced wrong diagnostic: %s\n' "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    exit 1
  }
}

write_fixture_manifest() {
  local alpha_name="$1"
  local include_gamma="$2"
  cat >"${FIXTURE_MANIFEST}" <<EOF
[package]
name = "rustc-codegen-fe2o3"
version = "0.0.0"
edition = "2024"
autotests = false

[workspace]

[lib]
path = "src/lib.rs"

[[test]]
name = "${alpha_name}"
path = "tests/alpha.rs"

[[test]]
name = "beta"
path = "nonstandard/beta.rs"
EOF
  if [[ "${include_gamma}" == yes ]]; then
    cat >>"${FIXTURE_MANIFEST}" <<'EOF'

[[test]]
name = "gamma"
path = "extra/gamma.rs"
EOF
  fi
}

# Exercise the production defaults against the real locked workspace first.
python3 "${POLICY}" check >/dev/null

FIXTURE_PACKAGE="${TEST_ROOT}/package"
readonly FIXTURE_PACKAGE
FIXTURE_MANIFEST="${FIXTURE_PACKAGE}/Cargo.toml"
readonly FIXTURE_MANIFEST
mkdir -p \
  "${FIXTURE_PACKAGE}/src" \
  "${FIXTURE_PACKAGE}/tests" \
  "${FIXTURE_PACKAGE}/nonstandard" \
  "${FIXTURE_PACKAGE}/extra"
touch \
  "${FIXTURE_PACKAGE}/src/lib.rs" \
  "${FIXTURE_PACKAGE}/tests/alpha.rs" \
  "${FIXTURE_PACKAGE}/nonstandard/beta.rs" \
  "${FIXTURE_PACKAGE}/extra/gamma.rs"
write_fixture_manifest alpha no
cargo generate-lockfile --quiet --manifest-path "${FIXTURE_MANIFEST}"

VALID="${TEST_ROOT}/valid.json"
readonly VALID
cat >"${VALID}" <<'EOF'
{"schema":1,"shards":[{"id":"01-a","tests":["alpha"]},{"id":"02-b","tests":["beta"]}]}
EOF
python3 "${POLICY}" \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check >/dev/null
asserted_list="$(
  python3 "${POLICY}" \
    --manifest "${VALID}" \
    --package-manifest "${FIXTURE_MANIFEST}" \
    list
)"
[[ "${asserted_list}" == $'01-a\n02-b' ]]
[[ "$(
  python3 "${POLICY}" \
    --manifest "${VALID}" \
    --package-manifest "${FIXTURE_MANIFEST}" \
    tests 01-a
)" == alpha ]]

DUPLICATE="${TEST_ROOT}/duplicate.json"
cat >"${DUPLICATE}" <<'EOF'
{"schema":1,"shards":[{"id":"01-a","tests":["alpha"]},{"id":"02-b","tests":["alpha","beta"]}]}
EOF
expect_policy_failure duplicate 'duplicate test target: alpha' \
  --manifest "${DUPLICATE}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

# beta exists only as an explicit Cargo test at a nonstandard source path.
[[ ! -e "${FIXTURE_PACKAGE}/tests/beta.rs" ]]
MISSING="${TEST_ROOT}/missing.json"
cat >"${MISSING}" <<'EOF'
{"schema":1,"shards":[{"id":"01-a","tests":["alpha"]}]}
EOF
expect_policy_failure missing-nonstandard-beta \
  'missing or newly unassigned test targets: beta' \
  --manifest "${MISSING}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

UNKNOWN="${TEST_ROOT}/unknown.json"
cat >"${UNKNOWN}" <<'EOF'
{"schema":1,"shards":[{"id":"01-a","tests":["alpha","gamma"]},{"id":"02-b","tests":["beta"]}]}
EOF
expect_policy_failure unknown 'unknown or renamed test targets: gamma' \
  --manifest "${UNKNOWN}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

MALFORMED="${TEST_ROOT}/malformed.json"
printf '%s\n' '{not-json' >"${MALFORMED}"
expect_policy_failure malformed-shard-manifest 'invalid JSON' \
  --manifest "${MALFORMED}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

EMPTY="${TEST_ROOT}/empty.json"
cat >"${EMPTY}" <<'EOF'
{"schema":1,"shards":[{"id":"01-a","tests":[]},{"id":"02-b","tests":["alpha","beta"]}]}
EOF
expect_policy_failure empty 'empty shard: 01-a' \
  --manifest "${EMPTY}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

write_fixture_manifest alpha yes
expect_policy_failure new-target \
  'missing or newly unassigned test targets: gamma' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check

write_fixture_manifest alpha_renamed no
expect_policy_failure renamed \
  'unknown or renamed test targets: alpha' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  check
write_fixture_manifest alpha no

if python3 "${POLICY}" \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  tests absent \
  >"${TEST_ROOT}/unknown-shard.out" 2>"${TEST_ROOT}/unknown-shard.err"; then
  printf '%s\n' 'unknown shard query unexpectedly passed' >&2
  exit 1
fi
rg -F 'unknown shard id: absent' "${TEST_ROOT}/unknown-shard.err" >/dev/null

MALFORMED_METADATA="${TEST_ROOT}/malformed-metadata.json"
printf '%s\n' '{not-json' >"${MALFORMED_METADATA}"
expect_policy_failure production-metadata-override \
  'fixture metadata cannot replace production Cargo metadata' \
  --metadata "${MALFORMED_METADATA}" \
  check
expect_policy_failure malformed-metadata 'invalid JSON' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${MALFORMED_METADATA}" \
  check

MISSING_PACKAGE_METADATA="${TEST_ROOT}/missing-package-metadata.json"
printf '%s\n' '{"version":1,"packages":[]}' >"${MISSING_PACKAGE_METADATA}"
expect_policy_failure missing-package \
  'missing rustc-codegen-fe2o3 package in Cargo metadata' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${MISSING_PACKAGE_METADATA}" \
  check

DUPLICATE_PACKAGE_METADATA="${TEST_ROOT}/duplicate-package-metadata.json"
cat >"${DUPLICATE_PACKAGE_METADATA}" <<EOF
{"version":1,"packages":[
  {"name":"rustc-codegen-fe2o3","manifest_path":"${FIXTURE_MANIFEST}"},
  {"name":"rustc-codegen-fe2o3","manifest_path":"${FIXTURE_MANIFEST}"}
]}
EOF
expect_policy_failure duplicate-package \
  'duplicate rustc-codegen-fe2o3 packages in Cargo metadata' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${DUPLICATE_PACKAGE_METADATA}" \
  check

WRONG_PACKAGE_METADATA="${TEST_ROOT}/wrong-package-metadata.json"
cat >"${WRONG_PACKAGE_METADATA}" <<EOF
{"version":1,"packages":[{"name":"rustc-codegen-fe2o3","manifest_path":"${TEST_ROOT}/elsewhere/Cargo.toml","targets":[]}]}
EOF
expect_policy_failure wrong-package-manifest \
  'Cargo metadata package manifest does not match' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${WRONG_PACKAGE_METADATA}" \
  check

MALFORMED_TARGET_METADATA="${TEST_ROOT}/malformed-target-metadata.json"
cat >"${MALFORMED_TARGET_METADATA}" <<EOF
{"version":1,"packages":[{"name":"rustc-codegen-fe2o3","manifest_path":"${FIXTURE_MANIFEST}","targets":[
  {"name":"bad-name","kind":["test"],"src_path":"${FIXTURE_PACKAGE}/tests/alpha.rs"}
]}]}
EOF
expect_policy_failure malformed-target 'malformed Cargo test target: bad-name' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${MALFORMED_TARGET_METADATA}" \
  check

DUPLICATE_TARGET_METADATA="${TEST_ROOT}/duplicate-target-metadata.json"
cat >"${DUPLICATE_TARGET_METADATA}" <<EOF
{"version":1,"packages":[{"name":"rustc-codegen-fe2o3","manifest_path":"${FIXTURE_MANIFEST}","targets":[
  {"name":"alpha","kind":["test"],"src_path":"${FIXTURE_PACKAGE}/tests/alpha.rs"},
  {"name":"alpha","kind":["test"],"src_path":"${FIXTURE_PACKAGE}/nonstandard/beta.rs"}
]}]}
EOF
expect_policy_failure duplicate-metadata-target \
  'duplicate Cargo test target: alpha' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${DUPLICATE_TARGET_METADATA}" \
  check

NO_TESTS_METADATA="${TEST_ROOT}/no-tests-metadata.json"
cat >"${NO_TESTS_METADATA}" <<EOF
{"version":1,"packages":[{"name":"rustc-codegen-fe2o3","manifest_path":"${FIXTURE_MANIFEST}","targets":[
  {"name":"rustc_codegen_fe2o3","kind":["lib"],"src_path":"${FIXTURE_PACKAGE}/src/lib.rs"}
]}]}
EOF
expect_policy_failure no-tests \
  'no Cargo integration test targets for rustc-codegen-fe2o3' \
  --manifest "${VALID}" \
  --package-manifest "${FIXTURE_MANIFEST}" \
  --metadata "${NO_TESTS_METADATA}" \
  check

BROKEN_PACKAGE="${TEST_ROOT}/broken-package"
mkdir -p "${BROKEN_PACKAGE}"
printf '%s\n' '[package' >"${BROKEN_PACKAGE}/Cargo.toml"
expect_policy_failure metadata-command-failure 'cargo metadata failed with status' \
  --manifest "${VALID}" \
  --package-manifest "${BROKEN_PACKAGE}/Cargo.toml" \
  check

bash "${ROOT}/scripts/require-ci-success.sh" success success >/dev/null
for result in failure cancelled skipped ''; do
  if bash "${ROOT}/scripts/require-ci-success.sh" success "${result}" \
    >"${TEST_ROOT}/aggregate-${result:-absent}.out" \
    2>"${TEST_ROOT}/aggregate-${result:-absent}.err"; then
    printf 'aggregate negative test unexpectedly passed: %s\n' "${result:-absent}" >&2
    exit 1
  fi
done
if bash "${ROOT}/scripts/require-ci-success.sh" >/dev/null 2>&1; then
  printf '%s\n' 'aggregate accepted an absent dependency list' >&2
  exit 1
fi

printf '%s\n' 'rustc-codegen shard policy regression passed'
