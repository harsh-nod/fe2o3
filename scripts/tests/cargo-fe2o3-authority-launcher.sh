#!/usr/bin/env bash

set -Eeuo pipefail
umask 022

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SOURCE="${REPO_ROOT}/scripts/cargo-fe2o3-authority-launcher.c"
readonly BUILD="${REPO_ROOT}/scripts/cargo-fe2o3-authority-build.sh"
TEST_ROOT="$(mktemp -d "${HOME}/.cargo-fe2o3-authority-test.XXXXXX")"
readonly TEST_ROOT
readonly LAUNCHER_DIR="${TEST_ROOT}/launcher"
readonly EXECUTABLE_DIR="${TEST_ROOT}/executable"
readonly POLICY_DIR="${TEST_ROOT}/policy"
readonly PRELOAD_DIR="${TEST_ROOT}/preload"
readonly BUILD_DIR="${TEST_ROOT}/build"
readonly FIXTURE_DIR="${TEST_ROOT}/fixtures"
readonly EXECUTABLE="${EXECUTABLE_DIR}/cargo-fe2o3"
readonly POLICY="${POLICY_DIR}/policy-v1"
readonly PRELOAD="${PRELOAD_DIR}/ld.so.preload"
readonly BASE_LAUNCHER="${LAUNCHER_DIR}/authority-launcher"
CURRENT_UID="$(id -u)"
readonly CURRENT_UID
CURRENT_GID="$(id -g)"
readonly CURRENT_GID

cleanup() {
  chmod -R u+w -- "${TEST_ROOT}" 2>/dev/null || true
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local output
  if output="$("$@" 2>&1)"; then
    fail "expected ${name} to fail"
  fi
  if [[ "${output}" != *"${expected}"* ]]; then
    printf '%s failed for the wrong reason:\n%s\n' "${name}" "${output}" >&2
    exit 1
  fi
}

assert_static_pie() {
  local executable="$1"
  local elf_type
  elf_type="$(
    /usr/bin/readelf --file-header --wide -- "${executable}" |
      /usr/bin/awk '$1 == "Type:" { print $2 }'
  )"
  [[ "${elf_type}" == DYN ]] || fail "${executable} is not ET_DYN"
  if /usr/bin/readelf --program-headers --wide -- "${executable}" |
    /usr/bin/grep -E '^[[:space:]]*INTERP[[:space:]]' >/dev/null; then
    fail "${executable} has PT_INTERP"
  fi
  if /usr/bin/readelf --dynamic --wide -- "${executable}" |
    /usr/bin/grep -E '\((NEEDED|RPATH|RUNPATH)\)' >/dev/null; then
    fail "${executable} has a dynamic dependency or search path"
  fi
}

assert_test_marker() {
  local executable="$1"
  if ! /usr/bin/strings --all -- "${executable}" |
    /usr/bin/grep -Fx 'FE2O3_AUTHORITY_TEST_ONLY_BUILD' >/dev/null; then
    fail "${executable} lacks the test-only build marker"
  fi
}

assert_no_test_marker() {
  local executable="$1"
  if /usr/bin/strings --all -- "${executable}" |
    /usr/bin/grep -Fx 'FE2O3_AUTHORITY_TEST_ONLY_BUILD' >/dev/null; then
    fail "${executable} contains the test-only build marker"
  fi
}

compile_launcher() {
  local output="$1"
  local executable_path="${2:-${EXECUTABLE}}"
  local policy_path="${3:-${POLICY}}"
  local preload_path="${4:-${PRELOAD}}"
  local handshake_timeout="${5:-2000}"
  local execution_timeout="${6:-5000}"
  local handshake_delay="${7:-0}"
  local preexec_delay="${8:-0}"
  local expected_uid="${9:-${CURRENT_UID}}"
  local expected_gid="${10:-${CURRENT_GID}}"

  /usr/bin/cc \
    -std=c11 -O2 -fPIE -static-pie \
    -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
    -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
    -DFE2O3_AUTHORITY_TEST_ONLY=1 \
    "-DFE2O3_AUTHORITY_TEST_LAUNCHER_PATH=\"${output}\"" \
    "-DFE2O3_AUTHORITY_TEST_EXECUTABLE_PATH=\"${executable_path}\"" \
    "-DFE2O3_AUTHORITY_TEST_POLICY_PATH=\"${policy_path}\"" \
    "-DFE2O3_AUTHORITY_TEST_LD_SO_PRELOAD_PATH=\"${preload_path}\"" \
    "-DFE2O3_AUTHORITY_TEST_EXPECTED_UID=${expected_uid}" \
    "-DFE2O3_AUTHORITY_TEST_EXPECTED_GID=${expected_gid}" \
    -DFE2O3_AUTHORITY_TEST_REQUIRE_IMMUTABLE=0 \
    "-DFE2O3_AUTHORITY_TEST_HANDSHAKE_TIMEOUT_MILLISECONDS=${handshake_timeout}U" \
    "-DFE2O3_AUTHORITY_TEST_EXECUTION_TIMEOUT_MILLISECONDS=${execution_timeout}U" \
    -DFE2O3_AUTHORITY_TEST_TERM_GRACE_MILLISECONDS=100U \
    -DFE2O3_AUTHORITY_TEST_KILL_GRACE_MILLISECONDS=3000U \
    "-DFE2O3_AUTHORITY_TEST_HANDSHAKE_DELAY_MILLISECONDS=${handshake_delay}U" \
    "-DFE2O3_AUTHORITY_TEST_PREEXEC_DELAY_MILLISECONDS=${preexec_delay}U" \
    "${SOURCE}" -o "${output}"
  chmod 0555 "${output}"
  assert_static_pie "${output}"
  assert_test_marker "${output}"
}

assert_recorded_processes_gone() {
  local name="$1"
  local pid_file="$2"
  local minimum="$3"
  local -a pids=()
  local process_id
  [[ -s "${pid_file}" ]] || fail "${name} recorded no descendant PIDs"
  mapfile -t pids <"${pid_file}"
  ((${#pids[@]} >= minimum)) ||
    fail "${name} recorded fewer than ${minimum} descendant PIDs"
  for process_id in "${pids[@]}"; do
    [[ "${process_id}" =~ ^[1-9][0-9]*$ ]] ||
      fail "${name} recorded malformed PID ${process_id}"
    if [[ -e "/proc/${process_id}" ]] ||
      kill -0 "${process_id}" 2>/dev/null; then
      fail "${name} leaked descendant process ${process_id}"
    fi
  done
}

wait_for_child() {
  local parent_pid="$1"
  local child_file="$2"
  local observed=""
  for _ in {1..200}; do
    if [[ -r "/proc/${parent_pid}/task/${parent_pid}/children" ]]; then
      observed="$(<"/proc/${parent_pid}/task/${parent_pid}/children")"
      if [[ -n "${observed}" ]]; then
        printf '%s\n' "${observed%% *}" >"${child_file}"
        return 0
      fi
    fi
    sleep 0.005
  done
  fail "launcher ${parent_pid} did not expose its bounded child"
}

mkdir -m 0755 \
  "${LAUNCHER_DIR}" "${EXECUTABLE_DIR}" "${POLICY_DIR}" \
  "${PRELOAD_DIR}" "${FIXTURE_DIR}"
mkdir -m 0700 "${BUILD_DIR}"

cat >"${FIXTURE_DIR}/cargo-fixture.c" <<'EOF'
#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#ifndef FIXTURE_VERSION
#define FIXTURE_VERSION "unknown"
#endif

#define POLICY_FD 240
#define CAPABILITY_FD 241

extern char **environ;

static int fail(const char *message) {
  (void)dprintf(STDERR_FILENO, "cargo fixture: %s\n", message);
  return 3;
}

static bool clean_environment(void) {
  static const char *const expected[] = {
      "HOME=/nonexistent",
      "LANG=C",
      "LC_ALL=C",
      "PATH=/nonexistent",
      "TZ=UTC",
  };
  bool found[sizeof(expected) / sizeof(expected[0])] = {false};
  size_t count = 0;
  for (char **entry = environ; *entry != NULL; ++entry) {
    bool matched = false;
    ++count;
    for (size_t index = 0; index < sizeof(expected) / sizeof(expected[0]);
         ++index) {
      if (strcmp(*entry, expected[index]) == 0 && !found[index]) {
        found[index] = true;
        matched = true;
        break;
      }
    }
    if (!matched) {
      return false;
    }
  }
  if (count != sizeof(expected) / sizeof(expected[0])) {
    return false;
  }
  for (size_t index = 0; index < sizeof(expected) / sizeof(expected[0]);
       ++index) {
    if (!found[index]) {
      return false;
    }
  }
  return true;
}

static int read_policy(char *buffer, size_t capacity) {
  if (capacity < 2U || lseek(POLICY_FD, 0, SEEK_SET) < 0) {
    return -1;
  }
  const ssize_t length = read(POLICY_FD, buffer, capacity - 1U);
  if (length <= 0 || (size_t)length >= capacity - 1U) {
    return -1;
  }
  buffer[length] = '\0';
  return 0;
}

static bool valid_capabilities(void) {
  const int policy_status = fcntl(POLICY_FD, F_GETFL);
  const int policy_flags = fcntl(POLICY_FD, F_GETFD);
  const int capability_flags = fcntl(CAPABILITY_FD, F_GETFD);
  int socket_type = 0;
  socklen_t socket_type_length = sizeof(socket_type);
  return policy_status >= 0 && (policy_status & O_ACCMODE) == O_RDONLY &&
         policy_flags >= 0 && (policy_flags & FD_CLOEXEC) == 0 &&
         capability_flags >= 0 && (capability_flags & FD_CLOEXEC) == 0 &&
         getsockopt(CAPABILITY_FD, SOL_SOCKET, SO_TYPE, &socket_type,
                    &socket_type_length) == 0 &&
         socket_type_length == sizeof(socket_type) &&
         socket_type == SOCK_SEQPACKET;
}

static int append_pid(const char *path, pid_t process_id) {
  int file_fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
  if (file_fd < 0) {
    return -1;
  }
  const int result = dprintf(file_fd, "%ld\n", (long)process_id);
  const int saved_errno = errno;
  if (close(file_fd) != 0 || result <= 0) {
    errno = saved_errno;
    return -1;
  }
  return 0;
}

static void wait_forever(void) {
  for (;;) {
    pause();
  }
}

static int create_descendants(const char *pid_file, bool retain_leader) {
  int ready[2] = {-1, -1};
  if (pipe2(ready, O_CLOEXEC) != 0) {
    return fail("cannot create descendant fixture pipe");
  }
  const pid_t child = fork();
  if (child < 0) {
    return fail("cannot create descendant fixture child");
  }
  if (child == 0) {
    close(ready[0]);
    if (setsid() < 0 || signal(SIGTERM, SIG_IGN) == SIG_ERR ||
        append_pid(pid_file, getpid()) != 0) {
      _exit(4);
    }
    const pid_t grandchild = fork();
    if (grandchild < 0) {
      _exit(5);
    }
    if (grandchild == 0) {
      const char marker = 'G';
      if (signal(SIGTERM, SIG_IGN) == SIG_ERR ||
          append_pid(pid_file, getpid()) != 0 ||
          write(ready[1], &marker, sizeof(marker)) !=
              (ssize_t)sizeof(marker)) {
        _exit(6);
      }
      close(ready[1]);
      wait_forever();
    }
    const char marker = 'C';
    if (write(ready[1], &marker, sizeof(marker)) != (ssize_t)sizeof(marker)) {
      _exit(7);
    }
    close(ready[1]);
    wait_forever();
  }
  close(ready[1]);
  char markers[2] = {0};
  size_t offset = 0;
  while (offset < sizeof(markers)) {
    const ssize_t length =
        read(ready[0], markers + offset, sizeof(markers) - offset);
    if (length <= 0) {
      return fail("descendant fixture did not become ready");
    }
    offset += (size_t)length;
  }
  close(ready[0]);
  if (!retain_leader) {
    return 0;
  }
  if (signal(SIGTERM, SIG_IGN) == SIG_ERR) {
    return fail("cannot retain timeout fixture leader");
  }
  wait_forever();
  return 0;
}

int main(int argc, char **argv) {
  if (argc < 2 || !clean_environment() || !valid_capabilities()) {
    return fail("launcher environment or capability contract failed");
  }
  static const char executed[] = "fixture-executed";
  if (send(CAPABILITY_FD, executed, sizeof(executed), MSG_NOSIGNAL) !=
      (ssize_t)sizeof(executed)) {
    return fail("cannot use retained socket capability");
  }
  if (strcmp(argv[1], "probe") == 0) {
    char policy[256];
    if (argc != 3 || read_policy(policy, sizeof(policy)) != 0) {
      return fail("probe arguments or policy descriptor are invalid");
    }
    (void)printf("version=%s\npolicy=%s\ntoken=%s\nenvironment=clean\n"
                 "capabilities=fixed\n",
                 FIXTURE_VERSION, policy, argv[2]);
    return 0;
  }
  if (strcmp(argv[1], "descendants") == 0 && argc == 3) {
    return create_descendants(argv[2], false);
  }
  if (strcmp(argv[1], "timeout") == 0 && argc == 3) {
    return create_descendants(argv[2], true);
  }
  return fail("unknown fixture command");
}
EOF

/usr/bin/cc \
  -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -D_FORTIFY_SOURCE=3 -DFIXTURE_VERSION='"one"' \
  "${FIXTURE_DIR}/cargo-fixture.c" -o "${EXECUTABLE}"
/usr/bin/cc \
  -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -D_FORTIFY_SOURCE=3 -DFIXTURE_VERSION='"two"' \
  "${FIXTURE_DIR}/cargo-fixture.c" -o "${FIXTURE_DIR}/cargo-fe2o3-v2"
chmod 0555 "${EXECUTABLE}" "${FIXTURE_DIR}/cargo-fe2o3-v2"
printf '%s' 'policy-one' >"${POLICY}"
chmod 0444 "${POLICY}"

# The production builder admits no compile-time test settings and verifies the ELF.
readonly PRODUCTION_LAUNCHER="${BUILD_DIR}/production-launcher"
"${BUILD}" "${PRODUCTION_LAUNCHER}" >/dev/null
assert_static_pie "${PRODUCTION_LAUNCHER}"
assert_no_test_marker "${PRODUCTION_LAUNCHER}"
[[ "$(stat -c '%a:%h' "${PRODUCTION_LAUNCHER}")" == 555:1 ]] ||
  fail 'production build output mode or link count changed'
expect_failure build_relative 'usage:' "${BUILD}" relative-output
ln -s "${PRODUCTION_LAUNCHER}" "${BUILD_DIR}/output-link"
expect_failure build_symlink_output 'output path must not be a symlink' \
  "${BUILD}" "${BUILD_DIR}/output-link"
rm "${BUILD_DIR}/output-link"

if /usr/bin/cc -std=c11 -O2 -fPIE -static-pie -Werror \
  -DFE2O3_AUTHORITY_TEST_EXPECTED_UID="${CURRENT_UID}" \
  "${SOURCE}" -o "${BUILD_DIR}/forbidden-production-override" \
  >"${BUILD_DIR}/forbidden.out" 2>&1; then
  fail 'production source accepted a test-only override without test mode'
fi

compile_launcher "${BASE_LAUNCHER}"

probe_output="$(
  LD_PRELOAD=/definitely/not/a/library.so \
  LD_AUDIT=/definitely/not/an/audit.so \
  RUSTUP_TOOLCHAIN=malicious \
  RUSTC_WRAPPER=/tmp/malicious-rustc-wrapper \
  CARGO_HOME=/tmp/malicious-cargo-home \
  CARGO_TARGET_DIR=/tmp/malicious-target \
  CARGO_NET_GIT_FETCH_WITH_CLI=true \
  GIT_CONFIG_GLOBAL=/tmp/malicious-gitconfig \
  GIT_SSH_COMMAND=/tmp/malicious-ssh \
  SSH_ASKPASS=/tmp/malicious-askpass \
  BROWSER=/tmp/malicious-browser \
  "${BASE_LAUNCHER}" -- probe baseline
)"
for expected in \
  'version=one' 'policy=policy-one' 'token=baseline' \
  'environment=clean' 'capabilities=fixed'; do
  [[ "${probe_output}" == *"${expected}"* ]] ||
    fail "baseline probe omitted ${expected}"
done

# Malformed invocations are rejected before any child is created.
expect_failure argv_empty 'expected -- followed' "${BASE_LAUNCHER}"
expect_failure argv_boundary 'expected -- followed' \
  "${BASE_LAUNCHER}" probe baseline
expect_failure argv_empty_argument 'expected -- followed' \
  "${BASE_LAUNCHER}" -- ''
printf -v overlong_argument '%4097s' ''
overlong_argument="${overlong_argument// /x}"
expect_failure argv_overlong 'expected -- followed' \
  "${BASE_LAUNCHER}" -- "${overlong_argument}"

# Final and intermediate symlinks are never traversed.
mv "${EXECUTABLE}" "${EXECUTABLE}.real"
ln -s "${EXECUTABLE}.real" "${EXECUTABLE}"
expect_failure executable_symlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${EXECUTABLE}"
mv "${EXECUTABLE}.real" "${EXECUTABLE}"

chmod u+w "${POLICY}"
mv "${POLICY}" "${POLICY}.real"
ln -s "${POLICY}.real" "${POLICY}"
expect_failure policy_symlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${POLICY}"
mv "${POLICY}.real" "${POLICY}"
chmod 0444 "${POLICY}"

mv "${EXECUTABLE_DIR}" "${EXECUTABLE_DIR}.real"
ln -s "${EXECUTABLE_DIR}.real" "${EXECUTABLE_DIR}"
expect_failure executable_parent_symlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${EXECUTABLE_DIR}"
mv "${EXECUTABLE_DIR}.real" "${EXECUTABLE_DIR}"

mv "${BASE_LAUNCHER}" "${BASE_LAUNCHER}.real"
ln -s "${BASE_LAUNCHER}.real" "${BASE_LAUNCHER}"
expect_failure launcher_symlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${BASE_LAUNCHER}"
mv "${BASE_LAUNCHER}.real" "${BASE_LAUNCHER}"

# Exact final-object mode, owner, and link-count contracts are fail-closed.
chmod 0755 "${EXECUTABLE}"
expect_failure executable_mode 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
chmod 0555 "${EXECUTABLE}"
chmod u+w "${POLICY}"
expect_failure policy_mode 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
chmod 0444 "${POLICY}"
chmod 0755 "${BASE_LAUNCHER}"
expect_failure launcher_mode 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
chmod 0555 "${BASE_LAUNCHER}"

ln "${EXECUTABLE}" "${EXECUTABLE}.hardlink"
expect_failure executable_hardlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${EXECUTABLE}.hardlink"
ln "${POLICY}" "${POLICY}.hardlink"
expect_failure policy_hardlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${POLICY}.hardlink"
ln "${BASE_LAUNCHER}" "${BASE_LAUNCHER}.hardlink"
expect_failure launcher_hardlink 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${BASE_LAUNCHER}.hardlink"

chmod 0775 "${POLICY_DIR}"
expect_failure writable_policy_parent 'fixed path, owner, mode, link' \
  "${BASE_LAUNCHER}" -- probe rejected
chmod 0755 "${POLICY_DIR}"

readonly WRONG_OWNER_LAUNCHER="${LAUNCHER_DIR}/wrong-owner-launcher"
compile_launcher "${WRONG_OWNER_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 2000 5000 0 0 \
  "$((CURRENT_UID + 1))" "${CURRENT_GID}"
expect_failure wrong_owner 'fixed path, owner, mode, link' \
  "${WRONG_OWNER_LAUNCHER}" -- probe rejected

readonly NONCANONICAL_LAUNCHER="${LAUNCHER_DIR}/noncanonical-launcher"
compile_launcher "${NONCANONICAL_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY_DIR}/../policy/policy-v1"
expect_failure noncanonical_policy 'fixed path, owner, mode, link' \
  "${NONCANONICAL_LAUNCHER}" -- probe rejected

# An absent or exact empty preload file is accepted; content and symlinks fail.
: >"${PRELOAD}"
chmod 0644 "${PRELOAD}"
"${BASE_LAUNCHER}" -- probe empty-preload >/dev/null
printf '%s\n' '/tmp/malicious.so' >"${PRELOAD}"
expect_failure nonempty_preload 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD}"
ln -s /dev/null "${PRELOAD}"
expect_failure preload_symlink 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD}"

# The child executes and reads the retained objects even after pathname swaps.
readonly DELAYED_LAUNCHER="${LAUNCHER_DIR}/delayed-launcher"
compile_launcher "${DELAYED_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 2000 5000 0 1000
substitution_output="${TEST_ROOT}/substitution.out"
substitution_error="${TEST_ROOT}/substitution.err"
"${DELAYED_LAUNCHER}" -- probe retained \
  >"${substitution_output}" 2>"${substitution_error}" &
substitution_launcher_pid=$!
wait_for_child "${substitution_launcher_pid}" "${TEST_ROOT}/substitution-child.pid"
mv "${EXECUTABLE}" "${EXECUTABLE}.opened"
cp "${FIXTURE_DIR}/cargo-fe2o3-v2" "${EXECUTABLE}"
chmod 0555 "${EXECUTABLE}"
mv "${POLICY}" "${POLICY}.opened"
printf '%s' 'policy-two' >"${POLICY}"
chmod 0444 "${POLICY}"
substitution_status=0
wait "${substitution_launcher_pid}" || substitution_status=$?
if ((substitution_status != 0)); then
  printf 'substitution launcher failed:\n%s\n' \
    "$(<"${substitution_error}")" >&2
  exit 1
fi
substitution_result="$(<"${substitution_output}")"
[[ "${substitution_result}" == *'version=one'* ]] ||
  fail 'executable pathname substitution changed the retained executable'
[[ "${substitution_result}" == *'policy=policy-one'* ]] ||
  fail 'policy pathname substitution changed the retained policy object'
rm "${EXECUTABLE}" "${POLICY}"
mv "${EXECUTABLE}.opened" "${EXECUTABLE}"
mv "${POLICY}.opened" "${POLICY}"

# Parent/child startup is bounded and a delayed handshake leaves no process.
readonly HANDSHAKE_LAUNCHER="${LAUNCHER_DIR}/handshake-timeout-launcher"
compile_launcher "${HANDSHAKE_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 250 5000 1000 0
handshake_output="${TEST_ROOT}/handshake.out"
"${HANDSHAKE_LAUNCHER}" -- probe timeout >"${handshake_output}" 2>&1 &
handshake_launcher_pid=$!
wait_for_child "${handshake_launcher_pid}" "${TEST_ROOT}/handshake-child.pid"
handshake_status=0
wait "${handshake_launcher_pid}" || handshake_status=$?
((handshake_status != 0)) || fail 'delayed child handshake unexpectedly succeeded'
[[ "$(<"${handshake_output}")" == *'handshake exceeded its bounded contract'* ]] ||
  fail 'handshake timeout failed for the wrong reason'
handshake_child_pid="$(<"${TEST_ROOT}/handshake-child.pid")"
[[ ! -e "/proc/${handshake_child_pid}" ]] ||
  fail "handshake timeout leaked child ${handshake_child_pid}"

# Successful leaders and timed-out leaders cannot leave setsid descendants.
descendant_pid_file="${TEST_ROOT}/descendants.pids"
"${BASE_LAUNCHER}" -- descendants "${descendant_pid_file}"
assert_recorded_processes_gone successful_descendants \
  "${descendant_pid_file}" 2

readonly TIMEOUT_LAUNCHER="${LAUNCHER_DIR}/execution-timeout-launcher"
compile_launcher "${TIMEOUT_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 2000 300 0 0
timeout_pid_file="${TEST_ROOT}/timeout.pids"
expect_failure execution_timeout 'exceeded its bounded authority lifetime' \
  "${TIMEOUT_LAUNCHER}" -- timeout "${timeout_pid_file}"
assert_recorded_processes_gone timed_out_descendants "${timeout_pid_file}" 2

if find "${BUILD_DIR}" -maxdepth 1 -name '.*.tmp.*' -print -quit |
  grep . >/dev/null; then
  fail 'authority builder left a temporary output'
fi

printf '%s\n' \
  'cargo-fe2o3 protected build-authority launcher foundation tests passed'
