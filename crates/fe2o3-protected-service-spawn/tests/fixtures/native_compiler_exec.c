/* Mechanical exec fixture, not a compiler, approved image or proof executor. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

extern char **environ;

static int bytes(int fd, const char *expected) {
    char buffer[64];
    size_t length = strlen(expected);
    return read(fd, buffer, sizeof(buffer)) == (ssize_t)length &&
           memcmp(buffer, expected, length) == 0;
}

int main(int argc, char **argv) {
    const char *arguments[] = {"captured-rustc", "repeat", "", "repeat", "two words", "\xff"};
    if (argc != 6) return 21;
    int channel = strcmp(argv[0], "captured-rustc-channel") == 0;
    if (channel) arguments[0] = "captured-rustc-channel";
    for (int i = 0; i < argc; ++i)
        if (strcmp(argv[i], arguments[i]) != 0) return 22;
    if (!environ[0] || !environ[1] || environ[2] ||
        strcmp(environ[0], "A=") || strcmp(environ[1], "B=x=y")) return 23;
    if (fcntl(2, F_GETFD) != -1 || errno != EBADF) return 24;
    for (int fd = 400; fd < 450; ++fd)
        if (fcntl(fd, F_GETFD) != -1 || errno != EBADF) return 25;
    if (channel) {
        int flags = fcntl(195, F_GETFD), type = 0;
        socklen_t length = sizeof(type);
        if (flags < 0 || (flags & FD_CLOEXEC) ||
            getsockopt(195, SOL_SOCKET, SO_TYPE, &type, &length) ||
            length != sizeof(type) || type != SOCK_SEQPACKET) return 30;
        struct ucred peer;
        length = sizeof(peer);
        if (getsockopt(195, SOL_SOCKET, SO_PEERCRED, &peer, &length) ||
            length != sizeof(peer) || peer.pid != getpid() ||
            peer.uid != getuid() || peer.gid != getgid() || peer.uid == 0) return 31;
        if (!bytes(195, "channel-request")) return 32;
        const char response[] = "channel-response";
        if (send(195, response, sizeof(response) - 1, MSG_NOSIGNAL) !=
            sizeof(response) - 1) return 33;
    }
    if (!bytes(0, "stdin-data") || !bytes(198, "binding-data")) return 26;
    int cwd = open("cwd-marker", O_RDONLY | O_CLOEXEC);
    if (cwd < 0 || !bytes(cwd, "cwd-data")) return 27;
    if (close(cwd)) return 28;
    const char output[] = "native compiler exec complete\n";
    if (write(1, output, sizeof(output) - 1) != sizeof(output) - 1) return 29;
    return 7; /* The owning tracer must preserve a real nonzero exit status. */
}
