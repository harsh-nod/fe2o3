#define _GNU_SOURCE

#include <fcntl.h>
#include <unistd.h>

#ifndef FE2O3_HOSTILE_MARKER_PATH
#error "FE2O3_HOSTILE_MARKER_PATH is required"
#endif

__attribute__((constructor)) static void hostile_constructor(void) {
  const int fd = open(FE2O3_HOSTILE_MARKER_PATH,
                      O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
  if (fd >= 0) {
    static const char evidence[] = "hostile constructor ran\n";
    const ssize_t ignored = write(fd, evidence, sizeof(evidence) - 1U);
    (void)ignored;
    (void)close(fd);
  }
}
