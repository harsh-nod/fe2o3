#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/fs.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef FE2O3_AUTHORITY_TEST_ONLY
#define FE2O3_AUTHORITY_TEST_ONLY 0
#endif

#if FE2O3_AUTHORITY_TEST_ONLY != 0 && FE2O3_AUTHORITY_TEST_ONLY != 1
#error "FE2O3_AUTHORITY_TEST_ONLY must be zero or one"
#endif

#if FE2O3_AUTHORITY_TEST_ONLY
#ifndef FE2O3_AUTHORITY_TEST_LAUNCHER_PATH
#error "test-only launcher path is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_EXECUTABLE_PATH
#error "test-only executable path is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_POLICY_PATH
#error "test-only policy path is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_LD_SO_PRELOAD_PATH
#error "test-only ld.so.preload path is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_EXPECTED_UID
#error "test-only expected UID is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_EXPECTED_GID
#error "test-only expected GID is required"
#endif
#ifndef FE2O3_AUTHORITY_TEST_REQUIRE_IMMUTABLE
#define FE2O3_AUTHORITY_TEST_REQUIRE_IMMUTABLE 0
#endif
#ifndef FE2O3_AUTHORITY_TEST_HANDSHAKE_TIMEOUT_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_HANDSHAKE_TIMEOUT_MILLISECONDS 2000U
#endif
#ifndef FE2O3_AUTHORITY_TEST_EXECUTION_TIMEOUT_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_EXECUTION_TIMEOUT_MILLISECONDS 5000U
#endif
#ifndef FE2O3_AUTHORITY_TEST_TERM_GRACE_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_TERM_GRACE_MILLISECONDS 100U
#endif
#ifndef FE2O3_AUTHORITY_TEST_KILL_GRACE_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_KILL_GRACE_MILLISECONDS 3000U
#endif
#ifndef FE2O3_AUTHORITY_TEST_HANDSHAKE_DELAY_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_HANDSHAKE_DELAY_MILLISECONDS 0U
#endif
#ifndef FE2O3_AUTHORITY_TEST_PREEXEC_DELAY_MILLISECONDS
#define FE2O3_AUTHORITY_TEST_PREEXEC_DELAY_MILLISECONDS 0U
#endif

#define FE2O3_LAUNCHER_PATH FE2O3_AUTHORITY_TEST_LAUNCHER_PATH
#define FE2O3_EXECUTABLE_PATH FE2O3_AUTHORITY_TEST_EXECUTABLE_PATH
#define FE2O3_POLICY_PATH FE2O3_AUTHORITY_TEST_POLICY_PATH
#define FE2O3_LD_SO_PRELOAD_PATH FE2O3_AUTHORITY_TEST_LD_SO_PRELOAD_PATH
#define FE2O3_EXPECTED_UID FE2O3_AUTHORITY_TEST_EXPECTED_UID
#define FE2O3_EXPECTED_GID FE2O3_AUTHORITY_TEST_EXPECTED_GID
#define FE2O3_REQUIRE_IMMUTABLE FE2O3_AUTHORITY_TEST_REQUIRE_IMMUTABLE
#define FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS                                  \
  FE2O3_AUTHORITY_TEST_HANDSHAKE_TIMEOUT_MILLISECONDS
#define FE2O3_EXECUTION_TIMEOUT_MILLISECONDS                                  \
  FE2O3_AUTHORITY_TEST_EXECUTION_TIMEOUT_MILLISECONDS
#define FE2O3_TERM_GRACE_MILLISECONDS                                         \
  FE2O3_AUTHORITY_TEST_TERM_GRACE_MILLISECONDS
#define FE2O3_KILL_GRACE_MILLISECONDS                                         \
  FE2O3_AUTHORITY_TEST_KILL_GRACE_MILLISECONDS
#define FE2O3_HANDSHAKE_DELAY_MILLISECONDS                                    \
  FE2O3_AUTHORITY_TEST_HANDSHAKE_DELAY_MILLISECONDS
#define FE2O3_PREEXEC_DELAY_MILLISECONDS                                      \
  FE2O3_AUTHORITY_TEST_PREEXEC_DELAY_MILLISECONDS

__attribute__((used, section(".rodata.fe2o3_test_only"))) static const char
    FE2O3_TEST_ONLY_MARKER[] = "FE2O3_AUTHORITY_TEST_ONLY_BUILD";
#else
#ifdef FE2O3_AUTHORITY_TEST_LAUNCHER_PATH
#error "test-only path override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_EXECUTABLE_PATH
#error "test-only path override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_POLICY_PATH
#error "test-only path override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_LD_SO_PRELOAD_PATH
#error "test-only path override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_EXPECTED_UID
#error "test-only owner override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_EXPECTED_GID
#error "test-only owner override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_REQUIRE_IMMUTABLE
#error "test-only immutable override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_HANDSHAKE_TIMEOUT_MILLISECONDS
#error "test-only deadline override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_EXECUTION_TIMEOUT_MILLISECONDS
#error "test-only deadline override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_TERM_GRACE_MILLISECONDS
#error "test-only deadline override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_KILL_GRACE_MILLISECONDS
#error "test-only deadline override is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_HANDSHAKE_DELAY_MILLISECONDS
#error "test-only delay is forbidden in production"
#endif
#ifdef FE2O3_AUTHORITY_TEST_PREEXEC_DELAY_MILLISECONDS
#error "test-only delay is forbidden in production"
#endif

#define FE2O3_LAUNCHER_PATH                                                   \
  "/usr/libexec/fe2o3/cargo-fe2o3-authority-launcher"
#define FE2O3_EXECUTABLE_PATH "/usr/libexec/fe2o3/cargo-fe2o3"
#define FE2O3_POLICY_PATH "/etc/fe2o3/build-authority/policy-v1"
#define FE2O3_LD_SO_PRELOAD_PATH "/etc/ld.so.preload"
#define FE2O3_EXPECTED_UID 0
#define FE2O3_EXPECTED_GID 0
#define FE2O3_REQUIRE_IMMUTABLE 1
#define FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS 5000U
#define FE2O3_EXECUTION_TIMEOUT_MILLISECONDS 3600000U
#define FE2O3_TERM_GRACE_MILLISECONDS 2000U
#define FE2O3_KILL_GRACE_MILLISECONDS 5000U
#define FE2O3_HANDSHAKE_DELAY_MILLISECONDS 0U
#define FE2O3_PREEXEC_DELAY_MILLISECONDS 0U
#endif

#if !FE2O3_AUTHORITY_TEST_ONLY && FE2O3_REQUIRE_IMMUTABLE != 1
#error "production authority objects must be immutable"
#endif

#if !FE2O3_AUTHORITY_TEST_ONLY &&                                             \
    (FE2O3_HANDSHAKE_DELAY_MILLISECONDS != 0U ||                              \
     FE2O3_PREEXEC_DELAY_MILLISECONDS != 0U)
#error "production authority launcher cannot contain test delays"
#endif

#define FE2O3_POLICY_FD 240
#define FE2O3_CAPABILITY_FD 241
#define FE2O3_EXECUTABLE_FD 242
#define FE2O3_MAX_FORWARDED_ARGUMENTS 256
#define FE2O3_MAX_ARGUMENT_BYTES 4096U
#define FE2O3_MAX_TOTAL_ARGUMENT_BYTES 65536U
#define FE2O3_MAX_POLICY_BYTES 1048576
#define FE2O3_SUPERVISION_POLL_MILLISECONDS 20U
#define FE2O3_MAX_REAPS_PER_PASS 65536U
#define FE2O3_MAX_CHILD_LIST_READS 256U
#define FE2O3_CHILD_LIST_BUFFER_BYTES 4096U

enum trusted_object_kind {
  TRUSTED_EXECUTABLE,
  TRUSTED_POLICY,
};

struct supervisor {
  pid_t leader;
  int children_fd;
  bool leader_reaped;
  int leader_status;
};

/*
 * This launcher is a pre-exec foundation, not an authoritative build service.
 * It does not provide cgroup survival after launcher SIGKILL, fs-verity, a
 * trusted caller, or post-exec cargo-fe2o3 policy integration.
 */

static volatile sig_atomic_t caught_signal = 0;

static int fail(const char *message) {
  (void)dprintf(STDERR_FILENO, "cargo-fe2o3-authority-launcher: %s\n",
                message);
  return 2;
}

static void record_signal(int signal_number) { caught_signal = signal_number; }

static int reset_signal_state(void) {
  static const int signals[] = {
      SIGHUP,  SIGINT,  SIGQUIT, SIGTERM, SIGPIPE,
      SIGALRM, SIGCHLD, SIGUSR1, SIGUSR2,
  };
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_handler = SIG_DFL;
  if (sigemptyset(&action.sa_mask) != 0) {
    return -1;
  }
  for (size_t index = 0; index < sizeof(signals) / sizeof(signals[0]);
       ++index) {
    if (sigaction(signals[index], &action, NULL) != 0) {
      return -1;
    }
  }
  sigset_t empty;
  if (sigemptyset(&empty) != 0 || sigprocmask(SIG_SETMASK, &empty, NULL) != 0) {
    return -1;
  }
  return 0;
}

static int install_signal_handlers(void) {
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_handler = record_signal;
  if (sigemptyset(&action.sa_mask) != 0) {
    return -1;
  }
  for (size_t index = 0; index < 4U; ++index) {
    const int signals[] = {SIGHUP, SIGINT, SIGTERM, SIGQUIT};
    if (sigaction(signals[index], &action, NULL) != 0) {
      return -1;
    }
  }
  struct sigaction ignore;
  memset(&ignore, 0, sizeof(ignore));
  ignore.sa_handler = SIG_IGN;
  if (sigemptyset(&ignore.sa_mask) != 0 ||
      sigaction(SIGPIPE, &ignore, NULL) != 0) {
    return -1;
  }
  return 0;
}

static bool exact_directory(const struct stat *info) {
  const bool expected_owner =
      (info->st_uid == 0 && info->st_gid == 0) ||
      (info->st_uid == FE2O3_EXPECTED_UID &&
       info->st_gid == FE2O3_EXPECTED_GID);
  return S_ISDIR(info->st_mode) && expected_owner && info->st_nlink >= 2 &&
         (info->st_mode & 07022) == 0 && (info->st_mode & 0500) == 0500;
}

static int require_immutable(int file_fd) {
#if FE2O3_REQUIRE_IMMUTABLE
  int flags = 0;
  if (ioctl(file_fd, FS_IOC_GETFLAGS, &flags) != 0 ||
      (flags & FS_IMMUTABLE_FL) == 0) {
    return -1;
  }
#else
  (void)file_fd;
#endif
  return 0;
}

static bool exact_trusted_file(const struct stat *info,
                               enum trusted_object_kind kind) {
  const mode_t expected_mode = kind == TRUSTED_EXECUTABLE ? 0555 : 0444;
  if (!S_ISREG(info->st_mode) || info->st_uid != FE2O3_EXPECTED_UID ||
      info->st_gid != FE2O3_EXPECTED_GID || info->st_nlink != 1 ||
      (info->st_mode & 07777) != expected_mode) {
    return false;
  }
  if (kind == TRUSTED_POLICY &&
      (info->st_size <= 0 || info->st_size > FE2O3_MAX_POLICY_BYTES)) {
    return false;
  }
  return true;
}

static bool canonical_absolute_path(const char *path) {
  if (path == NULL) {
    return false;
  }
  const size_t length = strlen(path);
  if (length < 2U || length >= PATH_MAX || path[0] != '/' ||
      path[length - 1U] == '/' || strstr(path, "//") != NULL) {
    return false;
  }
  const char *component = path + 1;
  while (*component != '\0') {
    const char *end = strchr(component, '/');
    const size_t component_length =
        end == NULL ? strlen(component) : (size_t)(end - component);
    if ((component_length == 1U && component[0] == '.') ||
        (component_length == 2U && component[0] == '.' &&
         component[1] == '.')) {
      return false;
    }
    if (end == NULL) {
      return true;
    }
    component = end + 1;
  }
  return false;
}

static int open_trusted_object(const char *path,
                               enum trusted_object_kind kind) {
  char copy[PATH_MAX];
  struct stat info;
  int parent_fd = -1;
  int result_fd = -1;

  if (!canonical_absolute_path(path)) {
    errno = EINVAL;
    return -1;
  }
  const size_t length = strlen(path);
  memcpy(copy, path + 1, length);

  parent_fd = open("/", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (parent_fd < 0 || fstat(parent_fd, &info) != 0 ||
      !exact_directory(&info)) {
    goto error;
  }

  char *save = NULL;
  char *component = strtok_r(copy, "/", &save);
  while (component != NULL) {
    char *next = strtok_r(NULL, "/", &save);
    if (next == NULL) {
      result_fd = openat(parent_fd, component,
                         O_RDONLY | O_NONBLOCK | O_NOFOLLOW | O_CLOEXEC);
      if (result_fd < 0 || fstat(result_fd, &info) != 0 ||
          !exact_trusted_file(&info, kind) ||
          require_immutable(result_fd) != 0) {
        goto error;
      }
      close(parent_fd);
      return result_fd;
    }

    int child_fd = openat(parent_fd, component,
                          O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (child_fd < 0 || fstat(child_fd, &info) != 0 ||
        !exact_directory(&info)) {
      if (child_fd >= 0) {
        close(child_fd);
      }
      goto error;
    }
    if (*save == '\0' && require_immutable(child_fd) != 0) {
      close(child_fd);
      goto error;
    }
    close(parent_fd);
    parent_fd = child_fd;
    component = next;
  }

  errno = EINVAL;
error:
  if (result_fd >= 0) {
    close(result_fd);
  }
  if (parent_fd >= 0) {
    close(parent_fd);
  }
  return -1;
}

static int validate_empty_ld_so_preload(void) {
  char copy[PATH_MAX];
  struct stat info;
  int parent_fd = -1;
  int file_fd = -1;

  if (!canonical_absolute_path(FE2O3_LD_SO_PRELOAD_PATH)) {
    errno = EINVAL;
    return -1;
  }
  const size_t length = strlen(FE2O3_LD_SO_PRELOAD_PATH);
  memcpy(copy, FE2O3_LD_SO_PRELOAD_PATH + 1, length);
  parent_fd = open("/", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (parent_fd < 0 || fstat(parent_fd, &info) != 0 ||
      !exact_directory(&info)) {
    goto error;
  }

  char *save = NULL;
  char *component = strtok_r(copy, "/", &save);
  while (component != NULL) {
    char *next = strtok_r(NULL, "/", &save);
    if (next == NULL) {
      file_fd = openat(parent_fd, component,
                       O_RDONLY | O_NONBLOCK | O_NOFOLLOW | O_CLOEXEC);
      if (file_fd < 0 && errno == ENOENT) {
        close(parent_fd);
        return 0;
      }
      if (file_fd < 0 || fstat(file_fd, &info) != 0 ||
          !S_ISREG(info.st_mode) || info.st_uid != FE2O3_EXPECTED_UID ||
          info.st_gid != FE2O3_EXPECTED_GID || info.st_nlink != 1 ||
          (info.st_mode & 07777) != 0644 || info.st_size != 0) {
        goto error;
      }
      close(file_fd);
      close(parent_fd);
      return 0;
    }
    int child_fd = openat(parent_fd, component,
                          O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (child_fd < 0 || fstat(child_fd, &info) != 0 ||
        !exact_directory(&info)) {
      if (child_fd >= 0) {
        close(child_fd);
      }
      goto error;
    }
    close(parent_fd);
    parent_fd = child_fd;
    component = next;
  }

  errno = EINVAL;
error:
  if (file_fd >= 0) {
    close(file_fd);
  }
  if (parent_fd >= 0) {
    close(parent_fd);
  }
  return -1;
}

static int validate_self(int launcher_fd) {
  struct stat expected;
  struct stat running;
  int running_fd = open("/proc/self/exe", O_RDONLY | O_CLOEXEC);
  if (running_fd < 0 || fstat(launcher_fd, &expected) != 0 ||
      fstat(running_fd, &running) != 0 || !S_ISREG(running.st_mode) ||
      expected.st_dev != running.st_dev || expected.st_ino != running.st_ino) {
    if (running_fd >= 0) {
      close(running_fd);
    }
    return -1;
  }
  close(running_fd);
  return 0;
}

static int close_all_inherited_descriptors(void) {
#ifdef SYS_close_range
  if (syscall(SYS_close_range, 3U, UINT_MAX, 0U) == 0) {
    return 0;
  }
  if (errno != ENOSYS && errno != EINVAL) {
    return -1;
  }
#endif
  long maximum = sysconf(_SC_OPEN_MAX);
  if (maximum < 0 || maximum > 1048576) {
    maximum = 1048576;
  }
  for (int file_fd = 3; file_fd < maximum; ++file_fd) {
    if (close(file_fd) != 0 && errno != EBADF) {
      return -1;
    }
  }
  return 0;
}

static int verify_standard_descriptor(int file_fd) {
  struct stat info;
  const int status_flags = fcntl(file_fd, F_GETFL);
  int descriptor_flags = fcntl(file_fd, F_GETFD);
  if (status_flags < 0 || descriptor_flags < 0 || fstat(file_fd, &info) != 0 ||
      (status_flags & O_PATH) != 0) {
    return -1;
  }
  const int access_mode = status_flags & O_ACCMODE;
  if ((file_fd == STDIN_FILENO && access_mode == O_WRONLY) ||
      (file_fd != STDIN_FILENO && access_mode == O_RDONLY)) {
    errno = EBADF;
    return -1;
  }
  descriptor_flags &= ~FD_CLOEXEC;
  return fcntl(file_fd, F_SETFD, descriptor_flags);
}

static int install_dev_null(int destination_fd) {
  const int access_flags =
      destination_fd == STDIN_FILENO ? O_RDONLY : O_WRONLY;
  int null_fd =
      open("/dev/null", access_flags | O_CLOEXEC | O_NOFOLLOW | O_NOCTTY);
  struct stat info;
  if (null_fd < 0 || fstat(null_fd, &info) != 0 || !S_ISCHR(info.st_mode) ||
      major(info.st_rdev) != 1U || minor(info.st_rdev) != 3U) {
    if (null_fd >= 0) {
      close(null_fd);
    }
    return -1;
  }
  if (null_fd != destination_fd) {
    if (dup3(null_fd, destination_fd, 0) < 0) {
      close(null_fd);
      return -1;
    }
    close(null_fd);
  }
  return verify_standard_descriptor(destination_fd);
}

static int normalize_standard_descriptors(void) {
  for (int file_fd = STDIN_FILENO; file_fd <= STDERR_FILENO; ++file_fd) {
    if (fcntl(file_fd, F_GETFD) >= 0) {
      if (verify_standard_descriptor(file_fd) != 0) {
        return -1;
      }
      continue;
    }
    if (errno != EBADF || install_dev_null(file_fd) != 0) {
      return -1;
    }
  }
  return 0;
}

static int normalize_initial_process_state(void) {
  const struct rlimit no_core = {
      .rlim_cur = 0,
      .rlim_max = 0,
  };
  struct rlimit observed_core;
  if (reset_signal_state() != 0 ||
      setrlimit(RLIMIT_CORE, &no_core) != 0 ||
      getrlimit(RLIMIT_CORE, &observed_core) != 0 ||
      observed_core.rlim_cur != 0 || observed_core.rlim_max != 0 ||
      prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0 ||
      prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) != 0 ||
      prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 ||
      prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1 ||
      normalize_standard_descriptors() != 0 ||
      close_all_inherited_descriptors() != 0 || clearenv() != 0 ||
      chdir("/") != 0) {
    return -1;
  }
  umask(0077);
  return 0;
}

static bool child_descriptor_is_preserved(int file_fd) {
  return file_fd == FE2O3_POLICY_FD || file_fd == FE2O3_CAPABILITY_FD ||
         file_fd == FE2O3_EXECUTABLE_FD;
}

static int close_child_descriptors(void) {
  long maximum = sysconf(_SC_OPEN_MAX);
  if (maximum < 0 || maximum > 1048576) {
    maximum = 1048576;
  }
  for (int file_fd = 3; file_fd < maximum; ++file_fd) {
    if (child_descriptor_is_preserved(file_fd)) {
      continue;
    }
    if (close(file_fd) != 0 && errno != EBADF) {
      return -1;
    }
  }
  return 0;
}

static int monotonic_milliseconds(uint64_t *value) {
  struct timespec now;
  if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < 0) {
    return -1;
  }
  *value = (uint64_t)now.tv_sec * 1000U +
           (uint64_t)now.tv_nsec / 1000000U;
  return 0;
}

static int deadline_after(uint64_t duration, uint64_t *deadline) {
  uint64_t now = 0;
  if (monotonic_milliseconds(&now) != 0 || duration > UINT64_MAX - now) {
    return -1;
  }
  *deadline = now + duration;
  return 0;
}

static int delay_milliseconds(uint64_t duration) {
  uint64_t deadline = 0;
  if (deadline_after(duration, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    const uint64_t remaining = deadline - now;
    struct timespec pause = {
        .tv_sec = (time_t)(remaining / 1000U),
        .tv_nsec = (long)(remaining % 1000U) * 1000000L,
    };
    if (nanosleep(&pause, NULL) != 0 && errno != EINTR) {
      return -1;
    }
  }
}

static int wait_until_readable(int file_fd, uint64_t timeout) {
  uint64_t deadline = 0;
  if (deadline_after(timeout, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    const uint64_t remaining = deadline - now;
    const int poll_timeout =
        remaining > (uint64_t)INT_MAX ? INT_MAX : (int)remaining;
    struct pollfd descriptor = {
        .fd = file_fd,
        .events = POLLIN,
        .revents = 0,
    };
    const int result = poll(&descriptor, 1, poll_timeout);
    if (result > 0) {
      if ((descriptor.revents & (POLLIN | POLLHUP)) != 0) {
        return 1;
      }
      errno = EIO;
      return -1;
    }
    if (result == 0) {
      return 0;
    }
    if (errno != EINTR) {
      return -1;
    }
    if (caught_signal != 0) {
      errno = EINTR;
      return -1;
    }
  }
}

static int validate_arguments(int argc, char **argv) {
  if (argc < 3 || argc > FE2O3_MAX_FORWARDED_ARGUMENTS + 2 ||
      strcmp(argv[1], "--") != 0) {
    return -1;
  }
  size_t total = 0;
  for (int index = 2; index < argc; ++index) {
    if (argv[index] == NULL) {
      return -1;
    }
    const size_t length = strnlen(argv[index], FE2O3_MAX_ARGUMENT_BYTES + 1U);
    if (length == 0U || length > FE2O3_MAX_ARGUMENT_BYTES ||
        total > FE2O3_MAX_TOTAL_ARGUMENT_BYTES - length - 1U) {
      return -1;
    }
    total += length + 1U;
  }
  return 0;
}

static int open_self_children(void) {
  char path[96];
  const int length = snprintf(path, sizeof(path), "/proc/self/task/%ld/children",
                              (long)getpid());
  if (length < 0 || (size_t)length >= sizeof(path)) {
    errno = EOVERFLOW;
    return -1;
  }
  return open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
}

static int signal_child_pid(uint64_t value, int signal_number) {
  if (value == 0U || value > (uint64_t)INT_MAX) {
    errno = EPROTO;
    return -1;
  }
  if (kill((pid_t)value, signal_number) != 0 && errno != ESRCH) {
    return -1;
  }
  return 0;
}

/* A subreaper exposes descendants that escaped process-group signaling. */
static int signal_adopted_children(int children_fd, int signal_number,
                                   bool *found_child) {
  char buffer[FE2O3_CHILD_LIST_BUFFER_BYTES];
  uint64_t pid_value = 0;
  unsigned int child_count = 0;
  bool have_digit = false;
  *found_child = false;

  if (lseek(children_fd, 0, SEEK_SET) < 0) {
    return -1;
  }
  for (unsigned int read_index = 0;
       read_index < FE2O3_MAX_CHILD_LIST_READS; ++read_index) {
    const ssize_t length = read(children_fd, buffer, sizeof(buffer));
    if (length < 0) {
      if (errno == EINTR) {
        continue;
      }
      return -1;
    }
    if (length == 0) {
      if (have_digit) {
        ++child_count;
        if (child_count > FE2O3_MAX_REAPS_PER_PASS ||
            signal_child_pid(pid_value, signal_number) != 0) {
          return -1;
        }
        *found_child = true;
      }
      return 0;
    }
    for (ssize_t index = 0; index < length; ++index) {
      const unsigned char value = (unsigned char)buffer[index];
      if (value >= (unsigned char)'0' && value <= (unsigned char)'9') {
        const uint64_t digit = (uint64_t)(value - (unsigned char)'0');
        if (pid_value > (UINT64_MAX - digit) / 10U) {
          errno = EOVERFLOW;
          return -1;
        }
        pid_value = pid_value * 10U + digit;
        have_digit = true;
      } else if (value == (unsigned char)' ' ||
                 value == (unsigned char)'\n') {
        if (have_digit) {
          ++child_count;
          if (child_count > FE2O3_MAX_REAPS_PER_PASS ||
              signal_child_pid(pid_value, signal_number) != 0) {
            return -1;
          }
          *found_child = true;
          pid_value = 0;
          have_digit = false;
        }
      } else {
        errno = EPROTO;
        return -1;
      }
    }
  }
  errno = E2BIG;
  return -1;
}

static int reap_exited_children(struct supervisor *supervisor,
                                bool *has_live_children) {
  *has_live_children = false;
  for (unsigned int index = 0; index < FE2O3_MAX_REAPS_PER_PASS; ++index) {
    int status = 0;
    const pid_t result = waitpid(-1, &status, WNOHANG);
    if (result > 0) {
      if (result == supervisor->leader) {
        supervisor->leader_reaped = true;
        supervisor->leader_status = status;
      }
      continue;
    }
    if (result == 0) {
      *has_live_children = true;
      return 0;
    }
    if (errno == EINTR) {
      continue;
    }
    if (errno == ECHILD) {
      return 0;
    }
    return -1;
  }
  errno = E2BIG;
  return -1;
}

static int poll_leader(struct supervisor *supervisor) {
  uint64_t deadline = 0;
  if (deadline_after(FE2O3_EXECUTION_TIMEOUT_MILLISECONDS, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    bool live = false;
    if (reap_exited_children(supervisor, &live) != 0) {
      return -1;
    }
    if (supervisor->leader_reaped) {
      return 1;
    }
    if (!live) {
      errno = ECHILD;
      return -1;
    }
    if (caught_signal != 0) {
      errno = EINTR;
      return -1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (delay_milliseconds(FE2O3_SUPERVISION_POLL_MILLISECONDS) != 0) {
      return -1;
    }
  }
}

static int signal_process_group(pid_t leader, int signal_number) {
  if (kill(-leader, signal_number) != 0 && errno != ESRCH) {
    return -1;
  }
  return 0;
}

static int drain_phase(struct supervisor *supervisor, int signal_number,
                       uint64_t grace) {
  uint64_t deadline = 0;
  if (deadline_after(grace, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    bool found_child = false;
    bool live = false;
    if (signal_process_group(supervisor->leader, signal_number) != 0 ||
        signal_adopted_children(supervisor->children_fd, signal_number,
                                &found_child) != 0 ||
        reap_exited_children(supervisor, &live) != 0) {
      return -1;
    }
    if (!live) {
      return 1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (delay_milliseconds(FE2O3_SUPERVISION_POLL_MILLISECONDS) != 0) {
      return -1;
    }
    (void)found_child;
  }
}

static int terminate_and_reap_tree(struct supervisor *supervisor) {
  int result =
      drain_phase(supervisor, SIGTERM, FE2O3_TERM_GRACE_MILLISECONDS);
  if (result == 1) {
    return 1;
  }
  result = drain_phase(supervisor, SIGKILL, FE2O3_KILL_GRACE_MILLISECONDS);
  if (result != 1) {
    return result;
  }
  bool live = false;
  if (reap_exited_children(supervisor, &live) != 0 || live) {
    return -1;
  }
  return 1;
}

static int install_fixed_descriptor(int source_fd, int destination_fd,
                                    bool close_on_exec) {
  if (source_fd == destination_fd) {
    int flags = fcntl(destination_fd, F_GETFD);
    if (flags < 0) {
      return -1;
    }
    if (close_on_exec) {
      flags |= FD_CLOEXEC;
    } else {
      flags &= ~FD_CLOEXEC;
    }
    return fcntl(destination_fd, F_SETFD, flags);
  }
  return dup3(source_fd, destination_fd, close_on_exec ? O_CLOEXEC : 0);
}

static int verify_child_descriptors(void) {
  const int policy_flags = fcntl(FE2O3_POLICY_FD, F_GETFL);
  const int policy_fd_flags = fcntl(FE2O3_POLICY_FD, F_GETFD);
  const int capability_fd_flags = fcntl(FE2O3_CAPABILITY_FD, F_GETFD);
  const int executable_fd_flags = fcntl(FE2O3_EXECUTABLE_FD, F_GETFD);
  int socket_type = 0;
  socklen_t socket_type_length = sizeof(socket_type);
  if (policy_flags < 0 || (policy_flags & O_ACCMODE) != O_RDONLY ||
      policy_fd_flags < 0 || (policy_fd_flags & FD_CLOEXEC) != 0 ||
      capability_fd_flags < 0 ||
      (capability_fd_flags & FD_CLOEXEC) != 0 || executable_fd_flags < 0 ||
      (executable_fd_flags & FD_CLOEXEC) == 0 ||
      getsockopt(FE2O3_CAPABILITY_FD, SOL_SOCKET, SO_TYPE, &socket_type,
                 &socket_type_length) != 0 ||
      socket_type_length != sizeof(socket_type) ||
      socket_type != SOCK_SEQPACKET) {
    return -1;
  }
  return 0;
}

static void authority_child(char **arguments, pid_t expected_parent,
                            int executable_fd, int policy_fd,
                            int capability_fd) {
  static char *const environment[] = {
      "HOME=/nonexistent", "LANG=C", "LC_ALL=C", "PATH=/nonexistent",
      "TZ=UTC", NULL,
  };
  static const char ready[] = "FE2O3-AUTHORITY-READY-V1";
  static const char allow[] = "FE2O3-AUTHORITY-EXEC-V1";
  char response[sizeof(allow)] = {0};

  if (setpgid(0, 0) != 0 || prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 ||
      getppid() != expected_parent ||
      install_fixed_descriptor(policy_fd, FE2O3_POLICY_FD, false) < 0 ||
      install_fixed_descriptor(capability_fd, FE2O3_CAPABILITY_FD, false) < 0 ||
      install_fixed_descriptor(executable_fd, FE2O3_EXECUTABLE_FD, true) < 0 ||
      close_child_descriptors() != 0 || verify_child_descriptors() != 0 ||
      clearenv() != 0 || chdir("/") != 0 ||
      delay_milliseconds(FE2O3_HANDSHAKE_DELAY_MILLISECONDS) != 0 ||
      send(FE2O3_CAPABILITY_FD, ready, sizeof(ready), MSG_NOSIGNAL) !=
          (ssize_t)sizeof(ready) ||
      wait_until_readable(FE2O3_CAPABILITY_FD,
                          FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS) != 1 ||
      recv(FE2O3_CAPABILITY_FD, response, sizeof(response), 0) !=
          (ssize_t)sizeof(response) ||
      memcmp(response, allow, sizeof(allow)) != 0 ||
      delay_milliseconds(FE2O3_PREEXEC_DELAY_MILLISECONDS) != 0 ||
      reset_signal_state() != 0) {
    _exit(fail("cannot establish bounded child authority capabilities"));
  }

  (void)syscall(SYS_execveat, FE2O3_EXECUTABLE_FD, "", arguments,
                environment, AT_EMPTY_PATH);
  _exit(fail("cannot execute retained cargo-fe2o3 object"));
}

static int child_exit_status(const struct supervisor *supervisor) {
  if (WIFEXITED(supervisor->leader_status)) {
    return WEXITSTATUS(supervisor->leader_status);
  }
  if (WIFSIGNALED(supervisor->leader_status)) {
    return 128 + WTERMSIG(supervisor->leader_status);
  }
  return fail("cargo-fe2o3 ended in an unknown state");
}

int main(int argc, char **argv) {
  if (normalize_initial_process_state() != 0) {
    return fail("cannot normalize inherited process state");
  }
  if (validate_arguments(argc, argv) != 0) {
    return fail("expected -- followed by one to 256 bounded cargo-fe2o3 arguments");
  }

  int launcher_fd = open_trusted_object(FE2O3_LAUNCHER_PATH,
                                        TRUSTED_EXECUTABLE);
  int executable_fd = open_trusted_object(FE2O3_EXECUTABLE_PATH,
                                          TRUSTED_EXECUTABLE);
  int policy_fd = open_trusted_object(FE2O3_POLICY_PATH, TRUSTED_POLICY);
  if (launcher_fd < 0 || executable_fd < 0 || policy_fd < 0 ||
      validate_self(launcher_fd) != 0) {
    if (launcher_fd >= 0) {
      close(launcher_fd);
    }
    if (executable_fd >= 0) {
      close(executable_fd);
    }
    if (policy_fd >= 0) {
      close(policy_fd);
    }
    return fail("fixed path, owner, mode, link, immutable, or self identity contract failed");
  }
  close(launcher_fd);

  if (validate_empty_ld_so_preload() != 0) {
    close(executable_fd);
    close(policy_fd);
    return fail("ld.so.preload is nonempty or violates its fixed empty-file contract");
  }

  int subreaper_state = 0;
  if (prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) != 0 ||
      prctl(PR_GET_CHILD_SUBREAPER, &subreaper_state, 0, 0, 0) != 0 ||
      subreaper_state != 1 || prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) != 0 ||
      prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1 ||
      install_signal_handlers() != 0) {
    close(executable_fd);
    close(policy_fd);
    return fail("cannot establish clean bounded supervisor state");
  }
  int children_fd = open_self_children();
  bool found_child = false;
  if (children_fd < 0 ||
      signal_adopted_children(children_fd, 0, &found_child) != 0 ||
      found_child) {
    if (children_fd >= 0) {
      close(children_fd);
    }
    close(executable_fd);
    close(policy_fd);
    return fail("cannot establish empty descendant supervision state");
  }

  int capability_fds[2] = {-1, -1};
  if (socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0,
                 capability_fds) != 0) {
    close(children_fd);
    close(executable_fd);
    close(policy_fd);
    return fail("cannot create authority capability socket");
  }

  char *child_arguments[FE2O3_MAX_FORWARDED_ARGUMENTS + 2];
  child_arguments[0] = (char *)FE2O3_EXECUTABLE_PATH;
  for (int index = 2; index < argc; ++index) {
    child_arguments[index - 1] = argv[index];
  }
  child_arguments[argc - 1] = NULL;

  const pid_t parent = getpid();
  const pid_t child = fork();
  if (child < 0) {
    close(capability_fds[0]);
    close(capability_fds[1]);
    close(children_fd);
    close(executable_fd);
    close(policy_fd);
    return fail("cannot fork bounded authority child");
  }
  if (child == 0) {
    close(capability_fds[0]);
    authority_child(child_arguments, parent, executable_fd, policy_fd,
                    capability_fds[1]);
  }
  close(capability_fds[1]);

  struct supervisor supervisor = {
      .leader = child,
      .children_fd = children_fd,
      .leader_reaped = false,
      .leader_status = 0,
  };
  static const char expected_ready[] = "FE2O3-AUTHORITY-READY-V1";
  static const char allow[] = "FE2O3-AUTHORITY-EXEC-V1";
  char ready[sizeof(expected_ready)] = {0};
  const bool child_ready =
      wait_until_readable(capability_fds[0],
                          FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS) == 1 &&
      recv(capability_fds[0], ready, sizeof(ready), 0) ==
          (ssize_t)sizeof(ready) &&
      memcmp(ready, expected_ready, sizeof(expected_ready)) == 0 &&
      getpgid(child) == child;
  if (!child_ready ||
      send(capability_fds[0], allow, sizeof(allow), MSG_NOSIGNAL) !=
          (ssize_t)sizeof(allow)) {
    close(capability_fds[0]);
    const int cleanup = terminate_and_reap_tree(&supervisor);
    close(children_fd);
    close(executable_fd);
    close(policy_fd);
    if (cleanup != 1) {
      return fail("cannot empty child tree after handshake failure");
    }
    return fail("authority child handshake exceeded its bounded contract");
  }

  const int wait_result = poll_leader(&supervisor);
  close(capability_fds[0]);
  const int cleanup = terminate_and_reap_tree(&supervisor);
  close(children_fd);
  close(executable_fd);
  close(policy_fd);
  if (cleanup != 1) {
    return fail("cannot prove cargo-fe2o3 process tree is empty");
  }
  if (wait_result == 0) {
    return fail("cargo-fe2o3 exceeded its bounded authority lifetime");
  }
  if (wait_result < 0) {
    if (caught_signal != 0) {
      return 128 + caught_signal;
    }
    return fail("cannot supervise cargo-fe2o3 process tree");
  }
  return child_exit_status(&supervisor);
}
