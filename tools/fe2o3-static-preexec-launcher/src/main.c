#define _GNU_SOURCE

#include "fe2o3_static_preexec_manifest.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/magic.h>
#include <signal.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/vfs.h>

#ifndef FE2O3_PREEXEC_TEST_ONLY
#define FE2O3_PREEXEC_TEST_ONLY 0
#endif
#ifndef FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK
#define FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK 0
#endif
#ifndef SYS_pidfd_send_signal
#define SYS_pidfd_send_signal 424
#endif

#if FE2O3_PREEXEC_TEST_ONLY != 0 && FE2O3_PREEXEC_TEST_ONLY != 1
#error "FE2O3_PREEXEC_TEST_ONLY must be zero or one"
#endif
#if FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK != 0 &&                            \
    FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK != 1
#error "FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK must be zero or one"
#endif
#if !FE2O3_PREEXEC_TEST_ONLY && FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK != 0
#error "production launcher cannot contain test hooks"
#endif

#define FE2O3_PREEXEC_MAX_PROC_STAT_BYTES 4096U
#define FE2O3_PREEXEC_FALLBACK_ENTRY_LIMIT 1048576U
#define FE2O3_PREEXEC_FALLBACK_BUFFER_BYTES 16384U
#define FE2O3_PREEXEC_REQUIRED_SEALS                                           \
  (F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE)
#define FE2O3_PREEXEC_REQUIRED_EXECUTABLE_SEALS                                \
  (FE2O3_PREEXEC_REQUIRED_SEALS | F_SEAL_EXEC)
#define FE2O3_LINUX_ERROR_LIMIT 4095L

struct fe2o3_object_snapshot {
  uint64_t device;
  uint64_t inode;
  uint64_t size;
  uint32_t mode;
};

struct fe2o3_kernel_sigaction {
  uint64_t handler;
  uint64_t flags;
  uint64_t restorer;
  uint64_t mask;
};

struct fe2o3_linux_rlimit64 {
  uint64_t current;
  uint64_t maximum;
};

static long linux_syscall6(long number, long argument1, long argument2,
                           long argument3, long argument4, long argument5,
                           long argument6) {
  register long register10 __asm__("r10") = argument4;
  register long register8 __asm__("r8") = argument5;
  register long register9 __asm__("r9") = argument6;
  long result;
  __asm__ volatile("syscall"
                   : "=a"(result)
                   : "a"(number), "D"(argument1), "S"(argument2),
                     "d"(argument3), "r"(register10), "r"(register8),
                     "r"(register9)
                   : "rcx", "r11", "memory");
  return result;
}

static long linux_syscall5(long number, long argument1, long argument2,
                           long argument3, long argument4, long argument5) {
  return linux_syscall6(number, argument1, argument2, argument3, argument4,
                        argument5, 0L);
}

static long linux_syscall4(long number, long argument1, long argument2,
                           long argument3, long argument4) {
  return linux_syscall6(number, argument1, argument2, argument3, argument4, 0L,
                        0L);
}

static long linux_syscall3(long number, long argument1, long argument2,
                           long argument3) {
  return linux_syscall6(number, argument1, argument2, argument3, 0L, 0L, 0L);
}

static long linux_syscall2(long number, long argument1, long argument2) {
  return linux_syscall6(number, argument1, argument2, 0L, 0L, 0L, 0L);
}

static long linux_syscall1(long number, long argument1) {
  return linux_syscall6(number, argument1, 0L, 0L, 0L, 0L, 0L);
}

static long linux_syscall0(long number) {
  return linux_syscall6(number, 0L, 0L, 0L, 0L, 0L, 0L);
}

static bool linux_error(long result) {
  return result < 0L && result >= -FE2O3_LINUX_ERROR_LIMIT;
}

static long pointer_argument(const void *pointer) {
  return (long)(uintptr_t)pointer;
}

static bool bytes_equal(const void *left, const void *right, size_t length) {
  const unsigned char *left_bytes = (const unsigned char *)left;
  const unsigned char *right_bytes = (const unsigned char *)right;
  for (size_t index = 0U; index < length; ++index) {
    if (left_bytes[index] != right_bytes[index]) {
      return false;
    }
  }
  return true;
}

static bool bytes_are_zero(const void *bytes, size_t length) {
  const unsigned char *values = (const unsigned char *)bytes;
  for (size_t index = 0U; index < length; ++index) {
    if (values[index] != 0U) {
      return false;
    }
  }
  return true;
}

static int fail(const char *message) {
  (void)message;
  return 126;
}

static int normalize_signal_state(void) {
  const struct fe2o3_kernel_sigaction default_action = {0};
  for (int signal_number = 1; signal_number <= 64; ++signal_number) {
    if (signal_number == SIGKILL || signal_number == SIGSTOP) {
      continue;
    }
    if (linux_error(linux_syscall4(SYS_rt_sigaction, (long)signal_number,
                                   pointer_argument(&default_action), 0L,
                                   (long)sizeof(uint64_t)))) {
      return -1;
    }
  }
  const uint64_t empty_mask = 0U;
  return linux_error(linux_syscall4(SYS_rt_sigprocmask, SIG_SETMASK,
                                    pointer_argument(&empty_mask), 0L,
                                    (long)sizeof(empty_mask)))
             ? -1
             : 0;
}

static int revalidate_signal_state(void) {
  for (int signal_number = 1; signal_number <= 64; ++signal_number) {
    if (signal_number == SIGKILL || signal_number == SIGSTOP) {
      continue;
    }
    struct fe2o3_kernel_sigaction observed = {UINT64_MAX, UINT64_MAX,
                                              UINT64_MAX, UINT64_MAX};
    if (linux_error(linux_syscall4(SYS_rt_sigaction, (long)signal_number, 0L,
                                   pointer_argument(&observed),
                                   (long)sizeof(uint64_t))) ||
        !bytes_are_zero(&observed, sizeof(observed))) {
      return -1;
    }
  }
  uint64_t observed_mask = UINT64_MAX;
  return linux_error(linux_syscall4(SYS_rt_sigprocmask, SIG_SETMASK, 0L,
                                    pointer_argument(&observed_mask),
                                    (long)sizeof(observed_mask))) ||
                 observed_mask != 0U
             ? -1
             : 0;
}

static int normalize_process_boundary(void) {
  const struct fe2o3_linux_rlimit64 no_core = {0U, 0U};
  struct fe2o3_linux_rlimit64 observed_core = {UINT64_MAX, UINT64_MAX};
  if (linux_error(linux_syscall5(SYS_prctl, PR_SET_DUMPABLE, 0L, 0L, 0L, 0L)) ||
      linux_syscall5(SYS_prctl, PR_GET_DUMPABLE, 0L, 0L, 0L, 0L) != 0L ||
      linux_error(
          linux_syscall5(SYS_prctl, PR_SET_NO_NEW_PRIVS, 1L, 0L, 0L, 0L)) ||
      linux_syscall5(SYS_prctl, PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L) != 1L ||
      linux_error(linux_syscall4(SYS_prlimit64, 0L, RLIMIT_CORE,
                                 pointer_argument(&no_core), 0L)) ||
      linux_error(linux_syscall4(SYS_prlimit64, 0L, RLIMIT_CORE, 0L,
                                 pointer_argument(&observed_core))) ||
      observed_core.current != 0U || observed_core.maximum != 0U ||
      normalize_signal_state() != 0) {
    return -1;
  }
  return 0;
}

static long read_retry(int fd, void *buffer, size_t length) {
  long result;
  do {
    result = linux_syscall3(SYS_read, (long)fd, pointer_argument(buffer),
                            (long)length);
  } while (result == -EINTR);
  return result;
}

static long pread_retry(int fd, void *buffer, size_t length, uint64_t offset) {
  long result;
  do {
    result = linux_syscall4(SYS_pread64, (long)fd, pointer_argument(buffer),
                            (long)length, (long)offset);
  } while (result == -EINTR);
  return result;
}

static bool descriptor_is_closed(int fd) {
  return linux_syscall3(SYS_fcntl, (long)fd, F_GETFD, 0L) == -EBADF;
}

static int close_and_verify(int fd) {
  (void)linux_syscall1(SYS_close, (long)fd);
  return descriptor_is_closed(fd) ? 0 : -1;
}

static int read_exact_at_start(int fd, void *buffer, size_t length) {
  size_t offset = 0U;
  while (offset < length) {
    const long count =
        pread_retry(fd, (char *)buffer + offset, length - offset, offset);
    if (linux_error(count) || count == 0L) {
      return -1;
    }
    offset += (size_t)count;
  }
  char extra = 0;
  return pread_retry(fd, &extra, 1U, length) == 0L ? 0 : -1;
}

static int append_decimal(char *path, size_t capacity, size_t *used,
                          uint32_t value) {
  char reversed[10];
  size_t digits = 0U;
  do {
    reversed[digits] = (char)('0' + (value % 10U));
    ++digits;
    value /= 10U;
  } while (value != 0U);
  if (*used + digits >= capacity) {
    return -1;
  }
  while (digits != 0U) {
    --digits;
    path[*used] = reversed[digits];
    ++*used;
  }
  return 0;
}

static int read_proc_start_time(int32_t pid, uint64_t *start_time) {
  static const char prefix[] = "/proc/";
  static const char suffix[] = "/stat";
  char path[64];
  size_t path_length = 0U;
  if (pid <= 1) {
    return -1;
  }
  for (size_t index = 0U; index < sizeof(prefix) - 1U; ++index) {
    path[path_length++] = prefix[index];
  }
  if (append_decimal(path, sizeof(path), &path_length, (uint32_t)pid) != 0) {
    return -1;
  }
  for (size_t index = 0U; index < sizeof(suffix); ++index) {
    if (path_length >= sizeof(path)) {
      return -1;
    }
    path[path_length++] = suffix[index];
  }

  const long opened =
      linux_syscall4(SYS_openat, AT_FDCWD, pointer_argument(path),
                     O_RDONLY | O_CLOEXEC | O_NOFOLLOW, 0L);
  if (linux_error(opened)) {
    return -1;
  }
  const int fd = (int)opened;
  char contents[FE2O3_PREEXEC_MAX_PROC_STAT_BYTES];
  size_t used = 0U;
  while (used < sizeof(contents)) {
    const long count = read_retry(fd, contents + used, sizeof(contents) - used);
    if (linux_error(count)) {
      (void)close_and_verify(fd);
      return -1;
    }
    if (count == 0L) {
      break;
    }
    used += (size_t)count;
  }
  char extra = 0;
  const long extra_count = read_retry(fd, &extra, 1U);
  if (close_and_verify(fd) != 0 || used == 0U || used == sizeof(contents) ||
      extra_count != 0L) {
    return -1;
  }

  size_t cursor = used;
  while (cursor != 0U && contents[cursor - 1U] != ')') {
    --cursor;
  }
  if (cursor == 0U || cursor >= used || contents[cursor] != ' ') {
    return -1;
  }
  ++cursor;
  for (unsigned field = 3U; field <= 22U; ++field) {
    while (cursor < used && contents[cursor] == ' ') {
      ++cursor;
    }
    const size_t first = cursor;
    while (cursor < used && contents[cursor] != ' ' &&
           contents[cursor] != '\n') {
      ++cursor;
    }
    if (first == cursor) {
      return -1;
    }
    if (field == 22U) {
      uint64_t value = 0U;
      for (size_t index = first; index < cursor; ++index) {
        if (contents[index] < '0' || contents[index] > '9') {
          return -1;
        }
        const uint64_t digit = (uint64_t)(contents[index] - '0');
        if (value > (UINT64_MAX - digit) / 10U) {
          return -1;
        }
        value = value * 10U + digit;
      }
      if (value == 0U) {
        return -1;
      }
      *start_time = value;
      return 0;
    }
  }
  return -1;
}

static int object_snapshot(int fd, struct fe2o3_object_snapshot *snapshot) {
  struct stat info;
  const long result =
      linux_syscall2(SYS_fstat, (long)fd, pointer_argument(&info));
  if (linux_error(result) || info.st_size < 0) {
    return -1;
  }
  snapshot->device = (uint64_t)info.st_dev;
  snapshot->inode = (uint64_t)info.st_ino;
  snapshot->size = (uint64_t)info.st_size;
  snapshot->mode = (uint32_t)info.st_mode;
  return 0;
}

static bool same_key(const struct fe2o3_object_snapshot *left,
                     const struct fe2o3_object_snapshot *right) {
  return left->device == right->device && left->inode == right->inode;
}

static bool
snapshot_matches_record(const struct fe2o3_object_snapshot *snapshot,
                        const struct fe2o3_preexec_object_identity_v1 *record) {
  return snapshot->device == record->device && snapshot->inode == record->inode &&
         snapshot->size == record->size && snapshot->mode == record->mode;
}

static bool
records_have_same_key(const struct fe2o3_preexec_object_identity_v1 *left,
                      const struct fe2o3_preexec_object_identity_v1 *right) {
  return left->device == right->device && left->inode == right->inode;
}

static bool
record_has_snapshot_key(const struct fe2o3_preexec_object_identity_v1 *record,
                        const struct fe2o3_object_snapshot *snapshot) {
  return record->device == snapshot->device && record->inode == snapshot->inode;
}

static bool object_classes_may_share_key(
    const struct fe2o3_preexec_object_identity_v1 *left,
    const struct fe2o3_preexec_object_identity_v1 *right) {
  return left->object_class == FE2O3_PREEXEC_OBJECT_CLASS_PROCESS_PIDFD &&
         right->object_class == FE2O3_PREEXEC_OBJECT_CLASS_PROCESS_PIDFD;
}

static int validate_object_class(int fd, uint32_t object_class) {
  if (object_class == FE2O3_PREEXEC_OBJECT_CLASS_FSTAT) {
    return 0;
  }
  if (object_class != FE2O3_PREEXEC_OBJECT_CLASS_PROCESS_PIDFD) {
    return -1;
  }
  const long result = linux_syscall4(SYS_pidfd_send_signal, (long)fd, 0L, 0L, 0L);
  return result == 0L || result == -EPERM ? 0 : -1;
}

static int file_control(int fd, int command, long argument) {
  const long result =
      linux_syscall3(SYS_fcntl, (long)fd, (long)command, argument);
  return linux_error(result) ? -1 : (int)result;
}

static int require_no_file_capability(int fd) {
  char value = 0;
  const long result = linux_syscall4(SYS_fgetxattr, (long)fd,
                                     pointer_argument("security.capability"),
                                     pointer_argument(&value), 1L);
  return result == -ENODATA || result == -EOPNOTSUPP ? 0 : -1;
}

static int require_sealed_regular_file(int fd, bool executable,
                                       struct fe2o3_object_snapshot *snapshot) {
  const int seals = file_control(fd, F_GET_SEALS, 0L);
  const int status = file_control(fd, F_GETFL, 0L);
  const int required_seals = executable
                                 ? FE2O3_PREEXEC_REQUIRED_EXECUTABLE_SEALS
                                 : FE2O3_PREEXEC_REQUIRED_SEALS;
  if (seals < 0 || (seals & required_seals) != required_seals || status < 0 ||
      (status & O_PATH) != 0 || object_snapshot(fd, snapshot) != 0 ||
      !S_ISREG(snapshot->mode) ||
      (executable &&
       (snapshot->size == 0U || (snapshot->mode & (S_ISUID | S_ISGID)) != 0U ||
        (snapshot->mode & (S_IXUSR | S_IXGRP | S_IXOTH)) == 0U ||
        require_no_file_capability(fd) != 0))) {
    return -1;
  }
  return 0;
}

static int
validate_manifest_file(struct fe2o3_preexec_manifest_v1 *manifest,
                       struct fe2o3_object_snapshot *manifest_object) {
  if (require_sealed_regular_file(FE2O3_PREEXEC_MANIFEST_FD, false,
                                  manifest_object) != 0 ||
      manifest_object->size != sizeof(*manifest) ||
      read_exact_at_start(FE2O3_PREEXEC_MANIFEST_FD, manifest,
                          sizeof(*manifest)) != 0 ||
      !bytes_equal(manifest->magic, FE2O3_PREEXEC_MANIFEST_MAGIC,
                   sizeof(manifest->magic)) ||
      manifest->version != FE2O3_PREEXEC_MANIFEST_VERSION ||
      manifest->reserved != 0U || manifest->parent_pid <= 1 ||
      manifest->parent_start_time == 0U || manifest->descriptor_count < 3U ||
      manifest->descriptor_count > FE2O3_PREEXEC_MAX_DESCRIPTORS ||
      manifest->executable.object_class != FE2O3_PREEXEC_OBJECT_CLASS_FSTAT) {
    return -1;
  }
  for (uint32_t index = manifest->descriptor_count;
       index < FE2O3_PREEXEC_MAX_DESCRIPTORS; ++index) {
    if (!bytes_are_zero(&manifest->descriptors[index],
                        sizeof(manifest->descriptors[index]))) {
      return -1;
    }
  }
  return 0;
}

static int
arm_parent_containment(const struct fe2o3_preexec_manifest_v1 *manifest) {
  uint64_t first_start_time = 0U;
  uint64_t second_start_time = 0U;
  const long expected_parent = (long)manifest->parent_pid;
  if (linux_syscall0(SYS_getppid) != expected_parent ||
      read_proc_start_time(manifest->parent_pid, &first_start_time) != 0 ||
      first_start_time != manifest->parent_start_time ||
      linux_syscall5(SYS_prctl, PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L) != 1L ||
      linux_error(
          linux_syscall5(SYS_prctl, PR_SET_PDEATHSIG, SIGKILL, 0L, 0L, 0L)) ||
      linux_syscall0(SYS_getppid) != expected_parent ||
      read_proc_start_time(manifest->parent_pid, &second_start_time) != 0 ||
      second_start_time != first_start_time) {
    return -1;
  }
  int signal_number = 0;
  if (linux_error(linux_syscall5(SYS_prctl, PR_GET_PDEATHSIG,
                                 pointer_argument(&signal_number), 0L, 0L,
                                 0L)) ||
      signal_number != SIGKILL) {
    return -1;
  }
  return 0;
}

static int revalidate_parent_containment(
    const struct fe2o3_preexec_manifest_v1 *manifest) {
  uint64_t start_time = 0U;
  int signal_number = 0;
  struct fe2o3_linux_rlimit64 observed_core = {UINT64_MAX, UINT64_MAX};
  return linux_syscall0(SYS_getppid) == (long)manifest->parent_pid &&
                 read_proc_start_time(manifest->parent_pid, &start_time) == 0 &&
                 start_time == manifest->parent_start_time &&
                 linux_syscall5(SYS_prctl, PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L,
                                0L) == 1L &&
                 linux_syscall5(SYS_prctl, PR_GET_DUMPABLE, 0L, 0L, 0L, 0L) ==
                     0L &&
                 !linux_error(
                     linux_syscall4(SYS_prlimit64, 0L, RLIMIT_CORE, 0L,
                                    pointer_argument(&observed_core))) &&
                 observed_core.current == 0U && observed_core.maximum == 0U &&
                 !linux_error(linux_syscall5(SYS_prctl, PR_GET_PDEATHSIG,
                                             pointer_argument(&signal_number),
                                             0L, 0L, 0L)) &&
                 signal_number == SIGKILL && revalidate_signal_state() == 0
             ? 0
             : -1;
}

static int descriptor_access_is_valid(int fd, int destination_fd) {
  const int status = file_control(fd, F_GETFL, 0L);
  if (status < 0 || (status & O_PATH) != 0) {
    return -1;
  }
  const int access = status & O_ACCMODE;
  if ((destination_fd == STDIN_FILENO && access == O_WRONLY) ||
      ((destination_fd == STDOUT_FILENO || destination_fd == STDERR_FILENO) &&
       access == O_RDONLY)) {
    return -1;
  }
  return 0;
}

static int validate_descriptor_manifest(
    const struct fe2o3_preexec_manifest_v1 *manifest,
    const struct fe2o3_object_snapshot *manifest_object) {
  struct fe2o3_object_snapshot executable;
  if (require_sealed_regular_file(FE2O3_PREEXEC_EXECUTABLE_FD, true,
                                  &executable) != 0 ||
      !snapshot_matches_record(&executable, &manifest->executable) ||
      same_key(&executable, manifest_object)) {
    return -1;
  }

  bool destinations[FE2O3_PREEXEC_MAX_DESTINATION_FD + 1U] = {false};
  for (uint32_t index = 0U; index < manifest->descriptor_count; ++index) {
    const struct fe2o3_preexec_descriptor_v1 *entry =
        &manifest->descriptors[index];
    const int expected_source = FE2O3_PREEXEC_SOURCE_FD_BASE + (int)index;
    if (entry->source_fd != expected_source || entry->destination_fd < 0 ||
        entry->destination_fd > FE2O3_PREEXEC_MAX_DESTINATION_FD ||
        destinations[entry->destination_fd] ||
        entry->object.object_class > FE2O3_PREEXEC_OBJECT_CLASS_PROCESS_PIDFD ||
        records_have_same_key(&entry->object, &manifest->executable) ||
        record_has_snapshot_key(&entry->object, manifest_object)) {
      return -1;
    }
    destinations[entry->destination_fd] = true;
    for (uint32_t previous = 0U; previous < index; ++previous) {
      if (records_have_same_key(&entry->object,
                                &manifest->descriptors[previous].object) &&
          !object_classes_may_share_key(
              &entry->object, &manifest->descriptors[previous].object)) {
        return -1;
      }
    }
  }
  if (!destinations[STDIN_FILENO] || !destinations[STDOUT_FILENO] ||
      !destinations[STDERR_FILENO]) {
    return -1;
  }

  for (uint32_t index = 0U; index < manifest->descriptor_count; ++index) {
    const struct fe2o3_preexec_descriptor_v1 *entry =
        &manifest->descriptors[index];
    struct fe2o3_object_snapshot observed;
    if (descriptor_access_is_valid(entry->source_fd, entry->destination_fd) !=
            0 ||
        object_snapshot(entry->source_fd, &observed) != 0 ||
        !snapshot_matches_record(&observed, &entry->object) ||
        validate_object_class(entry->source_fd, entry->object.object_class) != 0) {
      return -1;
    }
  }
  for (uint32_t index = manifest->descriptor_count;
       index < FE2O3_PREEXEC_MAX_DESCRIPTORS; ++index) {
    if (!descriptor_is_closed(FE2O3_PREEXEC_SOURCE_FD_BASE + (int)index)) {
      return -1;
    }
  }
  return 0;
}

static bool
descriptor_is_retained(int fd,
                       const struct fe2o3_preexec_manifest_v1 *manifest) {
  if (fd == FE2O3_PREEXEC_MANIFEST_FD || fd == FE2O3_PREEXEC_EXECUTABLE_FD) {
    return true;
  }
  for (uint32_t index = 0U; index < manifest->descriptor_count; ++index) {
    if (fd == manifest->descriptors[index].destination_fd) {
      return true;
    }
  }
  return false;
}

struct linux_directory_entry64 {
  uint64_t inode;
  int64_t offset;
  unsigned short record_length;
  unsigned char type;
  char name[];
};

static int parse_descriptor_name(const char *name, size_t length, int *fd) {
  if (length == 0U || name[0] == '.') {
    return 0;
  }
  unsigned value = 0U;
  size_t index = 0U;
  for (; index < length && name[index] != '\0'; ++index) {
    if (name[index] < '0' || name[index] > '9') {
      return -1;
    }
    const unsigned digit = (unsigned)(name[index] - '0');
    if (value > ((unsigned)INT_MAX - digit) / 10U) {
      return -1;
    }
    value = value * 10U + digit;
  }
  if (index == 0U || index == length) {
    return -1;
  }
  *fd = (int)value;
  return 1;
}

#if !FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK
static int
close_unrelated_with_range(const struct fe2o3_preexec_manifest_v1 *manifest) {
#if defined(SYS_close_range)
  unsigned first = 3U;
  for (unsigned fd = 3U; fd <= (unsigned)FE2O3_PREEXEC_EXECUTABLE_FD; ++fd) {
    if (!descriptor_is_retained((int)fd, manifest)) {
      continue;
    }
    if (first < fd && linux_error(linux_syscall3(SYS_close_range, (long)first,
                                                 (long)(fd - 1U), 0L))) {
      return -1;
    }
    first = fd + 1U;
  }
  return linux_error(
             linux_syscall3(SYS_close_range, (long)first, (long)UINT_MAX, 0L))
             ? -1
             : 0;
#else
  (void)manifest;
  return -ENOSYS;
#endif
}
#endif

static int close_unrelated_with_bounded_fallback(
    const struct fe2o3_preexec_manifest_v1 *manifest) {
  const long opened =
      linux_syscall4(SYS_openat, AT_FDCWD, pointer_argument("/proc/self/fd"),
                     O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC, 0L);
  if (linux_error(opened)) {
    return -1;
  }
  const int directory_fd = (int)opened;
  struct statfs filesystem;
  if (linux_error(linux_syscall2(SYS_fstatfs, (long)directory_fd,
                                 pointer_argument(&filesystem))) ||
      filesystem.f_type != (long)PROC_SUPER_MAGIC) {
    (void)close_and_verify(directory_fd);
    return -1;
  }

  _Alignas(struct linux_directory_entry64) char
      buffer[FE2O3_PREEXEC_FALLBACK_BUFFER_BYTES];
  uint32_t entries = 0U;
  for (;;) {
    long length;
    do {
      length = linux_syscall3(SYS_getdents64, (long)directory_fd,
                              pointer_argument(buffer), (long)sizeof(buffer));
    } while (length == -EINTR);
    if (linux_error(length)) {
      (void)close_and_verify(directory_fd);
      return -1;
    }
    if (length == 0L) {
      break;
    }
    size_t offset = 0U;
    while (offset < (size_t)length) {
      const size_t minimum =
          offsetof(struct linux_directory_entry64, name) + 2U;
      if ((size_t)length - offset < minimum) {
        (void)close_and_verify(directory_fd);
        return -1;
      }
      const struct linux_directory_entry64 *entry =
          (const struct linux_directory_entry64 *)(buffer + offset);
      const size_t record_length = (size_t)entry->record_length;
      if (record_length < minimum || record_length > (size_t)length - offset ||
          ++entries > FE2O3_PREEXEC_FALLBACK_ENTRY_LIMIT) {
        (void)close_and_verify(directory_fd);
        return -1;
      }
      int fd = -1;
      const int parsed = parse_descriptor_name(
          entry->name,
          record_length - offsetof(struct linux_directory_entry64, name), &fd);
      if (parsed < 0 || (parsed == 1 && fd >= 3 && fd != directory_fd &&
                         !descriptor_is_retained(fd, manifest) &&
                         close_and_verify(fd) != 0)) {
        (void)close_and_verify(directory_fd);
        return -1;
      }
      offset += record_length;
    }
  }
  return close_and_verify(directory_fd);
}

static int
install_descriptor_table(const struct fe2o3_preexec_manifest_v1 *manifest,
                         const struct fe2o3_object_snapshot *initial_manifest) {
  for (uint32_t index = 0U; index < manifest->descriptor_count; ++index) {
    const struct fe2o3_preexec_descriptor_v1 *entry =
        &manifest->descriptors[index];
    if (linux_error(linux_syscall3(SYS_dup3, (long)entry->source_fd,
                                   (long)entry->destination_fd, 0L))) {
      return -1;
    }
  }
  int executable_flags = file_control(FE2O3_PREEXEC_EXECUTABLE_FD, F_GETFD, 0L);
  if (executable_flags < 0 ||
      file_control(FE2O3_PREEXEC_EXECUTABLE_FD, F_SETFD,
                   (long)(executable_flags | FD_CLOEXEC)) < 0) {
    return -1;
  }

#if FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK
  if (close_unrelated_with_bounded_fallback(manifest) != 0) {
    return -1;
  }
#else
  const int range_result = close_unrelated_with_range(manifest);
  if (range_result != 0 &&
      close_unrelated_with_bounded_fallback(manifest) != 0) {
    return -1;
  }
#endif

  struct fe2o3_object_snapshot observed_manifest;
  struct fe2o3_object_snapshot observed_executable;
  if (require_sealed_regular_file(FE2O3_PREEXEC_MANIFEST_FD, false,
                                  &observed_manifest) != 0 ||
      !same_key(initial_manifest, &observed_manifest) ||
      observed_manifest.size != sizeof(*manifest) ||
      require_sealed_regular_file(FE2O3_PREEXEC_EXECUTABLE_FD, true,
                                  &observed_executable) != 0 ||
      !snapshot_matches_record(&observed_executable, &manifest->executable) ||
      same_key(&observed_manifest, &observed_executable)) {
    return -1;
  }
  for (uint32_t index = 0U; index < manifest->descriptor_count; ++index) {
    const struct fe2o3_preexec_descriptor_v1 *entry =
        &manifest->descriptors[index];
    struct fe2o3_object_snapshot observed;
    const int flags = file_control(entry->destination_fd, F_GETFD, 0L);
    if (flags < 0 || (flags & FD_CLOEXEC) != 0 ||
        descriptor_access_is_valid(entry->destination_fd,
                                   entry->destination_fd) != 0 ||
        object_snapshot(entry->destination_fd, &observed) != 0 ||
        !snapshot_matches_record(&observed, &entry->object)) {
      return -1;
    }
  }
  executable_flags = file_control(FE2O3_PREEXEC_EXECUTABLE_FD, F_GETFD, 0L);
  if (executable_flags < 0 || (executable_flags & FD_CLOEXEC) == 0) {
    return -1;
  }
  for (uint32_t index = 0U; index < FE2O3_PREEXEC_MAX_DESCRIPTORS; ++index) {
    if (!descriptor_is_closed(FE2O3_PREEXEC_SOURCE_FD_BASE + (int)index)) {
      return -1;
    }
  }
  if (close_and_verify(FE2O3_PREEXEC_MANIFEST_FD) != 0) {
    return -1;
  }
  return 0;
}

int fe2o3_preexec_entry(long argc, char **argv) {
  if (normalize_process_boundary() != 0) {
    return fail("cannot normalize process boundary");
  }
  if (argc != 1L || argv == NULL || argv[0] == NULL || argv[1] != NULL) {
    return fail("arguments are forbidden");
  }

  struct fe2o3_preexec_manifest_v1 manifest;
  struct fe2o3_object_snapshot manifest_object;
  if (validate_manifest_file(&manifest, &manifest_object) != 0) {
    return fail("invalid sealed descriptor manifest");
  }
  if (arm_parent_containment(&manifest) != 0) {
    return fail("parent identity changed while arming containment");
  }
  if (validate_descriptor_manifest(&manifest, &manifest_object) != 0) {
    return fail("descriptor identity or cardinality mismatch");
  }
  if (install_descriptor_table(&manifest, &manifest_object) != 0) {
    return fail("cannot install exact descriptor table");
  }
  if (revalidate_parent_containment(&manifest) != 0) {
    return fail("parent identity changed before target execution");
  }

  static char target_name[] = "fe2o3-protected-target";
  static char *const target_arguments[] = {target_name, NULL};
  static char *const empty_environment[] = {NULL};
  (void)linux_syscall5(SYS_execveat, FE2O3_PREEXEC_EXECUTABLE_FD,
                       pointer_argument(""), pointer_argument(target_arguments),
                       pointer_argument(empty_environment), AT_EMPTY_PATH);
  return fail("execveat of sealed target failed");
}
