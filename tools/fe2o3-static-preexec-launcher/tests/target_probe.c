#define _GNU_SOURCE

#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <unistd.h>

extern char **environ;

struct kernel_sigaction {
  unsigned long handler;
  unsigned long flags;
  unsigned long restorer;
  unsigned long mask;
};

static int report_failure(const char *reason) {
  char message[128];
  const int length = snprintf(message, sizeof(message), "FAIL:%s\n", reason);
  if (length > 0 && (size_t)length < sizeof(message)) {
    const ssize_t ignored = write(9, message, (size_t)length);
    (void)ignored;
  }
  return 1;
}

int main(int argc, char **argv) {
  if (argc != 1 || argv == NULL || argv[0] == NULL || argv[1] != NULL ||
      strcmp(argv[0], "fe2o3-protected-target") != 0) {
    return report_failure("argv");
  }
  if (environ == NULL || environ[0] != NULL) {
    return report_failure("environment");
  }
  if (prctl(PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L) != 1) {
    return report_failure("no-new-privileges");
  }
  for (int signal_number = 1; signal_number <= 64; ++signal_number) {
    if (signal_number == SIGKILL || signal_number == SIGSTOP) {
      continue;
    }
    struct kernel_sigaction action = {0};
    if (syscall(SYS_rt_sigaction, signal_number, NULL, &action,
                sizeof(action.mask)) != 0 ||
        action.handler != (unsigned long)SIG_DFL || action.flags != 0UL ||
        action.restorer != 0UL || action.mask != 0UL) {
      return report_failure("signal-disposition");
    }
  }
  unsigned long mask = ULONG_MAX;
  if (syscall(SYS_rt_sigprocmask, SIG_SETMASK, NULL, &mask, sizeof(mask)) !=
          0 ||
      mask != 0UL) {
    return report_failure("signal-mask");
  }
  struct rlimit core_limit;
  if (getrlimit(RLIMIT_CORE, &core_limit) != 0 || core_limit.rlim_cur != 0U ||
      core_limit.rlim_max != 0U) {
    return report_failure("core-limit");
  }
  int parent_signal = 0;
  if (prctl(PR_GET_PDEATHSIG, &parent_signal, 0L, 0L, 0L) != 0 ||
      parent_signal != SIGKILL) {
    return report_failure("parent-death-signal");
  }
  for (int fd = 0; fd <= 255; ++fd) {
    const int flags = fcntl(fd, F_GETFD);
    const int expected = fd == STDIN_FILENO || fd == STDOUT_FILENO ||
                         fd == STDERR_FILENO || fd == 9;
    if ((flags >= 0) != expected) {
      return report_failure("descriptor-table");
    }
    if (expected && (flags & FD_CLOEXEC) != 0) {
      return report_failure("close-on-exec");
    }
  }
  static const char success[] = "OK\n";
  if (write(9, success, sizeof(success) - 1U) !=
      (ssize_t)(sizeof(success) - 1U)) {
    return 1;
  }
  return 0;
}
