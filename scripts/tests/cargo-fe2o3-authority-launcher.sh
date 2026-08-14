#!/bin/bash

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
readonly WATCHDOG_SECONDS=30
readonly WATCHDOG_KILL_SECONDS=5
CURRENT_UID="$(id -u)"
readonly CURRENT_UID
CURRENT_GID="$(id -g)"
readonly CURRENT_GID
declare -A BACKGROUND_WATCHDOGS=()

watchdog_start_time() {
  local process_id="$1"
  local process_state
  local remainder
  local -a fields=()
  [[ -r "/proc/${process_id}/stat" ]] || return 1
  process_state="$(<"/proc/${process_id}/stat")"
  remainder="${process_state##*) }"
  read -r -a fields <<<"${remainder}"
  ((${#fields[@]} > 19)) || return 1
  [[ "${fields[2]}" == "${process_id}" ]] || return 1
  printf '%s\n' "${fields[19]}"
}

watchdog_identity_matches() {
  local process_id="$1"
  local expected_start="$2"
  local observed_start
  observed_start="$(watchdog_start_time "${process_id}")" || return 1
  [[ "${observed_start}" == "${expected_start}" ]]
}

watchdog_group_exists() {
  kill -0 -- "-$1" 2>/dev/null
}

forget_watchdog() {
  unset 'BACKGROUND_WATCHDOGS['"$1"']'
}

watchdog_registered() {
  [[ -n "${BACKGROUND_WATCHDOGS[$1]+registered}" ]]
}

wait_registered_watchdog() {
  local process_id="$1"
  local status=0
  wait "${process_id}" || status=$?
  forget_watchdog "${process_id}"
  return "${status}"
}

terminate_registered_watchdog() {
  local process_id="$1"
  local start_time
  local status=0
  watchdog_registered "${process_id}" || return 0
  start_time="${BACKGROUND_WATCHDOGS[${process_id}]}"
  if watchdog_identity_matches "${process_id}" "${start_time}"; then
    kill -TERM -- "-${process_id}" 2>/dev/null || true
    for _ in {1..100}; do
      watchdog_group_exists "${process_id}" || break
      sleep 0.01
    done
    if watchdog_group_exists "${process_id}"; then
      kill -KILL -- "-${process_id}" 2>/dev/null || true
    fi
  fi
  wait "${process_id}" 2>/dev/null || status=$?
  forget_watchdog "${process_id}"
  return "${status}"
}

cleanup() {
  local process_id
  for process_id in "${!BACKGROUND_WATCHDOGS[@]}"; do
    terminate_registered_watchdog "${process_id}" || true
  done
  chmod -R u+w -- "${TEST_ROOT}" 2>/dev/null || true
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

# Without --foreground, GNU timeout owns a process group and escalates every
# independently bounded adversarial command from TERM to KILL.
run_watchdog() {
  /usr/bin/timeout --signal=TERM \
    --kill-after="${WATCHDOG_KILL_SECONDS}s" \
    "${WATCHDOG_SECONDS}s" "$@"
}

start_watchdog() {
  local process_id
  local start_time=""
  /usr/bin/timeout --signal=TERM \
    --kill-after="${WATCHDOG_KILL_SECONDS}s" \
    "${WATCHDOG_SECONDS}s" "$@" &
  process_id=$!
  for _ in {1..200}; do
    if start_time="$(watchdog_start_time "${process_id}")"; then
      break
    fi
    kill -0 "${process_id}" 2>/dev/null || break
    sleep 0.005
  done
  if [[ -z "${start_time}" ]]; then
    kill -TERM "${process_id}" 2>/dev/null || true
    wait "${process_id}" 2>/dev/null || true
    fail 'cannot establish watchdog process-group identity'
  fi
  BACKGROUND_WATCHDOGS["${process_id}"]="${start_time}"
  STARTED_WATCHDOG_PID="${process_id}"
}

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local output
  if output="$(run_watchdog "$@" 2>&1)"; then
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
  local force_proc_fd_close="${11:-0}"

  run_watchdog /usr/bin/cc \
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
    "-DFE2O3_AUTHORITY_TEST_FORCE_PROC_FD_CLOSE=${force_proc_fd_close}" \
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
    for _ in {1..200}; do
      if [[ ! -e "/proc/${process_id}" ]] &&
        ! kill -0 "${process_id}" 2>/dev/null; then
        break
      fi
      sleep 0.005
    done
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

watchdog_cleanup_pids="${TEST_ROOT}/watchdog-cleanup.pids"
# shellcheck disable=SC2016  # Expanded by the independently bounded child.
start_watchdog /bin/bash -c \
  'trap "" TERM; sleep 300 & child=$!; printf "%s\n%s\n" "$$" "${child}" >"$1"; wait' \
  watchdog-cleanup "${watchdog_cleanup_pids}"
watchdog_cleanup_pid="${STARTED_WATCHDOG_PID}"
for _ in {1..200}; do
  [[ "$(wc -l <"${watchdog_cleanup_pids}" 2>/dev/null || true)" == 2 ]] && break
  sleep 0.005
done
[[ -s "${watchdog_cleanup_pids}" ]] ||
  fail 'watchdog cleanup fixture did not record its process group'
terminate_registered_watchdog "${watchdog_cleanup_pid}" || true
watchdog_registered "${watchdog_cleanup_pid}" &&
  fail 'terminated watchdog remained registered'
assert_recorded_processes_gone watchdog_cleanup "${watchdog_cleanup_pids}" 2

cat >"${FIXTURE_DIR}/cargo-fixture.c" <<'EOF'
#define _GNU_SOURCE

#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/prctl.h>
#include <sys/resource.h>
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
  errno = 0;
  const bool executable_closed =
      fcntl(242, F_GETFD) == -1 && errno == EBADF;
  return policy_status >= 0 && (policy_status & O_ACCMODE) == O_RDONLY &&
         policy_flags >= 0 && (policy_flags & FD_CLOEXEC) == 0 &&
         capability_flags >= 0 && (capability_flags & FD_CLOEXEC) == 0 &&
         getsockopt(CAPABILITY_FD, SOL_SOCKET, SO_TYPE, &socket_type,
                    &socket_type_length) == 0 &&
         socket_type_length == sizeof(socket_type) &&
         socket_type == SOCK_SEQPACKET && executable_closed;
}

static bool exact_descriptor_set(void) {
  DIR *directory = opendir("/proc/self/fd");
  if (directory == NULL) {
    return false;
  }
  const int enumeration_fd = dirfd(directory);
  bool standard[3] = {false, false, false};
  bool policy = false;
  bool capability = false;
  bool enumeration = false;
  size_t count = 0;
  errno = 0;
  for (;;) {
    struct dirent *entry = readdir(directory);
    if (entry == NULL) {
      break;
    }
    if (strcmp(entry->d_name, ".") == 0 ||
        strcmp(entry->d_name, "..") == 0) {
      continue;
    }
    char *end = NULL;
    errno = 0;
    const long value = strtol(entry->d_name, &end, 10);
    if (errno != 0 || end == entry->d_name || *end != '\0' || value < 0 ||
        value > 1024) {
      closedir(directory);
      return false;
    }
    ++count;
    if (value >= 0 && value <= 2) {
      standard[value] = true;
    } else if (value == POLICY_FD) {
      policy = true;
    } else if (value == CAPABILITY_FD) {
      capability = true;
    } else if (value == enumeration_fd) {
      enumeration = true;
    } else {
      closedir(directory);
      return false;
    }
  }
  const bool read_succeeded = errno == 0;
  const bool closed = closedir(directory) == 0;
  return read_succeeded && closed && count == 6U && standard[0] &&
         standard[1] && standard[2] && policy && capability && enumeration;
}

static bool normalized_process_state(void) {
  sigset_t mask;
  struct rlimit core_limit;
  struct rlimit descriptor_limit;
  struct rlimit process_limit;
  if (sigprocmask(SIG_SETMASK, NULL, &mask) != 0 ||
      getrlimit(RLIMIT_CORE, &core_limit) != 0 || core_limit.rlim_cur != 0 ||
      core_limit.rlim_max != 0 ||
      getrlimit(RLIMIT_NOFILE, &descriptor_limit) != 0 ||
      descriptor_limit.rlim_cur < 243 || descriptor_limit.rlim_max < 243 ||
      getrlimit(RLIMIT_NPROC, &process_limit) != 0 ||
      process_limit.rlim_cur == 0 || process_limit.rlim_max == 0 ||
      prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1) {
    return false;
  }
  for (int signal_number = 1; signal_number < NSIG; ++signal_number) {
    if (signal_number == SIGKILL || signal_number == SIGSTOP) {
      continue;
    }
    struct sigaction action;
    errno = 0;
    if (sigaction(signal_number, NULL, &action) != 0) {
      if (errno == EINVAL) {
        continue;
      }
      return false;
    }
    if (sigismember(&mask, signal_number) != 0 ||
        action.sa_handler != SIG_DFL) {
      return false;
    }
  }
  return true;
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
  if (argc < 2) {
    return fail("launcher argument contract failed");
  }
  if (!clean_environment()) {
    return fail("launcher environment contract failed");
  }
  if (!valid_capabilities()) {
    return fail("launcher capability contract failed");
  }
  if (!normalized_process_state()) {
    return fail("launcher process-state contract failed");
  }
  if (!exact_descriptor_set()) {
    return fail("launcher descriptor-set contract failed");
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
                 "capabilities=fixed\nprocess-state=normalized\n"
                 "descriptors=exact\nfd242=closed\n",
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

cat >"${FIXTURE_DIR}/process-state-shim.c" <<'EOF'
#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>

static int fail(const char *message) {
  (void)dprintf(STDERR_FILENO, "process state shim: %s\n", message);
  return 4;
}

static int poison_state(void) {
  sigset_t blocked;
  const int ignored[] = {
      SIGCHLD, SIGURG, SIGWINCH, SIGXFSZ, SIGRTMIN + 5,
  };
  for (size_t index = 0; index < sizeof(ignored) / sizeof(ignored[0]);
       ++index) {
    if (ignored[index] > SIGRTMAX || signal(ignored[index], SIG_IGN) == SIG_ERR) {
      return -1;
    }
  }
  if (sigemptyset(&blocked) != 0) {
    return -1;
  }
  const int signals[] = {
      SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGCHLD, SIGVTALRM, SIGUSR2,
      SIGRTMIN + 6,
  };
  for (size_t index = 0; index < sizeof(signals) / sizeof(signals[0]);
       ++index) {
    if (signals[index] > SIGRTMAX || sigaddset(&blocked, signals[index]) != 0) {
      return -1;
    }
  }
  if (sigprocmask(SIG_BLOCK, &blocked, NULL) != 0) {
    return -1;
  }
  int null_fd = open("/dev/null", O_RDWR | O_CLOEXEC);
  if (null_fd < 0 || dup3(null_fd, 100, 0) < 0 || dup3(null_fd, 101, 0) < 0) {
    return -1;
  }
  close(null_fd);
  return 0;
}

static int lower_descriptor_soft_limit(void) {
  struct rlimit limit;
  if (getrlimit(RLIMIT_NOFILE, &limit) != 0 || limit.rlim_max < 243) {
    return -1;
  }
  limit.rlim_cur = 64;
  return setrlimit(RLIMIT_NOFILE, &limit);
}

static int lower_descriptor_hard_limit(void) {
  const struct rlimit limit = {.rlim_cur = 64, .rlim_max = 64};
  return setrlimit(RLIMIT_NOFILE, &limit);
}

static int remove_process_creation_limit(void) {
  const struct rlimit limit = {.rlim_cur = 0, .rlim_max = 0};
  return setrlimit(RLIMIT_NPROC, &limit);
}

static int close_standard_descriptors(void) {
  for (int file_fd = STDIN_FILENO; file_fd <= STDERR_FILENO; ++file_fd) {
    if (close(file_fd) != 0 && errno != EBADF) {
      return -1;
    }
  }
  return 0;
}

static int poison_environment(void) {
  static const char *const variables[][2] = {
      {"LD_PRELOAD", "/definitely/missing-preload.so"},
      {"LD_AUDIT", "/definitely/missing-audit.so"},
      {"RUSTUP_TOOLCHAIN", "malicious"},
      {"RUSTC_WRAPPER", "/tmp/malicious-rustc-wrapper"},
      {"CARGO_HOME", "/tmp/malicious-cargo-home"},
      {"CARGO_TARGET_DIR", "/tmp/malicious-target"},
      {"CARGO_NET_GIT_FETCH_WITH_CLI", "true"},
      {"GIT_CONFIG_GLOBAL", "/tmp/malicious-gitconfig"},
      {"GIT_SSH_COMMAND", "/tmp/malicious-ssh"},
      {"SSH_ASKPASS", "/tmp/malicious-askpass"},
      {"BROWSER", "/tmp/malicious-browser"},
  };
  for (size_t index = 0; index < sizeof(variables) / sizeof(variables[0]);
       ++index) {
    if (setenv(variables[index][0], variables[index][1], 1) != 0) {
      return -1;
    }
  }
  return 0;
}

static int install_invalid_stdout(void) {
  int null_fd = open("/dev/null", O_RDONLY | O_CLOEXEC);
  if (null_fd < 0 || dup2(null_fd, STDOUT_FILENO) < 0) {
    return -1;
  }
  if (null_fd != STDOUT_FILENO) {
    close(null_fd);
  }
  return 0;
}

int main(int argc, char **argv) {
  if (argc < 4) {
    return fail("expected MODE LAUNCHER ARGS...");
  }
  if (strcmp(argv[1], "poison") == 0) {
    if (poison_state() != 0) {
      return fail("cannot install inherited signal and descriptor state");
    }
  } else if (strcmp(argv[1], "closed-stdio") == 0) {
    if (close_standard_descriptors() != 0) {
      return fail("cannot close inherited standard descriptors");
    }
  } else if (strcmp(argv[1], "invalid-stdout") == 0) {
    if (install_invalid_stdout() != 0) {
      return fail("cannot install invalid standard output");
    }
  } else if (strcmp(argv[1], "environment") == 0) {
    if (poison_environment() != 0) {
      return fail("cannot install poisoned environment");
    }
  } else if (strcmp(argv[1], "low-nofile") == 0) {
    if (lower_descriptor_soft_limit() != 0) {
      return fail("cannot lower descriptor soft limit");
    }
  } else if (strcmp(argv[1], "low-hard-nofile") == 0) {
    if (lower_descriptor_hard_limit() != 0) {
      return fail("cannot lower descriptor hard limit");
    }
  } else if (strcmp(argv[1], "zero-nproc") == 0) {
    if (remove_process_creation_limit() != 0) {
      return fail("cannot remove process creation capacity");
    }
  } else {
    return fail("unknown shim mode");
  }
  execv(argv[2], &argv[2]);
  return fail("cannot execute launcher");
}
EOF

cat >"${FIXTURE_DIR}/mutate-elf.py" <<'PY'
#!/usr/bin/python3

import os
from pathlib import Path
import struct
import sys

source, destination, mutation = sys.argv[1:]
image = bytearray(Path(source).read_bytes())
if image[:6] != b"\x7fELF\x02\x01":
    raise SystemExit("fixture is not little-endian ELF64")

program_offset = struct.unpack_from("<Q", image, 32)[0]
program_size = struct.unpack_from("<H", image, 54)[0]
program_count = struct.unpack_from("<H", image, 56)[0]

if mutation == "architecture":
    struct.pack_into("<H", image, 18, 183)
elif mutation in ("wx", "relro", "dynamic-tag", "now"):
    dynamic = None
    changed = False
    for index in range(program_count):
        offset = program_offset + index * program_size
        program_type, flags = struct.unpack_from("<II", image, offset)
        if mutation == "wx" and program_type == 1 and flags & 1:
            struct.pack_into("<I", image, offset + 4, flags | 2)
            changed = True
            break
        if mutation == "relro" and program_type == 0x6474E552:
            struct.pack_into("<I", image, offset, 0)
            changed = True
            break
        if program_type == 2:
            dynamic = (
                struct.unpack_from("<Q", image, offset + 8)[0],
                struct.unpack_from("<Q", image, offset + 32)[0],
            )
    if mutation in ("dynamic-tag", "now"):
        if dynamic is None:
            raise SystemExit("fixture has no PT_DYNAMIC")
        dynamic_offset, dynamic_size = dynamic
        wanted = 21 if mutation == "dynamic-tag" else 30
        for offset in range(dynamic_offset, dynamic_offset + dynamic_size, 16):
            tag = struct.unpack_from("<q", image, offset)[0]
            if tag == wanted:
                if mutation == "dynamic-tag":
                    struct.pack_into("<q", image, offset, 0x6FFFF000)
                else:
                    struct.pack_into("<Q", image, offset + 8, 0)
                changed = True
                break
    if not changed:
        raise SystemExit(f"cannot apply {mutation} mutation")
elif mutation == "undefined-symbol":
    section_offset = struct.unpack_from("<Q", image, 40)[0]
    section_size = struct.unpack_from("<H", image, 58)[0]
    section_count = struct.unpack_from("<H", image, 60)[0]
    changed = False
    for index in range(section_count):
        offset = section_offset + index * section_size
        section_type = struct.unpack_from("<I", image, offset + 4)[0]
        if section_type == 11:
            symbol_offset = struct.unpack_from("<Q", image, offset + 24)[0]
            image[symbol_offset + 4] = 0x10
            changed = True
            break
    if not changed:
        raise SystemExit("fixture has no dynamic symbol table")
else:
    raise SystemExit(f"unknown mutation: {mutation}")

Path(destination).write_bytes(image)
os.chmod(destination, 0o555)
PY
chmod 0555 "${FIXTURE_DIR}/mutate-elf.py"

run_watchdog /usr/bin/cc \
  -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -D_FORTIFY_SOURCE=3 -DFIXTURE_VERSION='"one"' \
  "${FIXTURE_DIR}/cargo-fixture.c" -o "${EXECUTABLE}"
run_watchdog /usr/bin/cc \
  -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -D_FORTIFY_SOURCE=3 -DFIXTURE_VERSION='"two"' \
  "${FIXTURE_DIR}/cargo-fixture.c" -o "${FIXTURE_DIR}/cargo-fe2o3-v2"
run_watchdog /usr/bin/cc \
  -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -D_FORTIFY_SOURCE=3 \
  "${FIXTURE_DIR}/process-state-shim.c" -o "${FIXTURE_DIR}/process-state-shim"
chmod 0555 \
  "${EXECUTABLE}" "${FIXTURE_DIR}/cargo-fe2o3-v2" \
  "${FIXTURE_DIR}/process-state-shim"
printf '%s' 'policy-one' >"${POLICY}"
chmod 0444 "${POLICY}"

# The production builder admits no compile-time test settings and verifies the ELF.
readonly PRODUCTION_LAUNCHER="${BUILD_DIR}/production-launcher"
run_watchdog "${BUILD}" "${PRODUCTION_LAUNCHER}" >/dev/null
run_watchdog "${BUILD}" --verify "${PRODUCTION_LAUNCHER}" >/dev/null
assert_static_pie "${PRODUCTION_LAUNCHER}"
assert_no_test_marker "${PRODUCTION_LAUNCHER}"
[[ "$(stat -c '%a:%h' "${PRODUCTION_LAUNCHER}")" == 555:1 ]] ||
  fail 'production build output mode or link count changed'
expect_failure build_relative 'usage:' "${BUILD}" relative-output
ln -s "${PRODUCTION_LAUNCHER}" "${BUILD_DIR}/output-link"
expect_failure build_symlink_output 'candidate path must not be a symlink' \
  "${BUILD}" "${BUILD_DIR}/output-link"
rm "${BUILD_DIR}/output-link"

cat >"${FIXTURE_DIR}/poison-compiler" <<EOF
#!/bin/sh
printf '%s\n' invoked >"${BUILD_DIR}/poison-compiler-invoked"
exit 99
EOF
chmod 0555 "${FIXTURE_DIR}/poison-compiler"
readonly POISONED_LAUNCHER="${BUILD_DIR}/poisoned-environment-launcher"
run_watchdog /usr/bin/env \
  AR="${FIXTURE_DIR}/poison-compiler" \
  CC="${FIXTURE_DIR}/poison-compiler" \
  CFLAGS=-include=/definitely/missing-poison-header.h \
  COMPILER_PATH="${FIXTURE_DIR}" \
  CPATH=/definitely/missing-poison-include \
  CPPFLAGS=-DPOISONED_BUILD=1 \
  GCC_EXEC_PREFIX="${FIXTURE_DIR}/" \
  HOME=/definitely/missing-poison-home \
  LDFLAGS=-Wl,--definitely-invalid-poison-option \
  LIBRARY_PATH=/definitely/missing-poison-library \
  PATH=/definitely/missing-poison-path \
  RUSTC="${FIXTURE_DIR}/poison-compiler" \
  RUSTUP_TOOLCHAIN=poison-toolchain \
  TMPDIR=/definitely/missing-poison-tmp \
  "${BUILD}" "${POISONED_LAUNCHER}" >/dev/null
[[ ! -e "${BUILD_DIR}/poison-compiler-invoked" ]] ||
  fail 'poison compiler was executed by production builder'
[[ "$(sha256sum <"${PRODUCTION_LAUNCHER}")" == \
  "$(sha256sum <"${POISONED_LAUNCHER}")" ]] ||
  fail 'compiler environment poison changed production launcher bytes'

printf '%s\n' ':' >"${FIXTURE_DIR}/bash-env"
expect_failure bash_env_boundary 'caller boundary variable must be absent: BASH_ENV' \
  /usr/bin/env BASH_ENV="${FIXTURE_DIR}/bash-env" \
  "${BUILD}" "${BUILD_DIR}/bash-env-launcher"
expect_failure loader_boundary 'caller boundary variable must be absent: LD_PRELOAD' \
  /usr/bin/env LD_PRELOAD=/definitely/missing-poison-library.so \
  "${BUILD}" "${BUILD_DIR}/loader-poison-launcher"

for mutation in architecture wx relro dynamic-tag now undefined-symbol; do
  corrupted="${BUILD_DIR}/corrupted-${mutation}"
  run_watchdog /usr/bin/python3 "${FIXTURE_DIR}/mutate-elf.py" \
    "${PRODUCTION_LAUNCHER}" "${corrupted}" "${mutation}"
  case "${mutation}" in
    architecture) expected_failure='ELF Machine' ;;
    wx | relro) expected_failure='program headers violate exact W^X' ;;
    dynamic-tag) expected_failure='unexpected dynamic tag set' ;;
    now) expected_failure='does not require RELRO-compatible immediate binding' ;;
    undefined-symbol) expected_failure='unexpected undefined dynamic symbols' ;;
  esac
  expect_failure "elf_${mutation}" "${expected_failure}" \
    "${BUILD}" --verify "${corrupted}"
done

if run_watchdog /usr/bin/cc -std=c11 -O2 -fPIE -static-pie -Werror \
  -DFE2O3_AUTHORITY_TEST_EXPECTED_UID="${CURRENT_UID}" \
  "${SOURCE}" -o "${BUILD_DIR}/forbidden-production-override" \
  >"${BUILD_DIR}/forbidden.out" 2>&1; then
  fail 'production source accepted a test-only override without test mode'
fi

compile_launcher "${BASE_LAUNCHER}"

readonly PROC_FALLBACK_LAUNCHER="${LAUNCHER_DIR}/proc-fallback-launcher"
compile_launcher "${PROC_FALLBACK_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 2000 5000 0 0 \
  "${CURRENT_UID}" "${CURRENT_GID}" 1

probe_output="$(run_watchdog "${BASE_LAUNCHER}" -- probe baseline)"
for expected in \
  'version=one' 'policy=policy-one' 'token=baseline' \
  'environment=clean' 'capabilities=fixed' 'process-state=normalized' \
  'descriptors=exact' 'fd242=closed'; do
  [[ "${probe_output}" == *"${expected}"* ]] ||
    fail "baseline probe omitted ${expected}"
done

environment_output="$(
  run_watchdog "${FIXTURE_DIR}/process-state-shim" environment \
    "${BASE_LAUNCHER}" -- probe poisoned-environment
)"
[[ "${environment_output}" == *'token=poisoned-environment'* ]] ||
  fail 'poisoned environment shim did not reach the retained executable'
[[ "${environment_output}" == *'environment=clean'* ]] ||
  fail 'launcher did not scrub the poisoned environment shim state'

shim_output="$(
  run_watchdog "${FIXTURE_DIR}/process-state-shim" poison \
    "${BASE_LAUNCHER}" -- probe inherited-state
)"
for expected in \
  'token=inherited-state' 'process-state=normalized' \
  'descriptors=exact' 'fd242=closed'; do
  [[ "${shim_output}" == *"${expected}"* ]] ||
    fail "inherited-state shim probe omitted ${expected}"
done
proc_fallback_output="$(
  run_watchdog "${FIXTURE_DIR}/process-state-shim" poison \
    "${PROC_FALLBACK_LAUNCHER}" -- probe proc-fallback
)"
for expected in \
  'token=proc-fallback' 'process-state=normalized' \
  'descriptors=exact' 'fd242=closed'; do
  [[ "${proc_fallback_output}" == *"${expected}"* ]] ||
    fail "proc descriptor fallback probe omitted ${expected}"
done

low_nofile_output="$(
  run_watchdog "${FIXTURE_DIR}/process-state-shim" low-nofile \
    "${BASE_LAUNCHER}" -- probe low-nofile
)"
[[ "${low_nofile_output}" == *'token=low-nofile'* ]] ||
  fail 'launcher did not restore the descriptor capacity required by its contract'
expect_failure low_hard_nofile 'cannot normalize inherited process state' \
  "${FIXTURE_DIR}/process-state-shim" low-hard-nofile \
  "${BASE_LAUNCHER}" -- probe rejected
expect_failure zero_nproc 'cannot normalize inherited process state' \
  "${FIXTURE_DIR}/process-state-shim" zero-nproc \
  "${BASE_LAUNCHER}" -- probe rejected

run_watchdog "${FIXTURE_DIR}/process-state-shim" closed-stdio \
  "${BASE_LAUNCHER}" -- probe closed-stdio
expect_failure invalid_standard_output 'cannot normalize inherited process state' \
  "${FIXTURE_DIR}/process-state-shim" invalid-stdout \
  "${BASE_LAUNCHER}" -- probe rejected

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
run_watchdog "${BASE_LAUNCHER}" -- probe empty-preload >/dev/null
chmod 0600 "${PRELOAD}"
expect_failure preload_mode 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
chmod 0644 "${PRELOAD}"
ln "${PRELOAD}" "${PRELOAD}.hardlink"
expect_failure preload_hardlink 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD}.hardlink"
printf '%s\n' '/tmp/malicious.so' >"${PRELOAD}"
expect_failure nonempty_preload 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD}"
ln -s /dev/null "${PRELOAD}"
expect_failure preload_symlink 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD}"

mv "${PRELOAD_DIR}" "${PRELOAD_DIR}.real"
ln -s "${PRELOAD_DIR}.real" "${PRELOAD_DIR}"
expect_failure preload_parent_symlink 'ld.so.preload is nonempty' \
  "${BASE_LAUNCHER}" -- probe rejected
rm "${PRELOAD_DIR}"
mv "${PRELOAD_DIR}.real" "${PRELOAD_DIR}"

if ((CURRENT_UID != 0)) && [[ -f /etc/odbcinst.ini ]] &&
  [[ "$(stat -c '%u:%g:%a:%h:%s' /etc/odbcinst.ini)" == 0:0:644:1:0 ]]; then
  readonly PRELOAD_OWNER_LAUNCHER="${LAUNCHER_DIR}/preload-owner-launcher"
  compile_launcher "${PRELOAD_OWNER_LAUNCHER}" \
    "${EXECUTABLE}" "${POLICY}" /etc/odbcinst.ini
  expect_failure preload_owner 'ld.so.preload is nonempty' \
    "${PRELOAD_OWNER_LAUNCHER}" -- probe rejected
fi

# The child executes and reads the retained objects even after pathname swaps.
readonly DELAYED_LAUNCHER="${LAUNCHER_DIR}/delayed-launcher"
compile_launcher "${DELAYED_LAUNCHER}" \
  "${EXECUTABLE}" "${POLICY}" "${PRELOAD}" 2000 5000 0 1000
substitution_output="${TEST_ROOT}/substitution.out"
substitution_error="${TEST_ROOT}/substitution.err"
start_watchdog "${DELAYED_LAUNCHER}" -- probe retained \
  >"${substitution_output}" 2>"${substitution_error}"
substitution_watchdog_pid="${STARTED_WATCHDOG_PID}"
wait_for_child "${substitution_watchdog_pid}" \
  "${TEST_ROOT}/substitution-launcher.pid"
substitution_launcher_pid="$(<"${TEST_ROOT}/substitution-launcher.pid")"
wait_for_child "${substitution_launcher_pid}" "${TEST_ROOT}/substitution-child.pid"
mv "${EXECUTABLE}" "${EXECUTABLE}.opened"
cp "${FIXTURE_DIR}/cargo-fe2o3-v2" "${EXECUTABLE}"
chmod 0555 "${EXECUTABLE}"
mv "${POLICY}" "${POLICY}.opened"
printf '%s' 'policy-two' >"${POLICY}"
chmod 0444 "${POLICY}"
substitution_status=0
wait_registered_watchdog "${substitution_watchdog_pid}" ||
  substitution_status=$?
watchdog_registered "${substitution_watchdog_pid}" &&
  fail 'completed substitution watchdog remained registered'
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
start_watchdog "${HANDSHAKE_LAUNCHER}" -- probe timeout \
  >"${handshake_output}" 2>&1
handshake_watchdog_pid="${STARTED_WATCHDOG_PID}"
wait_for_child "${handshake_watchdog_pid}" \
  "${TEST_ROOT}/handshake-launcher.pid"
handshake_launcher_pid="$(<"${TEST_ROOT}/handshake-launcher.pid")"
wait_for_child "${handshake_launcher_pid}" "${TEST_ROOT}/handshake-child.pid"
handshake_status=0
wait_registered_watchdog "${handshake_watchdog_pid}" || handshake_status=$?
watchdog_registered "${handshake_watchdog_pid}" &&
  fail 'completed handshake watchdog remained registered'
((handshake_status != 0)) || fail 'delayed child handshake unexpectedly succeeded'
[[ "$(<"${handshake_output}")" == *'handshake exceeded its bounded contract'* ]] ||
  fail 'handshake timeout failed for the wrong reason'
handshake_child_pid="$(<"${TEST_ROOT}/handshake-child.pid")"
[[ ! -e "/proc/${handshake_child_pid}" ]] ||
  fail "handshake timeout leaked child ${handshake_child_pid}"

# Successful leaders and timed-out leaders cannot leave setsid descendants.
descendant_pid_file="${TEST_ROOT}/descendants.pids"
run_watchdog "${BASE_LAUNCHER}" -- descendants "${descendant_pid_file}"
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
