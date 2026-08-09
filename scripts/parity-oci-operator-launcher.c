#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/fs.h>
#include <linux/magic.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/vfs.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef FE2O3_LAUNCHER_PATH
#define FE2O3_LAUNCHER_PATH "/usr/libexec/fe2o3-oci-operator"
#endif

#ifndef FE2O3_INTERPRETER_PATH
#define FE2O3_INTERPRETER_PATH "/usr/libexec/fe2o3-python/bin/python3"
#endif

#ifndef FE2O3_EXECUTOR_PATH
#define FE2O3_EXECUTOR_PATH "/usr/libexec/fe2o3-oci-executor.py"
#endif

#ifndef FE2O3_EXPECTED_UID
#define FE2O3_EXPECTED_UID 0
#endif

#ifndef FE2O3_EXPECTED_GID
#define FE2O3_EXPECTED_GID 0
#endif

#ifndef FE2O3_REQUIRE_IMMUTABLE
#define FE2O3_REQUIRE_IMMUTABLE 1
#endif

#ifndef FE2O3_CHILD_TIMEOUT_SECONDS
#define FE2O3_CHILD_TIMEOUT_SECONDS 900
#endif

#ifndef FE2O3_TERM_GRACE_MILLISECONDS
#define FE2O3_TERM_GRACE_MILLISECONDS 2000
#endif

#ifndef FE2O3_KILL_GRACE_MILLISECONDS
#define FE2O3_KILL_GRACE_MILLISECONDS 5000
#endif

#ifndef FE2O3_CGROUP_ROOT
#define FE2O3_CGROUP_ROOT "/sys/fs/cgroup/fe2o3-oci-operator"
#endif

#ifndef FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION
#define FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION 0
#endif

#ifndef FE2O3_TEST_ONLY_CGROUP_MECHANISM
#define FE2O3_TEST_ONLY_CGROUP_MECHANISM 0
#endif

#if FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION != 0 &&                      \
    FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION != 1
#error "FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION must be zero or one"
#endif

#if FE2O3_TEST_ONLY_CGROUP_MECHANISM != 0 &&                               \
    FE2O3_TEST_ONLY_CGROUP_MECHANISM != 1
#error "FE2O3_TEST_ONLY_CGROUP_MECHANISM must be zero or one"
#endif

#if FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION &&                         \
    FE2O3_TEST_ONLY_CGROUP_MECHANISM
#error "test execution cannot be both uncontained and mechanism-only"
#endif

#define FE2O3_SUPERVISION_POLL_MILLISECONDS 50U
#define FE2O3_STARTUP_HANDSHAKE_MILLISECONDS 5000U
#define FE2O3_MAX_REAPS_PER_PASS 65536U
#define FE2O3_MAX_CHILD_LIST_READS 256U
#define FE2O3_CHILD_LIST_BUFFER_BYTES 4096U

struct supervisor {
  pid_t leader;
  int children_fd;
  bool leader_reaped;
  int leader_status;
};

struct cgroup_containment {
  int root_fd;
  int events_fd;
  int kill_fd;
  int procs_fd;
  char leaf_name[64];
  bool child_migrated;
};

static int fail(const char *message) {
  (void)dprintf(STDERR_FILENO, "fe2o3-oci-operator: %s\n", message);
  return 2;
}

static bool valid_request_id(const char *value) {
  if (value == NULL || strlen(value) != 64) {
    return false;
  }
  for (size_t index = 0; index < 64; ++index) {
    if (!((value[index] >= '0' && value[index] <= '9') ||
          (value[index] >= 'a' && value[index] <= 'f'))) {
      return false;
    }
  }
  return true;
}

static bool valid_command(const char *value) {
  return strcmp(value, "verify") == 0 || strcmp(value, "plan") == 0 ||
         strcmp(value, "preflight") == 0;
}

static bool trusted_directory(const struct stat *info) {
  const bool expected_owner = (info->st_uid == 0 && info->st_gid == 0) ||
                              (info->st_uid == FE2O3_EXPECTED_UID &&
                               info->st_gid == FE2O3_EXPECTED_GID);
  return S_ISDIR(info->st_mode) && expected_owner &&
         (info->st_mode & 0022) == 0;
}

static bool trusted_file(const struct stat *info) {
  return S_ISREG(info->st_mode) && info->st_nlink == 1 &&
         info->st_uid == FE2O3_EXPECTED_UID &&
         info->st_gid == FE2O3_EXPECTED_GID && (info->st_mode & 07022) == 0 &&
         (info->st_mode & 0111) != 0;
}

#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
static bool trusted_cgroup_control(const struct stat *info) {
  const bool expected_owner = (info->st_uid == 0 && info->st_gid == 0) ||
                              (info->st_uid == FE2O3_EXPECTED_UID &&
                               info->st_gid == FE2O3_EXPECTED_GID);
  return S_ISREG(info->st_mode) && info->st_nlink == 1 && expected_owner &&
         (info->st_mode & 0022) == 0;
}

static int open_cgroup_control(int root_fd, const char *name, int flags) {
  struct stat info;
  int file_fd = openat(root_fd, name, flags | O_NOFOLLOW | O_CLOEXEC);
  if (file_fd < 0 || fstat(file_fd, &info) != 0 ||
      !trusted_cgroup_control(&info)) {
    if (file_fd >= 0) {
      close(file_fd);
    }
    return -1;
  }
  return file_fd;
}

static int validate_cgroup_control(int root_fd, const char *name, int flags) {
  int file_fd = open_cgroup_control(root_fd, name, flags);
  if (file_fd < 0) {
    return -1;
  }
  close(file_fd);
  return 0;
}

static int open_predelegated_cgroup_root(void) {
  struct stat info;
  struct statfs filesystem;
  int root_fd = open(FE2O3_CGROUP_ROOT,
                     O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (root_fd < 0 || fstat(root_fd, &info) != 0 ||
      fstatfs(root_fd, &filesystem) != 0 ||
      filesystem.f_type != (long)CGROUP2_SUPER_MAGIC ||
      !trusted_directory(&info) ||
      faccessat(root_fd, ".", W_OK, AT_EACCESS) != 0 ||
      validate_cgroup_control(root_fd, "cgroup.events", O_RDONLY) != 0 ||
      validate_cgroup_control(root_fd, "cgroup.kill", O_WRONLY) != 0 ||
      validate_cgroup_control(root_fd, "cgroup.procs", O_WRONLY) != 0) {
    if (root_fd >= 0) {
      close(root_fd);
    }
    return -1;
  }
  return root_fd;
}
#endif

static int validate_immutable(int file_fd) {
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

static int open_validated_file(const char *path) {
  char copy[PATH_MAX];
  struct stat info;
  int parent_fd = -1;
  int result_fd = -1;

  const size_t length = strlen(path);
  if (length < 2 || length >= sizeof(copy) || path[0] != '/' ||
      path[length - 1] == '/' || strstr(path, "//") != NULL) {
    errno = EINVAL;
    return -1;
  }
  memcpy(copy, path + 1, length);

  parent_fd = open("/", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (parent_fd < 0 || fstat(parent_fd, &info) != 0 ||
      !trusted_directory(&info)) {
    goto error;
  }

  char *save = NULL;
  char *component = strtok_r(copy, "/", &save);
  while (component != NULL) {
    char *next = strtok_r(NULL, "/", &save);
    if (strcmp(component, ".") == 0 || strcmp(component, "..") == 0) {
      errno = EINVAL;
      goto error;
    }
    if (next == NULL) {
      result_fd = openat(parent_fd, component,
                         O_RDONLY | O_NONBLOCK | O_NOFOLLOW | O_CLOEXEC);
      if (result_fd < 0 || fstat(result_fd, &info) != 0 ||
          !trusted_file(&info) || validate_immutable(result_fd) != 0) {
        goto error;
      }
      close(parent_fd);
      return result_fd;
    }

    int child_fd = openat(parent_fd, component,
                          O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (child_fd < 0 || fstat(child_fd, &info) != 0 ||
        !trusted_directory(&info)) {
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
  if (result_fd >= 0) {
    close(result_fd);
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
      fstat(running_fd, &running) != 0 || expected.st_dev != running.st_dev ||
      expected.st_ino != running.st_ino) {
    if (running_fd >= 0) {
      close(running_fd);
    }
    return -1;
  }
  close(running_fd);
  return 0;
}

static int close_inherited_descriptors_except(int preserved_fd) {
#ifdef SYS_close_range
  if (preserved_fd < 3) {
    if (syscall(SYS_close_range, 3U, UINT_MAX, 0U) == 0) {
      return 0;
    }
  } else {
    const unsigned int preserved = (unsigned int)preserved_fd;
    const int before_result =
        preserved == 3U ? 0 : (int)syscall(SYS_close_range, 3U,
                                          preserved - 1U, 0U);
    const int before_errno = errno;
    const int after_result =
        (int)syscall(SYS_close_range, preserved + 1U, UINT_MAX, 0U);
    const int after_errno = errno;
    if (before_result == 0 && after_result == 0) {
      return 0;
    }
    if ((before_result != 0 && before_errno != ENOSYS &&
         before_errno != EINVAL) ||
        (after_result != 0 && after_errno != ENOSYS &&
         after_errno != EINVAL)) {
      errno = before_result != 0 ? before_errno : after_errno;
      return -1;
    }
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
    if (file_fd == preserved_fd) {
      continue;
    }
    if (close(file_fd) != 0 && errno != EBADF) {
      return -1;
    }
  }
  return 0;
}

static int install_clean_environment(void) {
  if (clearenv() != 0 || setenv("HOME", "/nonexistent", 1) != 0 ||
      setenv("LC_ALL", "C", 1) != 0 ||
      setenv("PATH", "/usr/bin:/bin", 1) != 0 || setenv("TZ", "UTC", 1) != 0) {
    return -1;
  }
  return 0;
}

static int monotonic_milliseconds(uint64_t *value) {
  struct timespec now;
  if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < 0) {
    return -1;
  }
  *value = (uint64_t)now.tv_sec * 1000U + (uint64_t)now.tv_nsec / 1000000U;
  return 0;
}

static int deadline_after(uint64_t duration_milliseconds, uint64_t *deadline) {
  uint64_t now = 0;
  if (monotonic_milliseconds(&now) != 0 ||
      duration_milliseconds > UINT64_MAX - now) {
    return -1;
  }
  *deadline = now + duration_milliseconds;
  return 0;
}

static int pause_supervision(void) {
  const struct timespec pause = {
      .tv_sec = 0,
      .tv_nsec =
          (long)FE2O3_SUPERVISION_POLL_MILLISECONDS * 1000000L,
  };
  if (nanosleep(&pause, NULL) != 0 && errno != EINTR) {
    return -1;
  }
  return 0;
}

static int wait_until_readable(int file_fd, uint64_t timeout_milliseconds) {
  uint64_t deadline = 0;
  if (deadline_after(timeout_milliseconds, &deadline) != 0) {
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
  }
}

#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
static void initialize_containment(struct cgroup_containment *containment) {
  containment->root_fd = -1;
  containment->events_fd = -1;
  containment->kill_fd = -1;
  containment->procs_fd = -1;
  containment->leaf_name[0] = '\0';
  containment->child_migrated = false;
}

static int read_cgroup_populated(int events_fd, bool *populated) {
  char buffer[256];
  if (lseek(events_fd, 0, SEEK_SET) < 0) {
    return -1;
  }
  const ssize_t length = read(events_fd, buffer, sizeof(buffer) - 1U);
  if (length <= 0 || (size_t)length >= sizeof(buffer) - 1U) {
    errno = EPROTO;
    return -1;
  }
  buffer[length] = '\0';
  const char *line = strstr(buffer, "populated ");
  if (line == NULL || (line != buffer && line[-1] != '\n') ||
      (line[10] != '0' && line[10] != '1') || line[11] != '\n' ||
      strstr(line + 1, "\npopulated ") != NULL) {
    errno = EPROTO;
    return -1;
  }
  *populated = line[10] == '1';
  return 0;
}

static int close_containment_descriptors(
    struct cgroup_containment *containment) {
  int result = 0;
  int saved_errno = 0;
  int *const descriptors[] = {&containment->procs_fd, &containment->kill_fd,
                              &containment->events_fd, &containment->root_fd};
  for (size_t index = 0; index < sizeof(descriptors) / sizeof(descriptors[0]);
       ++index) {
    if (*descriptors[index] >= 0 && close(*descriptors[index]) != 0 &&
        result == 0) {
      result = -1;
      saved_errno = errno;
    }
    *descriptors[index] = -1;
  }
  if (result != 0) {
    errno = saved_errno;
  }
  return result;
}

static int remove_containment_leaf(struct cgroup_containment *containment) {
  if (containment->leaf_name[0] == '\0') {
    return close_containment_descriptors(containment);
  }
  int result = 0;
  int saved_errno = 0;
  int *const leaf_descriptors[] = {
      &containment->procs_fd,
      &containment->kill_fd,
      &containment->events_fd,
  };
  for (size_t index = 0;
       index < sizeof(leaf_descriptors) / sizeof(leaf_descriptors[0]);
       ++index) {
    if (*leaf_descriptors[index] >= 0 && close(*leaf_descriptors[index]) != 0 &&
        result == 0) {
      result = -1;
      saved_errno = errno;
    }
    *leaf_descriptors[index] = -1;
  }
  if (unlinkat(containment->root_fd, containment->leaf_name, AT_REMOVEDIR) !=
          0 &&
      result == 0) {
    result = -1;
    saved_errno = errno;
  }
  containment->leaf_name[0] = '\0';
  if (close_containment_descriptors(containment) != 0 && result == 0) {
    return -1;
  }
  if (result != 0) {
    errno = saved_errno;
  }
  return result;
}

static int prepare_containment(struct cgroup_containment *containment) {
  struct stat info;
  struct statfs filesystem;
  initialize_containment(containment);
  containment->root_fd = open_predelegated_cgroup_root();
  const int name_length =
      snprintf(containment->leaf_name, sizeof(containment->leaf_name),
               "launcher-%ld", (long)getpid());
  if (containment->root_fd < 0 || name_length < 0 ||
      (size_t)name_length >= sizeof(containment->leaf_name) ||
      mkdirat(containment->root_fd, containment->leaf_name, 0700) != 0) {
    containment->leaf_name[0] = '\0';
    (void)close_containment_descriptors(containment);
    return -1;
  }
  int leaf_fd = openat(containment->root_fd, containment->leaf_name,
                       O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (leaf_fd < 0 || fstat(leaf_fd, &info) != 0 ||
      fstatfs(leaf_fd, &filesystem) != 0 ||
      filesystem.f_type != (long)CGROUP2_SUPER_MAGIC ||
      !trusted_directory(&info)) {
    if (leaf_fd >= 0) {
      close(leaf_fd);
    }
    (void)remove_containment_leaf(containment);
    return -1;
  }
  containment->events_fd =
      open_cgroup_control(leaf_fd, "cgroup.events", O_RDONLY);
  containment->kill_fd =
      open_cgroup_control(leaf_fd, "cgroup.kill", O_WRONLY);
  containment->procs_fd =
      open_cgroup_control(leaf_fd, "cgroup.procs", O_RDWR);
  close(leaf_fd);
  bool populated = true;
  if (containment->events_fd < 0 || containment->kill_fd < 0 ||
      containment->procs_fd < 0 ||
      read_cgroup_populated(containment->events_fd, &populated) != 0 ||
      populated) {
    (void)remove_containment_leaf(containment);
    return -1;
  }
  return 0;
}

static int migrate_child_to_containment(
    struct cgroup_containment *containment, pid_t child) {
  char expected[64];
  const int length =
      snprintf(expected, sizeof(expected), "%ld\n", (long)child);
  if (length <= 0 || (size_t)length >= sizeof(expected) ||
      write(containment->procs_fd, expected, (size_t)length) !=
          (ssize_t)length) {
    return -1;
  }
  containment->child_migrated = true;
  if (lseek(containment->procs_fd, 0, SEEK_SET) < 0) {
    return -1;
  }
  char observed[64];
  const ssize_t observed_length =
      read(containment->procs_fd, observed, sizeof(observed));
  bool populated = false;
  if (observed_length != (ssize_t)length ||
      memcmp(observed, expected, (size_t)length) != 0 ||
      read_cgroup_populated(containment->events_fd, &populated) != 0 ||
      !populated) {
    errno = EPROTO;
    return -1;
  }
  return 0;
}

static int kill_containment_and_wait_empty(
    struct cgroup_containment *containment) {
  if (!containment->child_migrated) {
    return 1;
  }
  static const char command[] = "1\n";
  if (write(containment->kill_fd, command, sizeof(command) - 1U) !=
      (ssize_t)(sizeof(command) - 1U)) {
    return -1;
  }
  uint64_t deadline = 0;
  if (deadline_after((uint64_t)FE2O3_KILL_GRACE_MILLISECONDS, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    bool populated = true;
    if (read_cgroup_populated(containment->events_fd, &populated) != 0) {
      return -1;
    }
    if (!populated) {
      return 1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (pause_supervision() != 0) {
      return -1;
    }
  }
}
#endif

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

/* A subreaper exposes descendants that escaped group signaling with setsid. */
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

static int poll_leader(struct supervisor *supervisor,
                       unsigned int timeout_seconds) {
  uint64_t deadline = 0;
  if (deadline_after((uint64_t)timeout_seconds * 1000U, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    bool has_live_children = false;
    if (reap_exited_children(supervisor, &has_live_children) != 0) {
      return -1;
    }
    if (supervisor->leader_reaped) {
      return 1;
    }
    if (!has_live_children) {
      errno = ECHILD;
      return -1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (pause_supervision() != 0) {
      return -1;
    }
  }
}

static int signal_original_group(const struct supervisor *supervisor,
                                 int signal_number) {
  if (supervisor->leader_reaped) {
    return 0;
  }
  if (kill(-supervisor->leader, signal_number) != 0 && errno != ESRCH) {
    return -1;
  }
  return 0;
}

static int drain_phase(struct supervisor *supervisor, int signal_number,
                       uint64_t grace_milliseconds) {
  uint64_t deadline = 0;
  if (deadline_after(grace_milliseconds, &deadline) != 0) {
    return -1;
  }
  for (;;) {
    bool found_child = false;
    bool has_live_children = false;
    if (signal_original_group(supervisor, signal_number) != 0 ||
        signal_adopted_children(supervisor->children_fd, signal_number,
                                &found_child) != 0 ||
        reap_exited_children(supervisor, &has_live_children) != 0) {
      return -1;
    }
    if (!has_live_children) {
      return 1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (pause_supervision() != 0) {
      return -1;
    }
    (void)found_child;
  }
}

static int terminate_and_reap_tree(struct supervisor *supervisor) {
  int result = drain_phase(supervisor, SIGTERM,
                           (uint64_t)FE2O3_TERM_GRACE_MILLISECONDS);
  if (result == 1) {
    return 1;
  }
  result = drain_phase(supervisor, SIGKILL,
                       (uint64_t)FE2O3_KILL_GRACE_MILLISECONDS);
  if (result != 1) {
    return result;
  }
  bool has_live_children = false;
  if (reap_exited_children(supervisor, &has_live_children) != 0 ||
      has_live_children) {
    return -1;
  }
  return 1;
}

static int cleanup_supervised_execution(
    struct supervisor *supervisor,
    struct cgroup_containment *containment) {
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
  const int cgroup_result = kill_containment_and_wait_empty(containment);
#else
  (void)containment;
  const int cgroup_result = 1;
#endif
  const int process_result = terminate_and_reap_tree(supervisor);
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
  const int removal_result = remove_containment_leaf(containment);
#else
  const int removal_result = 0;
#endif
  return cgroup_result == 1 && process_result == 1 && removal_result == 0 ? 1
                                                                         : -1;
}

static void child_process(const char *command, const char *request_id,
                          pid_t expected_parent, int supervision_fd) {
  static char *const environment[] = {"HOME=/nonexistent", "LC_ALL=C",
                                      "PATH=/usr/bin:/bin", "TZ=UTC", NULL};
  char *const arguments[] = {
      (char *)FE2O3_INTERPRETER_PATH,
      "-I",
      "-S",
      (char *)FE2O3_EXECUTOR_PATH,
      "--operator-internal",
      (char *)command,
      "--request-id",
      (char *)request_id,
      NULL,
  };

  const char ready = 'R';
  char acknowledged = '\0';
  if (setpgid(0, 0) != 0 || prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 ||
      getppid() != expected_parent ||
      write(supervision_fd, &ready, sizeof(ready)) != (ssize_t)sizeof(ready) ||
      read(supervision_fd, &acknowledged, sizeof(acknowledged)) !=
          (ssize_t)sizeof(acknowledged) ||
      acknowledged != 'A') {
    _exit(fail("cannot bind executor lifetime to native launcher"));
  }
  int null_fd = open("/dev/null", O_RDONLY | O_CLOEXEC);
  if (null_fd < 0 || dup2(null_fd, STDIN_FILENO) < 0) {
    _exit(fail("cannot install fixed standard input"));
  }
  if (close_inherited_descriptors_except(-1) != 0) {
    _exit(fail("cannot close inherited descriptors"));
  }
  execve(FE2O3_INTERPRETER_PATH, arguments, environment);
  _exit(fail("cannot execute fixed isolated interpreter"));
}

int main(int argc, char **argv) {
  if (argc != 4 || !valid_command(argv[1]) ||
      strcmp(argv[2], "--request-id") != 0 || !valid_request_id(argv[3])) {
    return fail("expected COMMAND --request-id 64_lowercase_hex");
  }

  int launcher_fd = open_validated_file(FE2O3_LAUNCHER_PATH);
  int interpreter_fd = open_validated_file(FE2O3_INTERPRETER_PATH);
  int executor_fd = open_validated_file(FE2O3_EXECUTOR_PATH);
  if (launcher_fd < 0 || interpreter_fd < 0 || executor_fd < 0 ||
      validate_self(launcher_fd) != 0) {
    if (launcher_fd >= 0) {
      close(launcher_fd);
    }
    if (interpreter_fd >= 0) {
      close(interpreter_fd);
    }
    if (executor_fd >= 0) {
      close(executor_fd);
    }
    return fail("fixed executable path, ownership, mode, link, or immutable "
                "contract failed");
  }
  close(launcher_fd);
  close(interpreter_fd);
  close(executor_fd);

  int subreaper_state = 0;
  if (close_inherited_descriptors_except(-1) != 0 ||
      install_clean_environment() != 0 || chdir("/") != 0 ||
      prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 ||
      prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) != 0 ||
      prctl(PR_GET_CHILD_SUBREAPER, &subreaper_state, 0, 0, 0) != 0 ||
      subreaper_state != 1) {
    return fail("cannot establish clean native launcher state");
  }
  umask(0077);

#if !FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION &&                        \
    !FE2O3_TEST_ONLY_CGROUP_MECHANISM
  return fail("production execution disabled: privilege-separated cgroup "
              "supervisor, Docker binding, and daemon cleanup are not "
              "implemented");
#endif

  struct cgroup_containment containment = {
      .root_fd = -1,
      .events_fd = -1,
      .kill_fd = -1,
      .procs_fd = -1,
      .leaf_name = "",
      .child_migrated = false,
  };
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
  if (prepare_containment(&containment) != 0) {
    return fail("test-only cgroup mechanism is unavailable");
  }
#endif

  int children_fd = open_self_children();
  bool found_child = false;
  if (children_fd < 0 ||
      signal_adopted_children(children_fd, 0, &found_child) != 0 ||
      found_child) {
    if (children_fd >= 0) {
      close(children_fd);
    }
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
    (void)remove_containment_leaf(&containment);
#endif
    return fail("cannot establish bounded descendant supervision");
  }

  int supervision_fds[2] = {-1, -1};
  if (socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0,
                 supervision_fds) != 0) {
    close(children_fd);
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
    (void)remove_containment_leaf(&containment);
#endif
    return fail("cannot establish bounded descendant supervision");
  }

  const pid_t parent = getpid();
  const pid_t child = fork();
  if (child < 0) {
    close(supervision_fds[0]);
    close(supervision_fds[1]);
    close(children_fd);
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
    (void)remove_containment_leaf(&containment);
#endif
    return fail("cannot create isolated executor process");
  }
  if (child == 0) {
    close(supervision_fds[0]);
    child_process(argv[1], argv[3], parent, supervision_fds[1]);
  }
  close(supervision_fds[1]);

  struct supervisor supervisor = {
      .leader = child,
      .children_fd = children_fd,
      .leader_reaped = false,
      .leader_status = 0,
  };
  char ready = '\0';
  const char acknowledged = 'A';
  const bool child_ready =
      wait_until_readable(supervision_fds[0],
                          FE2O3_STARTUP_HANDSHAKE_MILLISECONDS) == 1 &&
      read(supervision_fds[0], &ready, sizeof(ready)) ==
          (ssize_t)sizeof(ready) &&
      ready == 'R' && getpgid(child) == child;
  bool child_contained = child_ready;
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
  child_contained =
      child_ready && migrate_child_to_containment(&containment, child) == 0;
#endif
  if (!child_contained ||
      write(supervision_fds[0], &acknowledged, sizeof(acknowledged)) !=
          (ssize_t)sizeof(acknowledged)) {
    close(supervision_fds[0]);
    (void)cleanup_supervised_execution(&supervisor, &containment);
    close(children_fd);
    return fail("cannot migrate and verify executor cgroup before exec");
  }
  close(supervision_fds[0]);

  const int wait_result =
      poll_leader(&supervisor, FE2O3_CHILD_TIMEOUT_SECONDS);
  if (wait_result == 0) {
    const int cleanup_result =
        cleanup_supervised_execution(&supervisor, &containment);
    close(children_fd);
    if (cleanup_result != 1) {
      return fail("cannot prove timed-out executor cgroup is empty");
    }
    return fail("isolated executor exceeded bounded launcher lifetime");
  }
  if (wait_result < 0) {
    const int cleanup_result =
        cleanup_supervised_execution(&supervisor, &containment);
    close(children_fd);
    if (cleanup_result != 1) {
      return fail("cannot prove executor cgroup is empty after supervision "
                  "error");
    }
    return fail("cannot poll isolated executor process");
  }
  const int cleanup_result =
      cleanup_supervised_execution(&supervisor, &containment);
  close(children_fd);
  if (cleanup_result != 1) {
    return fail("cannot prove executor cgroup is empty after executor exit");
  }
  if (WIFEXITED(supervisor.leader_status)) {
#if FE2O3_TEST_ONLY_CGROUP_MECHANISM
    if (WEXITSTATUS(supervisor.leader_status) == 0) {
      return fail("test-only cgroup mechanism completed; no production "
                  "verdict");
    }
#endif
    return WEXITSTATUS(supervisor.leader_status);
  }
  if (WIFSIGNALED(supervisor.leader_status)) {
    return 128 + WTERMSIG(supervisor.leader_status);
  }
  return fail("isolated executor ended in an unknown state");
}
