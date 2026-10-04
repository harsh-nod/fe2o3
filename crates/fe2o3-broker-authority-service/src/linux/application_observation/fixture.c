#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

/* Private qualification fixture: parent 187, app control 183, handoff 180..182. */
static long kernel_call(long number, long a, long b, long c, long d, long e, long f) {
    register long r10 __asm__("r10") = d;
    register long r8 __asm__("r8") = e;
    register long r9 __asm__("r9") = f;
    long result;
    __asm__ volatile("syscall" : "=a"(result)
        : "a"(number), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9)
        : "rcx", "r11", "memory");
    return result;
}

#define call3(number, a, b, c) kernel_call(number, (long)(a), (long)(b), (long)(c), 0, 0, 0)
#define read(fd, bytes, size) call3(SYS_read, fd, bytes, size)
#define write(fd, bytes, size) call3(SYS_write, fd, bytes, size)
#define close(fd) call3(SYS_close, fd, 0, 0)
#define fcntl(fd, command, arg) call3(SYS_fcntl, fd, command, arg)
#define dup3(source, target, flags) call3(SYS_dup3, source, target, flags)
#define open(path, flags) call3(SYS_open, path, flags, 0)
#define fork() call3(SYS_fork, 0, 0, 0)
#define getppid() call3(SYS_getppid, 0, 0, 0)
#define prctl(option, value) call3(SYS_prctl, option, value, 0)
#define sendmsg(fd, message, flags) call3(SYS_sendmsg, fd, message, flags)
#define recvmsg(fd, message, flags) call3(SYS_recvmsg, fd, message, flags)
#define waitpid(pid, status, options) call3(SYS_wait4, pid, status, options)
#define fexecve(fd, argv, env) kernel_call(SYS_execveat, fd, (long)"", (long)argv, (long)env, AT_EMPTY_PATH, 0)

static void terminate(int status) __attribute__((noreturn));
static void terminate(int status) {
    call3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
#define _exit(status) terminate(status)

static void check(int ok) { if (!ok) _exit(111); }

static void transfer(int fd, void *buffer, size_t size, int writing) {
    unsigned char *bytes = buffer;
    while (size) {
        ssize_t count = writing ? write(fd, bytes, size) : read(fd, bytes, size);
        if (count == -EINTR) continue;
        check(count > 0);
        bytes += count;
        size -= (size_t)count;
    }
}

static void ready(void) {
    char byte = 'r';
    transfer(183, &byte, 1, 1);
}

static void proof_pair(int pair[2]) {
    check(kernel_call(SYS_socketpair, AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC | SOCK_NONBLOCK,
                      0, (long)pair, 0, 0) == 0);
    for (int i = 0; i != 2; i++) {
        int enabled = 1;
        check(kernel_call(SYS_setsockopt, pair[i], SOL_SOCKET, SO_PASSCRED,
                          (long)&enabled, sizeof(enabled), 0) == 0);
        struct sockaddr address = { .sa_family = AF_UNIX };
        check(call3(SYS_bind, pair[i], &address, sizeof(address.sa_family)) == 0);
    }
}

static int parent(void) {
    int proof[2];
    proof_pair(proof);
    struct stat object;
    check(call3(SYS_fstat, proof[0], &object, 0) == 0);
    pid_t child = fork();
    check(child >= 0);
    if (!child) {
        check(prctl(PR_SET_PDEATHSIG, SIGKILL) == 0 && getppid() > 1);
        close(187);
        close(proof[1]);
        if (proof[0] != 190) { check(dup3(proof[0], 190, 0) == 190); close(proof[0]); }
        check(fcntl(190, F_SETFD, 0) == 0);
        char *argv[] = {"observed-application", NULL};
        char *env[] = {NULL};
        fexecve(188, argv, env);
        _exit(112);
    }
    close(proof[0]);
    int pidfd = (int)call3(SYS_pidfd_open, child, 0, 0);
    check(pidfd >= 0);
    uint64_t facts[] = {(uint64_t)child, object.st_dev, object.st_ino, object.st_mode};
    struct iovec iov = {facts, sizeof(facts)};
    union { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(2 * sizeof(int))]; } control = {0};
    struct msghdr message = {0};
    message.msg_iov = &iov;
    message.msg_iovlen = 1;
    message.msg_control = control.bytes;
    message.msg_controllen = sizeof(control.bytes);
    struct cmsghdr *header = CMSG_FIRSTHDR(&message);
    header->cmsg_level = SOL_SOCKET;
    header->cmsg_type = SCM_RIGHTS;
    header->cmsg_len = CMSG_LEN(2 * sizeof(int));
    *(int *)CMSG_DATA(header) = pidfd;
    ((int *)CMSG_DATA(header))[1] = proof[1];
    check(sendmsg(187, &message, MSG_NOSIGNAL) == sizeof(facts));
    close(pidfd);
    close(proof[1]);
    for (int fd = 180; fd <= 189; fd++) if (fd != 187) close(fd);
    char release;
    int alternate = -1;
    for (;;) {
        transfer(187, &release, 1, 0);
        if (release == 'q') break;
        check(release == 'p' && alternate == -1);
        int pair[2];
        proof_pair(pair);
        alternate = pair[0];
        iov.iov_base = &release;
        iov.iov_len = 1;
        message.msg_controllen = CMSG_SPACE(sizeof(int));
        header->cmsg_len = CMSG_LEN(sizeof(int));
        *(int *)CMSG_DATA(header) = pair[1];
        check(sendmsg(187, &message, MSG_NOSIGNAL) == 1);
        close(pair[1]);
    }
    if (alternate >= 0) close(alternate);
    close(187);
    int status = 0;
    pid_t waited;
    do { waited = waitpid(child, &status, 0); } while (waited == -EINTR);
    check(waited == child);
    return WIFEXITED(status) ? WEXITSTATUS(status) : 113;
}

int fixture_main(int argc, char **argv) {
    if (argc == 2 && argv[1][0] == 'p') return parent();
    for (int fd = 180; fd <= 182; fd++) check(fcntl(fd, F_SETFD, FD_CLOEXEC) == 0);
    for (int fd = 184; fd <= 186; fd++) check(fcntl(fd, F_SETFD, FD_CLOEXEC) == 0);
    check(fcntl(188, F_SETFD, FD_CLOEXEC) == 0);
    check(fcntl(190, F_SETFD, FD_CLOEXEC) == 0);
    ready();
    for (;;) {
        char command;
        transfer(183, &command, 1, 0);
        switch (command) {
        case 'H':
        case 'I': {
            uint32_t length;
            unsigned char bytes[952];
            transfer(183, &length, sizeof(length), 0);
            check(length <= sizeof(bytes));
            transfer(183, bytes, length, 0);
            struct iovec iov = {bytes, length};
            union { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(sizeof(int))]; } control = {0};
            struct msghdr message = {0};
            message.msg_iov = &iov; message.msg_iovlen = 1;
            if (command == 'I') {
                message.msg_control = control.bytes; message.msg_controllen = sizeof(control.bytes);
                struct cmsghdr *header = CMSG_FIRSTHDR(&message);
                header->cmsg_level = SOL_SOCKET; header->cmsg_type = SCM_RIGHTS;
                header->cmsg_len = CMSG_LEN(sizeof(int));
                *(int *)CMSG_DATA(header) = 180;
            }
            check(sendmsg(190, &message, MSG_NOSIGNAL) == length);
            break;
        }
        case 'R': {
            unsigned char bytes[952];
            struct iovec iov = {bytes, sizeof(bytes)};
            union { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(sizeof(struct ucred)) + CMSG_SPACE(sizeof(int))]; } control = {0};
            struct msghdr message = {0};
            message.msg_iov = &iov; message.msg_iovlen = 1;
            message.msg_control = control.bytes; message.msg_controllen = sizeof(control.bytes);
            long length = recvmsg(190, &message, MSG_CMSG_CLOEXEC);
            check(length > 0 && !(message.msg_flags & (MSG_TRUNC | MSG_CTRUNC)));
            uint32_t rights = 0;
            for (struct cmsghdr *header = CMSG_FIRSTHDR(&message); header; header = CMSG_NXTHDR(&message, header)) {
                check(header->cmsg_level == SOL_SOCKET);
                if (header->cmsg_type == SCM_RIGHTS) {
                    check(header->cmsg_len == CMSG_LEN(sizeof(int)));
                    int fd = *(int *)CMSG_DATA(header);
                    check(fcntl(fd, F_GETFD, 0) == FD_CLOEXEC);
                    close(fd); rights++;
                } else check(header->cmsg_type == SCM_CREDENTIALS);
            }
            uint32_t fields[] = {(uint32_t)length, rights};
            transfer(183, fields, sizeof(fields), 1);
            transfer(183, bytes, (size_t)length, 1);
            break;
        }
        case 'C': {
            int compiler[2];
            check(kernel_call(SYS_socketpair, AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC,
                              0, (long)compiler, 0, 0) == 0);
            check(dup3(compiler[0], 191, O_CLOEXEC) == 191);
            close(compiler[0]);
            char byte = 'c';
            struct iovec iov = {&byte, 1};
            union { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(sizeof(int))]; } control = {0};
            struct msghdr message = {0};
            message.msg_iov = &iov; message.msg_iovlen = 1;
            message.msg_control = control.bytes; message.msg_controllen = sizeof(control.bytes);
            struct cmsghdr *header = CMSG_FIRSTHDR(&message);
            header->cmsg_level = SOL_SOCKET; header->cmsg_type = SCM_RIGHTS;
            header->cmsg_len = CMSG_LEN(sizeof(int));
            *(int *)CMSG_DATA(header) = compiler[1];
            check(sendmsg(183, &message, MSG_NOSIGNAL) == 1);
            close(compiler[1]);
            break;
        }
        case 'a': {
            uint32_t length;
            unsigned char ack[512];
            transfer(183, &length, sizeof(length), 0);
            check(length && length <= sizeof(ack));
            transfer(183, ack, length, 0);
            transfer(182, ack, length, 1);
            close(182);
            int replacement = open("/dev/null", O_WRONLY | O_CLOEXEC);
            check(replacement >= 0);
            if (replacement != 182) { check(dup3(replacement, 182, O_CLOEXEC) == 182); close(replacement); }
            break;
        }
        case 'e': check(dup3(184, 180, O_CLOEXEC) == 180); break;
        case 'd': check(dup3(185, 181, O_CLOEXEC) == 181); break;
        case 'k': check(dup3(186, 182, O_CLOEXEC) == 182); break;
        case 'f': check(fcntl(180, F_SETFD, 0) == 0); break;
        case 'g': check(fcntl(181, F_SETFD, 0) == 0); break;
        case 'h': check(fcntl(182, F_SETFD, 0) == 0); break;
        case 'b': check(fcntl(182, F_SETFL, fcntl(182, F_GETFL, 0) & ~O_NONBLOCK) == 0); break;
        case 'j': check(fcntl(180, F_SETFL, fcntl(180, F_GETFL, 0) | O_APPEND) == 0); break;
        case 't': close(190); break;
        case 'm': check(write(190, "p", 1) == 1); break;
        case 'c': check(fcntl(190, F_SETFD, 0) == 0); break;
        case 'n': check(fcntl(190, F_SETFL, fcntl(190, F_GETFL, 0) & ~O_NONBLOCK) == 0); break;
        case 'v': {
            int enabled = 0;
            check(kernel_call(SYS_setsockopt, 190, SOL_SOCKET, SO_PASSCRED,
                              (long)&enabled, sizeof(enabled), 0) == 0);
            break;
        }
        case 's': check(dup3(184, 190, O_CLOEXEC) == 190); break;
        case 'o': {
            int replacement[2];
            proof_pair(replacement);
            check(dup3(replacement[0], 190, O_CLOEXEC) == 190);
            close(replacement[0]); close(replacement[1]);
            break;
        }
        case 'w':
        case 'p': {
            int flags = command == 'w' ? O_RDWR : O_PATH;
            int replacement = open("/proc/self/fd/180", flags | O_CLOEXEC);
            check(replacement >= 0 && dup3(replacement, 180, O_CLOEXEC) == 180);
            close(replacement);
            break;
        }
        case 'r': {
            for (int fd = 180; fd <= 188; fd++) {
                if (fd != 187) check(fcntl(fd, F_SETFD, 0) == 0);
            }
            check(fcntl(190, F_SETFD, 0) == 0);
            char *next_argv[] = {"observed-application", NULL};
            char *next_env[] = {NULL};
            fexecve(189, next_argv, next_env);
            return 115;
        }
        case 'x': return 0;
        default: return 114;
        }
        ready();
    }
}

__asm__(".text\n.global _start\n_start:\n"
        "mov (%rsp), %edi\nlea 8(%rsp), %rsi\ncall fixture_main\n"
        "mov %eax, %edi\nmov $231, %eax\nsyscall\nud2\n");
