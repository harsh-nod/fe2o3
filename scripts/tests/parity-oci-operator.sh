#!/usr/bin/env bash

set -Eeuo pipefail
umask 022

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly EXECUTOR="${REPO_ROOT}/scripts/parity-oci-executor.py"
readonly OPERATOR="${REPO_ROOT}/scripts/parity-oci-operator.py"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
readonly CONFIG_ROOT="${TEST_ROOT}/operator"
readonly CONFIG="${CONFIG_ROOT}/operator-v1.tsv"
readonly CONFIG_DIGEST="${CONFIG_ROOT}/operator-v1.sha256"

cleanup() {
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local output
  if output="$("$@" 2>&1)"; then
    printf 'expected %s to fail\n' "${name}" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected}"* ]]; then
    printf '%s failed for the wrong reason:\n%s\n' "${name}" "${output}" >&2
    exit 1
  fi
}

write_digest() {
  sha256sum -- "${CONFIG}" | cut -d' ' -f1 >"${CONFIG_DIGEST}"
}

mkdir -m 755 "${CONFIG_ROOT}"
cat >"${CONFIG}" <<'EOF'
oci_operator_config_schema_version	1
config_id	mi300x-gfx942-production-v1
trusted_root	/etc/fe2o3/oci-executor/trust
policy_path	policy.tsv
policy_identity	mi300x-production-policy-v1
policy_size	4096
policy_sha256	1111111111111111111111111111111111111111111111111111111111111111
trusted_owner_uid	0
trusted_owner_gid	0
trust_file_contract	linux-immutable
inbox_root	/var/lib/fe2o3/oci-inbox
inbox_owner_uid	0
inbox_owner_gid	0
request_owner_uid	2001
request_owner_gid	2001
queue_trust_sha256	2222222222222222222222222222222222222222222222222222222222222222
EOF
write_digest

PYTHONDONTWRITEBYTECODE=1 python3 - \
  "${EXECUTOR}" "${CONFIG_ROOT}" "$(id -u)" "$(id -g)" <<'PY'
import importlib.util
import os
from pathlib import Path
import sys

module_path, root_text, uid_text, gid_text = sys.argv[1:]
spec = importlib.util.spec_from_file_location("parity_oci_operator_test", module_path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

config = module.load_operator_config(
    Path(root_text),
    provision_uid=int(uid_text),
    provision_gid=int(gid_text),
    require_immutable=False,
)
assert config.config_id == "mi300x-gfx942-production-v1"
assert config.trust_file_contract == "linux-immutable"
assert config.queue_trust_digest == "2" * 64

real_fstat = module.os.fstat
failed_fds = []


def reject_config_fstat(file_fd):
    failed_fds.append(file_fd)
    raise OSError("injected operator configuration fstat failure")


module.os.fstat = reject_config_fstat
try:
    try:
        module.load_operator_config(
            Path(root_text),
            provision_uid=int(uid_text),
            provision_gid=int(gid_text),
            require_immutable=False,
        )
    except module.ExecutorError as error:
        assert "cannot open test operator configuration directory" in str(error)
    else:
        raise AssertionError("operator configuration fstat failure was accepted")
finally:
    module.os.fstat = real_fstat
assert len(failed_fds) == 1
try:
    os.fstat(failed_fds[0])
except OSError:
    pass
else:
    raise AssertionError("failed operator configuration descriptor was not closed")

try:
    module.load_operator_config()
except module.ExecutorError as error:
    assert "operator configuration" in str(error)
else:
    raise AssertionError("host unexpectedly supplied a production operator config")

try:
    module.verify_installed_operator_entrypoint()
except module.ExecutorError as error:
    assert "fixed installed entrypoint paths" in str(error)
else:
    raise AssertionError("repository script was accepted as installed operator")
PY

expect_failure repository_operator 'fixed installed path' \
  python3 "${OPERATOR}" verify --request-id "$(printf '3%.0s' {1..64})"
expect_failure production_cli_removed 'invalid choice' "${EXECUTOR}" verify

cp "${CONFIG}" "${TEST_ROOT}/config.good"
printf '# mutation\n' >>"${CONFIG}"
expect_failure mutated_config 'provisioned digest' \
  env PYTHONDONTWRITEBYTECODE=1 python3 - \
    "${EXECUTOR}" "${CONFIG_ROOT}" "$(id -u)" "$(id -g)" <<'PY'
import importlib.util
from pathlib import Path
import sys

spec = importlib.util.spec_from_file_location("operator_mutation", sys.argv[1])
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
module.load_operator_config(
    Path(sys.argv[2]),
    provision_uid=int(sys.argv[3]),
    provision_gid=int(sys.argv[4]),
    require_immutable=False,
)
PY
mv "${TEST_ROOT}/config.good" "${CONFIG}"

mv "${CONFIG}" "${TEST_ROOT}/config.real"
ln -s "${TEST_ROOT}/config.real" "${CONFIG}"
expect_failure config_symlink 'cannot open fixed operator configuration' \
  env PYTHONDONTWRITEBYTECODE=1 python3 - \
    "${EXECUTOR}" "${CONFIG_ROOT}" "$(id -u)" "$(id -g)" <<'PY'
import importlib.util
from pathlib import Path
import sys

spec = importlib.util.spec_from_file_location("operator_symlink", sys.argv[1])
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
module.load_operator_config(
    Path(sys.argv[2]),
    provision_uid=int(sys.argv[3]),
    provision_gid=int(sys.argv[4]),
    require_immutable=False,
)
PY
rm "${CONFIG}"
mv "${TEST_ROOT}/config.real" "${CONFIG}"

printf 'parity OCI operator boundary tests passed\n'
