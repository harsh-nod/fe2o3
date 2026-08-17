#define _GNU_SOURCE

#include "fe2o3_static_preexec_manifest.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/memfd.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define TEST_TIMEOUT_MILLISECONDS 5000
#define REQUIRED_CONTENT_SEALS                                                 \
  (F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE)
#define REQUIRED_EXECUTABLE_SEALS (REQUIRED_CONTENT_SEALS | F_SEAL_EXEC)

enum case_kind {
  CASE_SUCCESS,
  CASE_SUCCESS_LOW_LIMIT,
  CASE_ALIAS,
  CASE_ALIAS_MUTABLE_METADATA,
  CASE_MANIFEST_EXECUTABLE_ALIAS,
  CASE_TARGET_SUBSTITUTION,
  CASE_EXTRA_SOURCE,
  CASE_MISSING_SOURCE,
  CASE_STANDARD_FD,
  CASE_DUPLICATE_DESTINATION,
  CASE_PARENT_IDENTITY,
  CASE_SIGNAL_STATE,
};

struct launch_inputs {
  int target_fd;
  int substitute_target_fd;
  int manifest_fd;
  int sources[4];
  int input_write_fd;
  int stdout_read_fd;
  int stderr_read_fd;
  int report_read_fd;
  int extra_fd;
};

static int error_message(const char *message) {
  (void)fprintf(stderr, "static-preexec-supervisor: %s: %s\n", message,
                strerror(errno));
  return 1;
}

static int write_all(int fd, const void *bytes, size_t length) {
  size_t offset = 0U;
  while (offset < length) {
    const ssize_t count =
        write(fd, (const char *)bytes + offset, length - offset);
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count <= 0) {
      return -1;
    }
    offset += (size_t)count;
  }
  return 0;
}

static int copy_to_sealed_memfd(const char *path, const char *name) {
  const int source = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (source < 0) {
    return -1;
  }
  const int destination =
      (int)syscall(SYS_memfd_create, name, MFD_CLOEXEC | MFD_ALLOW_SEALING);
  if (destination < 0) {
    const int saved_errno = errno;
    (void)close(source);
    errno = saved_errno;
    return -1;
  }
  char buffer[16384];
  for (;;) {
    const ssize_t count = read(source, buffer, sizeof(buffer));
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count < 0 ||
        (count > 0 && write_all(destination, buffer, (size_t)count) != 0)) {
      const int saved_errno = errno;
      (void)close(source);
      (void)close(destination);
      errno = saved_errno;
      return -1;
    }
    if (count == 0) {
      break;
    }
  }
  if (close(source) != 0 || fchmod(destination, 0555) != 0 ||
      lseek(destination, 0, SEEK_SET) != 0 ||
      fcntl(destination, F_ADD_SEALS, REQUIRED_EXECUTABLE_SEALS) != 0 ||
      fchmod(destination, 0444) == 0 || errno != EPERM) {
    const int saved_errno = errno;
    (void)close(destination);
    errno = saved_errno;
    return -1;
  }
  return destination;
}

static int
create_sealed_manifest(const struct fe2o3_preexec_manifest_v1 *manifest) {
  const int fd = (int)syscall(SYS_memfd_create, "fe2o3-preexec-manifest",
                              MFD_CLOEXEC | MFD_ALLOW_SEALING);
  if (fd < 0 || write_all(fd, manifest, sizeof(*manifest)) != 0 ||
      lseek(fd, 0, SEEK_SET) != 0 ||
      fcntl(fd, F_ADD_SEALS, REQUIRED_CONTENT_SEALS) != 0) {
    const int saved_errno = errno;
    if (fd >= 0) {
      (void)close(fd);
    }
    errno = saved_errno;
    return -1;
  }
  return fd;
}

static int
create_manifest_executable_alias(struct fe2o3_preexec_manifest_v1 *manifest) {
  const int fd = (int)syscall(SYS_memfd_create, "fe2o3-manifest-exec-alias",
                              MFD_CLOEXEC | MFD_ALLOW_SEALING);
  struct stat info;
  if (fd < 0 || fchmod(fd, 0555) != 0 || fstat(fd, &info) != 0) {
    return -1;
  }
  manifest->executable.device = (uint64_t)info.st_dev;
  manifest->executable.inode = (uint64_t)info.st_ino;
  manifest->executable.size = sizeof(*manifest);
  manifest->executable.mode = (uint32_t)info.st_mode;
  manifest->executable.reserved = 0U;
  if (write_all(fd, manifest, sizeof(*manifest)) != 0 ||
      lseek(fd, 0, SEEK_SET) != 0 ||
      fcntl(fd, F_ADD_SEALS, REQUIRED_EXECUTABLE_SEALS) != 0) {
    const int saved_errno = errno;
    (void)close(fd);
    errno = saved_errno;
    return -1;
  }
  return fd;
}

static int identity(int fd, struct fe2o3_preexec_object_identity_v1 *object) {
  struct stat info;
  if (fstat(fd, &info) != 0 || info.st_size < 0) {
    return -1;
  }
  object->device = (uint64_t)info.st_dev;
  object->inode = (uint64_t)info.st_ino;
  object->size = (uint64_t)info.st_size;
  object->mode = (uint32_t)info.st_mode;
  object->reserved = 0U;
  return 0;
}

static int self_start_time(uint64_t *start_time) {
  const int fd = open("/proc/self/stat", O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (fd < 0) {
    return -1;
  }
  char contents[4097];
  const ssize_t length = read(fd, contents, sizeof(contents) - 1U);
  const int saved_errno = errno;
  (void)close(fd);
  if (length <= 0 || (size_t)length >= sizeof(contents) - 1U) {
    errno = length < 0 ? saved_errno : EPROTO;
    return -1;
  }
  contents[length] = '\0';
  char *cursor = strrchr(contents, ')');
  if (cursor == NULL || cursor[1] != ' ') {
    errno = EPROTO;
    return -1;
  }
  cursor += 2;
  for (unsigned field = 3U; field <= 22U; ++field) {
    while (*cursor == ' ') {
      ++cursor;
    }
    char *end = cursor;
    while (*end != '\0' && *end != ' ' && *end != '\n') {
      ++end;
    }
    if (field == 22U) {
      char *number_end = NULL;
      errno = 0;
      const unsigned long long parsed = strtoull(cursor, &number_end, 10);
      if (errno != 0 || number_end != end || parsed == 0ULL) {
        errno = EPROTO;
        return -1;
      }
      *start_time = (uint64_t)parsed;
      return 0;
    }
    cursor = end;
  }
  errno = EPROTO;
  return -1;
}

static int create_pipe(int descriptors[2]) {
  return pipe2(descriptors, O_CLOEXEC);
}

static void close_if_open(int fd) {
  if (fd >= 0) {
    (void)close(fd);
  }
}

static void close_inputs(struct launch_inputs *inputs) {
  close_if_open(inputs->target_fd);
  close_if_open(inputs->substitute_target_fd);
  close_if_open(inputs->manifest_fd);
  for (size_t index = 0U; index < 4U; ++index) {
    close_if_open(inputs->sources[index]);
  }
  close_if_open(inputs->input_write_fd);
  close_if_open(inputs->stdout_read_fd);
  close_if_open(inputs->stderr_read_fd);
  close_if_open(inputs->report_read_fd);
  close_if_open(inputs->extra_fd);
}

static int prepare_inputs(const char *target_path, enum case_kind kind,
                          struct launch_inputs *inputs) {
  *inputs = (struct launch_inputs){
      .target_fd = -1,
      .substitute_target_fd = -1,
      .manifest_fd = -1,
      .sources = {-1, -1, -1, -1},
      .input_write_fd = -1,
      .stdout_read_fd = -1,
      .stderr_read_fd = -1,
      .report_read_fd = -1,
      .extra_fd = -1,
  };
  int input[2] = {-1, -1};
  int output[2] = {-1, -1};
  int errors[2] = {-1, -1};
  int report[2] = {-1, -1};
  if (create_pipe(input) != 0 || create_pipe(output) != 0 ||
      create_pipe(errors) != 0 || create_pipe(report) != 0) {
    goto fail;
  }
  inputs->sources[0] = input[0];
  inputs->input_write_fd = input[1];
  inputs->stdout_read_fd = output[0];
  inputs->sources[1] = output[1];
  inputs->stderr_read_fd = errors[0];
  inputs->sources[2] = errors[1];
  inputs->report_read_fd = report[0];
  inputs->sources[3] = report[1];

  inputs->target_fd = copy_to_sealed_memfd(target_path, "fe2o3-test-target");
  inputs->extra_fd = open("/dev/null", O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (inputs->target_fd < 0 || inputs->extra_fd < 0) {
    goto fail;
  }

  struct fe2o3_preexec_manifest_v1 manifest = {0};
  (void)memcpy(manifest.magic, FE2O3_PREEXEC_MANIFEST_MAGIC,
               sizeof(manifest.magic));
  manifest.version = FE2O3_PREEXEC_MANIFEST_VERSION;
  manifest.descriptor_count = 4U;
  manifest.parent_pid = (int32_t)getpid();
  if (self_start_time(&manifest.parent_start_time) != 0 ||
      identity(inputs->target_fd, &manifest.executable) != 0) {
    goto fail;
  }
  const int destinations[4] = {0, 1, 2, 9};
  for (size_t index = 0U; index < 4U; ++index) {
    manifest.descriptors[index].source_fd =
        FE2O3_PREEXEC_SOURCE_FD_BASE + (int32_t)index;
    manifest.descriptors[index].destination_fd = destinations[index];
    if (identity(inputs->sources[index], &manifest.descriptors[index].object) !=
        0) {
      goto fail;
    }
  }

  if (kind == CASE_ALIAS || kind == CASE_ALIAS_MUTABLE_METADATA) {
    manifest.descriptors[2].object = manifest.descriptors[1].object;
    if (kind == CASE_ALIAS_MUTABLE_METADATA) {
      manifest.descriptors[2].object.mode ^= S_IXUSR;
    }
  } else if (kind == CASE_STANDARD_FD) {
    manifest.descriptors[2].destination_fd = 10;
  } else if (kind == CASE_DUPLICATE_DESTINATION) {
    manifest.descriptors[3].destination_fd = 2;
  } else if (kind == CASE_PARENT_IDENTITY) {
    ++manifest.parent_start_time;
  }
  inputs->manifest_fd = kind == CASE_MANIFEST_EXECUTABLE_ALIAS
                            ? create_manifest_executable_alias(&manifest)
                            : create_sealed_manifest(&manifest);
  if (inputs->manifest_fd < 0) {
    goto fail;
  }
  if (kind == CASE_TARGET_SUBSTITUTION) {
    inputs->substitute_target_fd =
        copy_to_sealed_memfd(target_path, "fe2o3-substitute-target");
    if (inputs->substitute_target_fd < 0) {
      goto fail;
    }
  }
  return 0;

fail:
  close_if_open(input[0]);
  close_if_open(input[1]);
  close_if_open(output[0]);
  close_if_open(output[1]);
  close_if_open(errors[0]);
  close_if_open(errors[1]);
  close_if_open(report[0]);
  close_if_open(report[1]);
  close_inputs(inputs);
  return -1;
}

static int install_fixed_inputs(const struct launch_inputs *inputs,
                                enum case_kind kind) {
  for (size_t index = 0U; index < 4U; ++index) {
    if (kind == CASE_MISSING_SOURCE && index == 3U) {
      continue;
    }
    const int source =
        (kind == CASE_ALIAS || kind == CASE_ALIAS_MUTABLE_METADATA) &&
                index == 2U
            ? inputs->sources[1]
            : inputs->sources[index];
    if (dup3(source, FE2O3_PREEXEC_SOURCE_FD_BASE + (int)index, 0) < 0) {
      return -1;
    }
  }
  const int target =
      kind == CASE_TARGET_SUBSTITUTION
          ? inputs->substitute_target_fd
          : (kind == CASE_MANIFEST_EXECUTABLE_ALIAS ? inputs->manifest_fd
                                                    : inputs->target_fd);
  if (dup3(inputs->manifest_fd, FE2O3_PREEXEC_MANIFEST_FD, 0) < 0 ||
      dup3(target, FE2O3_PREEXEC_EXECUTABLE_FD, 0) < 0 ||
      dup3(inputs->extra_fd, 77, 0) < 0) {
    return -1;
  }
  if (kind == CASE_EXTRA_SOURCE &&
      dup3(inputs->extra_fd, FE2O3_PREEXEC_SOURCE_FD_BASE + 4, 0) < 0) {
    return -1;
  }
  return 0;
}

static int wait_bounded(pid_t child, int *status) {
  struct timespec delay = {.tv_sec = 0, .tv_nsec = 10000000L};
  for (int elapsed = 0; elapsed < TEST_TIMEOUT_MILLISECONDS; elapsed += 10) {
    const pid_t result = waitpid(child, status, WNOHANG);
    if (result == child) {
      return 0;
    }
    if (result < 0) {
      return -1;
    }
    (void)nanosleep(&delay, NULL);
  }
  (void)kill(child, SIGKILL);
  (void)waitpid(child, status, 0);
  errno = ETIMEDOUT;
  return -1;
}

static int read_report(int fd, char *buffer, size_t capacity) {
  size_t used = 0U;
  while (used + 1U < capacity) {
    const ssize_t count = read(fd, buffer + used, capacity - used - 1U);
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count < 0) {
      return -1;
    }
    if (count == 0) {
      break;
    }
    used += (size_t)count;
  }
  buffer[used] = '\0';
  return 0;
}

static int parse_case(const char *name, enum case_kind *kind) {
  static const struct {
    const char *name;
    enum case_kind kind;
  } cases[] = {
      {"success", CASE_SUCCESS},
      {"success-low-limit", CASE_SUCCESS_LOW_LIMIT},
      {"alias", CASE_ALIAS},
      {"alias-mutable-metadata", CASE_ALIAS_MUTABLE_METADATA},
      {"manifest-executable-alias", CASE_MANIFEST_EXECUTABLE_ALIAS},
      {"target-substitution", CASE_TARGET_SUBSTITUTION},
      {"extra-source", CASE_EXTRA_SOURCE},
      {"missing-source", CASE_MISSING_SOURCE},
      {"standard-fd", CASE_STANDARD_FD},
      {"duplicate-destination", CASE_DUPLICATE_DESTINATION},
      {"parent-identity", CASE_PARENT_IDENTITY},
      {"signal-state", CASE_SIGNAL_STATE},
  };
  for (size_t index = 0U; index < sizeof(cases) / sizeof(cases[0]); ++index) {
    if (strcmp(name, cases[index].name) == 0) {
      *kind = cases[index].kind;
      return 0;
    }
  }
  errno = EINVAL;
  return -1;
}

int main(int argc, char **argv) {
  if (argc != 6) {
    errno = EINVAL;
    return error_message(
        "expected CASE LAUNCHER TARGET HOSTILE_PRELOAD MARKER");
  }
  enum case_kind kind;
  if (parse_case(argv[1], &kind) != 0 ||
      (unlink(argv[5]) != 0 && errno != ENOENT)) {
    return error_message("cannot initialize test case");
  }
  struct launch_inputs inputs;
  if (prepare_inputs(argv[3], kind, &inputs) != 0) {
    return error_message("cannot prepare sealed launch inputs");
  }

  const pid_t child = fork();
  if (child < 0) {
    close_inputs(&inputs);
    return error_message("cannot fork launcher child");
  }
  if (child == 0) {
    if (install_fixed_inputs(&inputs, kind) != 0) {
      _exit(125);
    }
    if (kind == CASE_SUCCESS_LOW_LIMIT) {
      const struct rlimit low_limit = {.rlim_cur = 128U, .rlim_max = 128U};
      if (setrlimit(RLIMIT_NOFILE, &low_limit) != 0) {
        _exit(125);
      }
    }
    if (kind == CASE_SIGNAL_STATE) {
      sigset_t blocked;
      if (signal(SIGUSR1, SIG_IGN) == SIG_ERR || sigemptyset(&blocked) != 0 ||
          sigaddset(&blocked, SIGUSR2) != 0 ||
          sigprocmask(SIG_BLOCK, &blocked, NULL) != 0) {
        _exit(125);
      }
    }
    char preload[4096];
    const int length =
        snprintf(preload, sizeof(preload), "LD_PRELOAD=%s", argv[4]);
    if (length <= 0 || (size_t)length >= sizeof(preload)) {
      _exit(125);
    }
    char *const arguments[] = {argv[2], NULL};
    char tunables[] = "GLIBC_TUNABLES=glibc.malloc.mxfast=0";
    char malloc_check[] = "MALLOC_CHECK_=3";
    char *const environment[] = {preload, tunables, malloc_check, NULL};
    execve(argv[2], arguments, environment);
    _exit(125);
  }

  close_if_open(inputs.sources[0]);
  close_if_open(inputs.sources[1]);
  close_if_open(inputs.sources[2]);
  close_if_open(inputs.sources[3]);
  inputs.sources[0] = -1;
  inputs.sources[1] = -1;
  inputs.sources[2] = -1;
  inputs.sources[3] = -1;
  close_if_open(inputs.input_write_fd);
  inputs.input_write_fd = -1;
  close_if_open(inputs.target_fd);
  inputs.target_fd = -1;
  close_if_open(inputs.substitute_target_fd);
  inputs.substitute_target_fd = -1;
  close_if_open(inputs.manifest_fd);
  inputs.manifest_fd = -1;
  close_if_open(inputs.extra_fd);
  inputs.extra_fd = -1;

  int status = 0;
  if (wait_bounded(child, &status) != 0) {
    close_inputs(&inputs);
    return error_message("launcher child exceeded fixed deadline");
  }
  char report[128];
  if (read_report(inputs.report_read_fd, report, sizeof(report)) != 0) {
    close_inputs(&inputs);
    return error_message("cannot read target report");
  }
  const bool marker_absent = access(argv[5], F_OK) != 0 && errno == ENOENT;
  close_inputs(&inputs);

  const bool expected_success = kind == CASE_SUCCESS ||
                                kind == CASE_SUCCESS_LOW_LIMIT ||
                                kind == CASE_SIGNAL_STATE;
  if (!marker_absent) {
    errno = EACCES;
    return error_message("hostile LD_PRELOAD constructor ran");
  }
  if (expected_success) {
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
        strcmp(report, "OK\n") != 0) {
      errno = EPROTO;
      return error_message("valid launch did not reach the exact target state");
    }
  } else if (!WIFEXITED(status) || WEXITSTATUS(status) != 126 ||
             report[0] != '\0') {
    errno = EPROTO;
    return error_message("malformed launch was not rejected before exec");
  }
  return 0;
}
