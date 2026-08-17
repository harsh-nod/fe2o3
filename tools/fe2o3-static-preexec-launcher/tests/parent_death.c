#define _GNU_SOURCE

#include "fe2o3_static_preexec_manifest.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/memfd.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define REQUIRED_CONTENT_SEALS                                                 \
  (F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE)
#define REQUIRED_EXECUTABLE_SEALS (REQUIRED_CONTENT_SEALS | F_SEAL_EXEC)

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

static int sealed_target(const char *path) {
  const int source = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  const int target = (int)syscall(SYS_memfd_create, "parent-death-target",
                                  MFD_CLOEXEC | MFD_ALLOW_SEALING);
  if (source < 0 || target < 0) {
    return -1;
  }
  char buffer[16384];
  for (;;) {
    const ssize_t count = read(source, buffer, sizeof(buffer));
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count < 0 ||
        (count > 0 && write_all(target, buffer, (size_t)count) != 0)) {
      return -1;
    }
    if (count == 0) {
      break;
    }
  }
  if (close(source) != 0 || fchmod(target, 0555) != 0 ||
      lseek(target, 0, SEEK_SET) != 0 ||
      fcntl(target, F_ADD_SEALS, REQUIRED_EXECUTABLE_SEALS) != 0) {
    return -1;
  }
  return target;
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
  return 0;
}

static int process_start_time(pid_t process, uint64_t *start_time) {
  char path[64];
  const int path_length =
      snprintf(path, sizeof(path), "/proc/%ld/stat", (long)process);
  if (path_length <= 0 || (size_t)path_length >= sizeof(path)) {
    errno = EOVERFLOW;
    return -1;
  }
  const int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (fd < 0) {
    return -1;
  }
  char contents[4097];
  const ssize_t length = read(fd, contents, sizeof(contents) - 1U);
  (void)close(fd);
  if (length <= 0 || (size_t)length >= sizeof(contents) - 1U) {
    errno = EPROTO;
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
      const unsigned long long parsed = strtoull(cursor, &number_end, 10);
      if (number_end != end || parsed == 0ULL) {
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

static int sealed_manifest(const struct fe2o3_preexec_manifest_v1 *manifest) {
  const int fd = (int)syscall(SYS_memfd_create, "parent-death-manifest",
                              MFD_CLOEXEC | MFD_ALLOW_SEALING);
  if (fd < 0 || write_all(fd, manifest, sizeof(*manifest)) != 0 ||
      lseek(fd, 0, SEEK_SET) != 0 ||
      fcntl(fd, F_ADD_SEALS, REQUIRED_CONTENT_SEALS) != 0) {
    return -1;
  }
  return fd;
}

static void launcher_child(const char *launcher, const char *target_path,
                           int ready_write) {
  int standard_input[2];
  int standard_output[2];
  int standard_error[2];
  if (pipe2(standard_input, O_CLOEXEC) != 0 ||
      pipe2(standard_output, O_CLOEXEC) != 0 ||
      pipe2(standard_error, O_CLOEXEC) != 0) {
    _exit(125);
  }
  const int sources[4] = {standard_input[0], standard_output[1],
                          standard_error[1], ready_write};
  const int destinations[4] = {0, 1, 2, 127};
  const int target = sealed_target(target_path);
  if (target < 0) {
    _exit(125);
  }

  struct fe2o3_preexec_manifest_v1 manifest = {0};
  (void)memcpy(manifest.magic, FE2O3_PREEXEC_MANIFEST_MAGIC,
               sizeof(manifest.magic));
  manifest.version = FE2O3_PREEXEC_MANIFEST_VERSION;
  manifest.descriptor_count = 4U;
  manifest.parent_pid = (int32_t)getppid();
  if (process_start_time((pid_t)manifest.parent_pid,
                         &manifest.parent_start_time) != 0 ||
      identity(target, &manifest.executable) != 0) {
    _exit(125);
  }
  for (size_t index = 0U; index < 4U; ++index) {
    manifest.descriptors[index].source_fd =
        FE2O3_PREEXEC_SOURCE_FD_BASE + (int32_t)index;
    manifest.descriptors[index].destination_fd = destinations[index];
    if (identity(sources[index], &manifest.descriptors[index].object) != 0 ||
        dup3(sources[index], FE2O3_PREEXEC_SOURCE_FD_BASE + (int)index, 0) <
            0) {
      _exit(125);
    }
  }
  const int manifest_fd = sealed_manifest(&manifest);
  if (manifest_fd < 0 || dup3(manifest_fd, FE2O3_PREEXEC_MANIFEST_FD, 0) < 0 ||
      dup3(target, FE2O3_PREEXEC_EXECUTABLE_FD, 0) < 0) {
    _exit(125);
  }
  char *const arguments[] = {(char *)launcher, NULL};
  char *const environment[] = {NULL};
  execve(launcher, arguments, environment);
  _exit(125);
}

static int wait_bounded(pid_t child, int *status) {
  struct timespec delay = {.tv_sec = 0, .tv_nsec = 10000000L};
  for (int elapsed = 0; elapsed < 5000; elapsed += 10) {
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

int main(int argc, char **argv) {
  if (argc != 3 || prctl(PR_SET_CHILD_SUBREAPER, 1L, 0L, 0L, 0L) != 0) {
    return 1;
  }
  int child_identity[2];
  if (pipe2(child_identity, O_CLOEXEC) != 0) {
    return 1;
  }
  const pid_t supervisor = fork();
  if (supervisor < 0) {
    return 1;
  }
  if (supervisor == 0) {
    (void)close(child_identity[0]);
    int ready[2];
    if (pipe2(ready, O_CLOEXEC) != 0) {
      _exit(1);
    }
    const pid_t child = fork();
    if (child < 0) {
      _exit(1);
    }
    if (child == 0) {
      (void)close(ready[0]);
      launcher_child(argv[1], argv[2], ready[1]);
    }
    (void)close(ready[1]);
    char signal_byte = 0;
    if (read(ready[0], &signal_byte, 1U) != 1 || signal_byte != 'R' ||
        write_all(child_identity[1], &child, sizeof(child)) != 0) {
      (void)kill(child, SIGKILL);
      _exit(1);
    }
    _exit(0);
  }

  (void)close(child_identity[1]);
  pid_t launcher = -1;
  if (read(child_identity[0], &launcher, sizeof(launcher)) !=
      (ssize_t)sizeof(launcher)) {
    return 1;
  }
  int supervisor_status = 0;
  if (waitpid(supervisor, &supervisor_status, 0) != supervisor ||
      !WIFEXITED(supervisor_status) || WEXITSTATUS(supervisor_status) != 0) {
    return 1;
  }
  int launcher_status = 0;
  if (wait_bounded(launcher, &launcher_status) != 0 ||
      !WIFSIGNALED(launcher_status) || WTERMSIG(launcher_status) != SIGKILL) {
    return 1;
  }
  return 0;
}
