#!/usr/bin/env bash

set -Eeuo pipefail
umask 022

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly EXECUTOR="${REPO_ROOT}/scripts/parity-oci-executor.py"
readonly LAUNCHER_SOURCE="${REPO_ROOT}/scripts/parity-oci-operator-launcher.c"
readonly BUILD_LAUNCHER="${REPO_ROOT}/scripts/build-parity-oci-operator.sh"
TEST_ROOT="$(mktemp -d "${HOME}/.fe2o3-oci-operator-test.XXXXXX")"
readonly TEST_ROOT
readonly CONFIG_ROOT="${TEST_ROOT}/operator"
readonly CONFIG="${CONFIG_ROOT}/operator-v1.tsv"
readonly CONFIG_DIGEST="${CONFIG_ROOT}/operator-v1.sha256"
REQUEST_ID="$(printf '3%.0s' {1..64})"
readonly REQUEST_ID

cleanup() {
  chmod -R u+w -- "${TEST_ROOT}" 2>/dev/null || true
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

compile_test_launcher() {
  local output="$1"
  local interpreter="$2"
  local executor="$3"
  local timeout_seconds="${4:-30}"
  /usr/bin/cc \
    -std=c11 -O2 -fPIE -pie -static \
    -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
    -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
    "-DFE2O3_LAUNCHER_PATH=\"${output}\"" \
    "-DFE2O3_INTERPRETER_PATH=\"${interpreter}\"" \
    "-DFE2O3_EXECUTOR_PATH=\"${executor}\"" \
    "-DFE2O3_EXPECTED_UID=$(id -u)" \
    "-DFE2O3_EXPECTED_GID=$(id -g)" \
    -DFE2O3_REQUIRE_IMMUTABLE=0 \
    "-DFE2O3_CHILD_TIMEOUT_SECONDS=${timeout_seconds}" \
    "${LAUNCHER_SOURCE}" -o "${output}"
  chmod 0555 "${output}"
}

mkdir -m 755 "${CONFIG_ROOT}"
cat >"${CONFIG}" <<'EOF'
oci_operator_config_schema_version	2
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
queue_authorization_root	/var/lib/fe2o3/oci-authorizations
queue_authorization_owner_uid	0
queue_authorization_owner_gid	0
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
assert config.queue_authorization_root == "/var/lib/fe2o3/oci-authorizations"

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
    assert "fixed isolated contract" in str(error)
else:
    raise AssertionError("repository script was accepted as installed operator")
PY

readonly DEFAULT_LAUNCHER="${TEST_ROOT}/default-operator"
"${BUILD_LAUNCHER}" "${DEFAULT_LAUNCHER}" >"${TEST_ROOT}/default.sha256"
/usr/bin/file --brief -- "${DEFAULT_LAUNCHER}" | grep -F 'statically linked' >/dev/null
expect_failure uninstalled_default_launcher 'fixed executable path' \
  "${DEFAULT_LAUNCHER}" verify --request-id "${REQUEST_ID}"

readonly TEST_INTERPRETER="${TEST_ROOT}/python3"
cp -- "$(readlink -f -- "$(command -v python3)")" "${TEST_INTERPRETER}"
chmod 0555 "${TEST_INTERPRETER}"
readonly PROBE="${TEST_ROOT}/startup-probe.py"
readonly PROBE_OUTPUT="${TEST_ROOT}/startup-state"
cat >"${PROBE}" <<EOF
#!/usr/bin/python3
import os
from pathlib import Path
import sys

fds = []
for item in os.listdir("/proc/self/fd"):
    file_fd = int(item)
    if file_fd <= 2:
        continue
    try:
        os.fstat(file_fd)
    except OSError:
        continue
    fds.append(file_fd)
fds.sort()
parent = os.readlink(f"/proc/{os.getppid()}/exe")
old_umask = os.umask(0o077)
os.umask(old_umask)
status = Path("/proc/self/status").read_text(encoding="ascii")
no_new_privs = next(line.split()[1] for line in status.splitlines() if line.startswith("NoNewPrivs:"))
Path("${PROBE_OUTPUT}").write_text(
    repr(
        {
            "argv": sys.argv,
            "cwd": os.getcwd(),
            "env": dict(os.environ),
            "fds": fds,
            "flags": (
                sys.flags.isolated,
                sys.flags.no_site,
                sys.flags.ignore_environment,
                sys.flags.no_user_site,
            ),
            "no_new_privs": no_new_privs,
            "parent": parent,
            "stdin_rdev": os.fstat(0).st_rdev,
            "umask": old_umask,
        }
    ),
    encoding="ascii",
)
EOF
chmod 0555 "${PROBE}"

readonly TEST_LAUNCHER="${TEST_ROOT}/test-operator"
compile_test_launcher "${TEST_LAUNCHER}" "${TEST_INTERPRETER}" "${PROBE}"
mkdir -m 755 "${TEST_ROOT}/malicious" "${TEST_ROOT}/fake-bin"
readonly SITECUSTOMIZE_MARKER="${TEST_ROOT}/sitecustomize-executed"
readonly PATH_MARKER="${TEST_ROOT}/path-python-executed"
readonly PRELOAD_MARKER="${TEST_ROOT}/preload-executed"
printf 'from pathlib import Path\nPath("%s").write_text("executed")\n' \
  "${SITECUSTOMIZE_MARKER}" >"${TEST_ROOT}/malicious/sitecustomize.py"
printf '#!/usr/bin/env bash\nprintf executed > %q\nexit 97\n' \
  "${PATH_MARKER}" >"${TEST_ROOT}/fake-bin/python3"
chmod 0555 "${TEST_ROOT}/fake-bin/python3"
cat >"${TEST_ROOT}/preload.c" <<EOF
#include <fcntl.h>
#include <unistd.h>

__attribute__((constructor)) static void candidate_constructor(void) {
  int fd = open("${PRELOAD_MARKER}", O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (fd >= 0) {
    (void)write(fd, "executed\n", 9);
    (void)close(fd);
  }
}
EOF
/usr/bin/cc -shared -fPIC "${TEST_ROOT}/preload.c" \
  -o "${TEST_ROOT}/candidate-preload.so"

env \
  HOME="${TEST_ROOT}/candidate-home" \
  LC_ALL=C.UTF-8 \
  PATH="${TEST_ROOT}/fake-bin" \
  PYTHONHOME="${TEST_ROOT}/malicious" \
  PYTHONPATH="${TEST_ROOT}/malicious" \
  PYTHONSTARTUP="${TEST_ROOT}/malicious/sitecustomize.py" \
  PYTHONUSERBASE="${TEST_ROOT}/malicious" \
  LD_PRELOAD="${TEST_ROOT}/candidate-preload.so" \
  TZ=Candidate/Controlled \
  "${TEST_LAUNCHER}" verify --request-id "${REQUEST_ID}"

if [[ -e "${SITECUSTOMIZE_MARKER}" || -e "${PATH_MARKER}" || \
  -e "${PRELOAD_MARKER}" ]]; then
  printf 'caller-controlled native or Python startup code executed\n' >&2
  exit 1
fi
PYTHONDONTWRITEBYTECODE=1 python3 - \
  "${PROBE_OUTPUT}" "${TEST_LAUNCHER}" "${PROBE}" "${REQUEST_ID}" <<'PY'
import ast
import os
from pathlib import Path
import stat
import sys

state = ast.literal_eval(Path(sys.argv[1]).read_text(encoding="ascii"))
assert state["argv"] == [
    sys.argv[3],
    "--operator-internal",
    "verify",
    "--request-id",
    sys.argv[4],
]
assert state["cwd"] == "/"
assert state["env"] == {
    "HOME": "/nonexistent",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "TZ": "UTC",
}
assert state["fds"] == []
assert state["flags"] == (1, 1, 1, 1)
assert state["no_new_privs"] == "1"
assert Path(state["parent"]).samefile(sys.argv[2])
assert stat.S_ISCHR(os.stat("/dev/null").st_mode)
assert state["stdin_rdev"] == os.stat("/dev/null").st_rdev
assert state["umask"] == 0o077
PY

readonly REJECTION_LAUNCHER="${TEST_ROOT}/repository-executor-operator"
readonly REJECTION_EXECUTOR="${TEST_ROOT}/repository-executor.py"
cp -- "${EXECUTOR}" "${REJECTION_EXECUTOR}"
chmod 0555 "${REJECTION_EXECUTOR}"
compile_test_launcher \
  "${REJECTION_LAUNCHER}" "${TEST_INTERPRETER}" "${REJECTION_EXECUTOR}"
expect_failure isolated_repository_executor 'fixed isolated contract' \
  env PATH="${TEST_ROOT}/fake-bin" \
    PYTHONHOME="${TEST_ROOT}/malicious" \
    PYTHONPATH="${TEST_ROOT}/malicious" \
    PYTHONSTARTUP="${TEST_ROOT}/malicious/sitecustomize.py" \
    LD_PRELOAD="${TEST_ROOT}/candidate-preload.so" \
    "${REJECTION_LAUNCHER}" verify --request-id "${REQUEST_ID}"
if [[ -e "${SITECUSTOMIZE_MARKER}" || -e "${PATH_MARKER}" || \
  -e "${PRELOAD_MARKER}" ]]; then
  printf 'caller-controlled startup code ran before executor rejection\n' >&2
  exit 1
fi

expect_failure malformed_request_id 'expected COMMAND' \
  "${TEST_LAUNCHER}" verify --request-id ABC
ln "${TEST_LAUNCHER}" "${TEST_ROOT}/launcher-hardlink"
expect_failure launcher_hardlink 'fixed executable path' \
  "${TEST_LAUNCHER}" verify --request-id "${REQUEST_ID}"
rm "${TEST_ROOT}/launcher-hardlink"
mv "${TEST_INTERPRETER}" "${TEST_ROOT}/python3.real"
ln -s "${TEST_ROOT}/python3.real" "${TEST_INTERPRETER}"
expect_failure interpreter_symlink 'fixed executable path' \
  "${TEST_LAUNCHER}" verify --request-id "${REQUEST_ID}"
rm "${TEST_INTERPRETER}"
mv "${TEST_ROOT}/python3.real" "${TEST_INTERPRETER}"

readonly HANG_PROBE="${TEST_ROOT}/hang-probe.py"
readonly HANG_PID="${TEST_ROOT}/hang.pid"
cat >"${HANG_PROBE}" <<EOF
#!/usr/bin/python3
import os
from pathlib import Path
import signal
import time

Path("${HANG_PID}").write_text(str(os.getpid()), encoding="ascii")
signal.signal(signal.SIGTERM, signal.SIG_IGN)
while True:
    time.sleep(1)
EOF
chmod 0555 "${HANG_PROBE}"
readonly HANG_LAUNCHER="${TEST_ROOT}/hang-operator"
compile_test_launcher \
  "${HANG_LAUNCHER}" "${TEST_INTERPRETER}" "${HANG_PROBE}" 1
expect_failure bounded_launcher_wait 'exceeded bounded launcher lifetime' \
  "${HANG_LAUNCHER}" verify --request-id "${REQUEST_ID}"
if [[ ! -s "${HANG_PID}" ]] || kill -0 "$(<"${HANG_PID}")" 2>/dev/null; then
  printf 'bounded launcher left its interruptible child alive\n' >&2
  exit 1
fi

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
