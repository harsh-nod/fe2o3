/* Diagnostic only: the root exits while a descendant stays in its birth cgroup. */
#include <errno.h>
#include <stdint.h>
#include <sys/types.h>
#include <unistd.h>

int main(void) {
    pid_t child = fork();
    if (child < 0) return 101;
    if (child == 0) {
        close(3);
        for (;;) pause();
    }
    int32_t pid = child;
    ssize_t sent;
    do {
        sent = write(3, &pid, sizeof(pid));
    } while (sent < 0 && errno == EINTR);
    return sent == sizeof(pid) ? 0 : 102;
}
