#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/fs.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
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

static int close_inherited_descriptors(void) {
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

static int poll_child(pid_t child, unsigned int timeout_seconds, int *status) {
  uint64_t start = 0;
  if (monotonic_milliseconds(&start) != 0) {
    return -1;
  }
  const uint64_t deadline = start + (uint64_t)timeout_seconds * 1000U;
  const struct timespec pause = {.tv_sec = 0, .tv_nsec = 100000000};
  for (;;) {
    const pid_t result = waitpid(child, status, WNOHANG);
    if (result == child) {
      return 1;
    }
    if (result < 0 && errno != EINTR) {
      return -1;
    }
    uint64_t now = 0;
    if (monotonic_milliseconds(&now) != 0) {
      return -1;
    }
    if (now >= deadline) {
      return 0;
    }
    if (nanosleep(&pause, NULL) != 0 && errno != EINTR) {
      return -1;
    }
  }
}

static int terminate_and_reap(pid_t child) {
  int status = 0;
  if (kill(child, SIGTERM) != 0 && errno != ESRCH) {
    return -1;
  }
  int result = poll_child(child, 2U, &status);
  if (result != 0) {
    return result;
  }
  if (kill(child, SIGKILL) != 0 && errno != ESRCH) {
    return -1;
  }
  return poll_child(child, 5U, &status);
}

static void child_process(const char *command, const char *request_id,
                          pid_t expected_parent) {
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

  if (prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 || getppid() != expected_parent) {
    _exit(fail("cannot bind executor lifetime to native launcher"));
  }
  int null_fd = open("/dev/null", O_RDONLY | O_CLOEXEC);
  if (null_fd < 0 || dup2(null_fd, STDIN_FILENO) < 0) {
    _exit(fail("cannot install fixed standard input"));
  }
  if (close_inherited_descriptors() != 0) {
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

  if (install_clean_environment() != 0 || chdir("/") != 0 ||
      prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0) {
    return fail("cannot establish clean native launcher state");
  }
  umask(0077);

  const pid_t parent = getpid();
  const pid_t child = fork();
  if (child < 0) {
    return fail("cannot create isolated executor process");
  }
  if (child == 0) {
    child_process(argv[1], argv[3], parent);
  }
  if (close_inherited_descriptors() != 0) {
    (void)terminate_and_reap(child);
    return fail("cannot close native launcher descriptors");
  }

  int status = 0;
  const int wait_result =
      poll_child(child, FE2O3_CHILD_TIMEOUT_SECONDS, &status);
  if (wait_result == 0) {
    (void)terminate_and_reap(child);
    return fail("isolated executor exceeded bounded launcher lifetime");
  }
  if (wait_result < 0) {
    (void)terminate_and_reap(child);
    return fail("cannot poll isolated executor process");
  }
  if (WIFEXITED(status)) {
    return WEXITSTATUS(status);
  }
  if (WIFSIGNALED(status)) {
    return 128 + WTERMSIG(status);
  }
  return fail("isolated executor ended in an unknown state");
}
