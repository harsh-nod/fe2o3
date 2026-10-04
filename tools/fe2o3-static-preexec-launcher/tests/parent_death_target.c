#define _GNU_SOURCE

#include <fcntl.h>
#include <signal.h>
#include <stddef.h>
#include <string.h>
#include <sys/prctl.h>
#include <unistd.h>

extern char **environ;

int main(int argc, char **argv) {
  int parent_signal = 0;
  static const char ready = 'R';
  if (argc != 1 || argv == NULL || argv[0] == NULL || argv[1] != NULL ||
      strcmp(argv[0], "fe2o3-protected-target") != 0 || environ == NULL ||
      environ[0] != NULL || prctl(PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L) != 1 ||
      prctl(PR_GET_PDEATHSIG, &parent_signal, 0L, 0L, 0L) != 0 ||
      parent_signal != SIGKILL || fcntl(127, F_GETFD) < 0 ||
      write(127, &ready, 1U) != 1) {
    return 1;
  }
  for (;;) {
    (void)pause();
  }
}
