/* Mechanical exec fixture, not a compiler, approved image or proof executor. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

/* Diagnostic modes only. No generated code is written or executed. Build with
 * -static -pthread. Memory denials kill; namespace denials return exact errno. */
static void *denied_thread(void *unused) {
    (void)unused;
    syscall(SYS_mmap, 0, 4096, PROT_READ | PROT_EXEC,
            MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    _exit(99);
}

static int restriction_probe(const char *mode) {
    if (prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1 ||
        prctl(PR_GET_SECCOMP, 0, 0, 0, 0) != 2) return 40;
    const char entered[] = "restriction probe\n";
    if (write(1, entered, sizeof(entered) - 1) != sizeof(entered) - 1) return 41;
    void *p = mmap(0, 4096, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) return 42;
    if (!strcmp(mode, "ordinary")) {
        memset(p, 0x5a, 4096);
        if (mprotect(p, 4096, PROT_READ) || mprotect(p, 4096, PROT_READ | PROT_WRITE)) return 43;
        if (munmap(p, 4096)) return 44;
        int file = open("/proc/self/exe", O_RDONLY | O_CLOEXEC);
        if (file < 0) return 45;
        p = mmap(0, 4096, PROT_READ | PROT_EXEC, MAP_PRIVATE, file, 0);
        /* This is deliberately allowed: the scalar floor is NOT ELF admission. */
        if (p == MAP_FAILED || munmap(p, 4096) || close(file)) return 46;
        int output = open("ordinary-output", O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
        if (output < 0 || write(output, "data", 4) != 4 || close(output)) return 47;
        return 7;
    }
    if (!strcmp(mode, "thread")) {
        pthread_t thread;
        if (pthread_create(&thread, 0, denied_thread, 0)) return 48;
        pthread_join(thread, 0);
        return 99;
    }
    if (!strcmp(mode, "fork") || !strcmp(mode, "fork-exec")) {
        pid_t child = fork();
        if (child < 0) return 49;
        if (!child) {
            if (!strcmp(mode, "fork-exec")) {
                char *argv[] = {"compiler-restriction", "anon-rx", 0};
                char *envp[] = {0};
                execve("/proc/self/exe", argv, envp);
                _exit(98);
            }
            denied_thread(0);
        }
        int status;
        if (waitpid(child, &status, 0) != child) return 50;
        return WIFSIGNALED(status) && WTERMSIG(status) == SIGSYS ? 7 : 51;
    }
    if (!strcmp(mode, "disable")) {
        if (!prctl(PR_SET_NO_NEW_PRIVS, 0, 0, 0, 0) ||
            !prctl(PR_SET_SECCOMP, 0, 0, 0, 0)) return 52;
        denied_thread(0);
    } else if (!strcmp(mode, "anon-rx")) {
        denied_thread(0);
    } else if (!strcmp(mode, "anon-rwx")) {
        syscall(SYS_mmap, 0, 4096, PROT_READ | PROT_WRITE | PROT_EXEC,
                MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    } else if (!strcmp(mode, "mprotect")) {
        syscall(SYS_mprotect, p, 4096, PROT_READ | PROT_EXEC);
    } else if (!strcmp(mode, "pkey-mprotect")) {
        syscall(SYS_pkey_mprotect, p, 4096, PROT_READ | PROT_EXEC, -1);
    } else if (!strcmp(mode, "file-rwx") || !strcmp(mode, "restore-exec")) {
        int file = open("/proc/self/exe", O_RDONLY | O_CLOEXEC);
        if (file < 0) return 53;
        if (!strcmp(mode, "file-rwx")) {
            syscall(SYS_mmap, 0, 4096, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE, file, 0);
        } else {
            p = mmap(0, 4096, PROT_READ | PROT_EXEC, MAP_PRIVATE, file, 0);
            if (p == MAP_FAILED || mprotect(p, 4096, PROT_READ | PROT_WRITE)) return 54;
            ((volatile unsigned char *)p)[0] ^= 1;
            syscall(SYS_mprotect, p, 4096, PROT_READ | PROT_EXEC);
        }
    } else if (!strcmp(mode, "personality")) {
        syscall(SYS_personality, 0x00400000);
    } else if (!strcmp(mode, "userfaultfd")) {
        syscall(SYS_userfaultfd, O_CLOEXEC);
    } else if (!strcmp(mode, "io-uring")) {
        syscall(SYS_io_uring_setup, 0, 0);
    } else {
        return 55;
    }
    return 99; /* Any ordinary syscall return means the restriction was absent. */
}

static int bytes(int fd, const char *expected) {
    char buffer[64];
    size_t length = strlen(expected);
    return read(fd, buffer, sizeof(buffer)) == (ssize_t)length &&
           memcmp(buffer, expected, length) == 0;
}

static int namespace_checks(void) {
    if (prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1 ||
        prctl(PR_GET_SECCOMP, 0, 0, 0, 0) != 2) return 60;
    /* These inert arguments distinguish the filter from ordinary kernel checks:
     * unshare(0) succeeds, setns(-1) is EBADF, clone3(NULL, 0) is EINVAL. */
    errno = 0;
    if (syscall(SYS_unshare, 0) != -1 || errno != EPERM) return 61;
    errno = 0;
    if (syscall(SYS_setns, -1, 0) != -1 || errno != EPERM) return 62;
    errno = 0;
    if (syscall(SYS_clone3, 0, 0) != -1 || errno != ENOSYS) return 63;
    const unsigned long flags[] = {0x80, 0x20000, 0x2000000, 0x4000000,
        0x8000000, 0x10000000, 0x20000000, 0x40000000};
    for (unsigned i = 0; i < sizeof(flags) / sizeof(flags[0]); ++i) {
        errno = 0;
        long child = syscall(SYS_clone, flags[i] | SIGCHLD, 0, 0, 0, 0);
        int observed = errno;
        if (!child) _exit(64);
        if (child > 0) {
            int status;
            while (waitpid((pid_t)child, &status, 0) < 0 && errno == EINTR) {}
            return 65;
        }
        if (child != -1 || observed != EPERM) return 66;
    }
    return 0;
}

static void *namespace_thread(void *result) {
    *(int *)result = namespace_checks();
    return 0;
}

static int namespace_probe(int output) {
    const char entered[] = "namespace probe\n";
    if (write(output, entered, sizeof(entered) - 1) != sizeof(entered) - 1) return 67;
    int result = namespace_checks();
    if (result) return result;
    pthread_t thread;
    result = -1;
    if (pthread_create(&thread, 0, namespace_thread, &result) ||
        pthread_join(thread, 0) || result) return 68;
    for (int reexec = 0; reexec < 2; ++reexec) {
        pid_t child = fork();
        if (child < 0) return 69;
        if (!child) {
            if (reexec) {
                char *argv[] = {"namespace-child", 0};
                char *envp[] = {0};
                execve("/proc/self/exe", argv, envp);
                _exit(70);
            }
            _exit(namespace_checks() ? 71 : 7);
        }
        int status;
        pid_t waited;
        do { waited = waitpid(child, &status, 0); } while (waited < 0 && errno == EINTR);
        if (waited != child || !WIFEXITED(status) || WEXITSTATUS(status) != 7) return 72;
    }
    const char completed[] = "namespace thread fork exec complete\n";
    if (write(output, completed, sizeof(completed) - 1) != sizeof(completed) - 1) return 73;
    return 7;
}

int main(int argc, char **argv) {
    if (argc == 1 && !strcmp(argv[0], "namespace-child"))
        return namespace_checks() ? 74 : 7;
    if (argc == 1 && !strcmp(argv[0], "fe2o3-protected-service"))
        return namespace_probe(198);
    if (argc == 2 && !strcmp(argv[0], "compiler-restriction") &&
        !strcmp(argv[1], "namespace")) return namespace_probe(1);
    if (argc == 2 && !strcmp(argv[0], "compiler-restriction"))
        return restriction_probe(argv[1]);
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
