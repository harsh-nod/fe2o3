#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <linux/memfd.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/types.h>
#include <unistd.h>

#define RESULT_FD 91
#define MAX_INPUTS 2048

static const char *find_prefix(int argc, char **argv, const char *prefix) {
    size_t length = strlen(prefix);
    for (int index = 1; index < argc; ++index) {
        if (strncmp(argv[index], prefix, length) == 0) {
            return argv[index] + length;
        }
    }
    return NULL;
}

static int has_argument(int argc, char **argv, const char *argument) {
    for (int index = 1; index < argc; ++index) {
        if (strcmp(argv[index], argument) == 0) {
            return 1;
        }
    }
    return 0;
}

static int parse_inputs(int argc, char **argv, int *inputs, size_t *input_count) {
    const char *prefix = "--fe2o3-input-v1=";
    size_t prefix_length = strlen(prefix);
    *input_count = 0;
    for (int index = 1; index < argc; ++index) {
        if (strncmp(argv[index], prefix, prefix_length) != 0) {
            continue;
        }
        if (*input_count == MAX_INPUTS) {
            return -1;
        }
        char *end = NULL;
        long descriptor = strtol(argv[index] + prefix_length, &end, 10);
        if (descriptor < 100 || descriptor > 8191 || end == NULL || *end != ':') {
            return -1;
        }
        const char *kind = end + 1;
        const char *kind_end = strchr(kind, ':');
        if (kind_end == NULL ||
            !((size_t)(kind_end - kind) == strlen("elf-rel") &&
               memcmp(kind, "elf-rel", strlen("elf-rel")) == 0)) {
            return -1;
        }
        unsigned char magic[4];
        if (pread((int)descriptor, magic, sizeof(magic), 0) != (ssize_t)sizeof(magic) ||
            memcmp(magic, "\177ELF", sizeof(magic)) != 0) {
            return -1;
        }
        inputs[(*input_count)++] = (int)descriptor;
    }
    return *input_count == 0 ? -1 : 0;
}

static int descriptor_is_input(int descriptor, const int *inputs, size_t input_count) {
    for (size_t index = 0; index < input_count; ++index) {
        if (inputs[index] == descriptor) {
            return 1;
        }
    }
    return 0;
}

static int validate_descriptor_table(const int *inputs, size_t input_count) {
    for (int descriptor = 3; descriptor < 8192; ++descriptor) {
        if (fcntl(descriptor, F_GETFD) >= 0) {
            if (descriptor != RESULT_FD && !descriptor_is_input(descriptor, inputs, input_count)) {
                return -1;
            }
        } else if (errno != EBADF) {
            return -1;
        }
    }
    return 0;
}

static int validate_stdio(void) {
    struct stat expected;
    if (fstat(STDIN_FILENO, &expected) != 0 || !S_ISCHR(expected.st_mode) ||
        major(expected.st_rdev) != 1 || minor(expected.st_rdev) != 3) {
        return -1;
    }
    for (int descriptor = STDIN_FILENO; descriptor <= STDERR_FILENO; ++descriptor) {
        struct stat observed;
        int flags = fcntl(descriptor, F_GETFL);
        if (flags < 0 || (flags & O_ACCMODE) != O_RDWR || fstat(descriptor, &observed) != 0 ||
            observed.st_dev != expected.st_dev || observed.st_ino != expected.st_ino ||
            observed.st_rdev != expected.st_rdev) {
            return -1;
        }
    }
    return 0;
}

static int validate_signal_state(void) {
    sigset_t mask;
    if (sigprocmask(SIG_SETMASK, NULL, &mask) != 0) {
        return -1;
    }
    for (int signal = 1; signal < NSIG; ++signal) {
        if (sigismember(&mask, signal) == 1) {
            return -1;
        }
    }
    struct sigaction action;
    if (sigaction(SIGUSR1, NULL, &action) != 0 || action.sa_handler != SIG_DFL ||
        sigaction(SIGUSR2, NULL, &action) != 0 || action.sa_handler != SIG_DFL) {
        return -1;
    }
    return 0;
}

static int validate_result_socket(const char *identity) {
    int descriptor = -1;
    unsigned long long expected_device = 0;
    unsigned long long expected_inode = 0;
    char trailing = '\0';
    if (identity == NULL ||
        sscanf(identity, "%d:%llu:%llu%c", &descriptor, &expected_device, &expected_inode,
               &trailing) != 3 ||
        descriptor != RESULT_FD) {
        return -1;
    }
    struct stat status;
    int socket_type = 0;
    socklen_t socket_type_length = sizeof(socket_type);
    if (fstat(RESULT_FD, &status) != 0 || !S_ISSOCK(status.st_mode) ||
        getsockopt(RESULT_FD, SOL_SOCKET, SO_TYPE, &socket_type, &socket_type_length) != 0 ||
        socket_type != SOCK_SEQPACKET || (unsigned long long)status.st_dev != expected_device ||
        (unsigned long long)status.st_ino != expected_inode) {
        return -1;
    }
    return 0;
}

static void store_u64_le(unsigned char *destination, uint64_t value) {
    for (size_t index = 0; index < sizeof(value); ++index) {
        destination[index] = (unsigned char)(value >> (index * 8));
    }
}

static int send_output(const char *request, int wrong_hash, int unsealed, int second_packet,
                       int wrong_mode, int wrong_binding, int output_type,
                       int mutate_mode_after_send, size_t output_length) {
    char plan[65] = {0};
    char closure[65] = {0};
    char nonce[65] = {0};
    char trailing = '\0';
    if (request == NULL ||
        sscanf(request, "%64[0-9a-f]:%64[0-9a-f]:%64[0-9a-f]%c", plan, closure, nonce,
               &trailing) != 3 ||
        strlen(plan) != 64 || strlen(closure) != 64 || strlen(nonce) != 64) {
        return -1;
    }
    if (wrong_binding == 1) {
        plan[0] = plan[0] == '0' ? '1' : '0';
    } else if (wrong_binding == 2) {
        closure[0] = closure[0] == '0' ? '1' : '0';
    } else if (wrong_binding == 3) {
        nonce[0] = nonce[0] == '0' ? '1' : '0';
    }

    unsigned char output[121] = {0};
    memcpy(output, "\177ELF", 4);
    output[4] = 2;
    output[5] = 1;
    output[6] = 1;
    output[16] = (unsigned char)output_type;
    output[18] = 62;
    output[20] = 1;
    store_u64_le(output + 24, 0x400078);
    store_u64_le(output + 32, 64);
    output[52] = 64;
    output[54] = 56;
    output[56] = 1;
    output[58] = 64;
    output[64] = 1;
    output[68] = 5;
    store_u64_le(output + 80, 0x400000);
    store_u64_le(output + 88, 0x400000);
    store_u64_le(output + 96, sizeof(output));
    store_u64_le(output + 104, sizeof(output));
    store_u64_le(output + 112, 0x1000);
    output[120] = 0xc3;
    int output_fd = (int)syscall(SYS_memfd_create, "fe2o3-test-worker-output",
                                 MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (output_fd < 0 || output_length < sizeof(output) ||
        write(output_fd, output, sizeof(output)) != (ssize_t)sizeof(output) ||
        ftruncate(output_fd, (off_t)output_length) != 0 ||
        fchmod(output_fd, wrong_mode ? 0444 : 0555) != 0) {
        return -1;
    }
    if (!unsealed &&
        fcntl(output_fd, F_ADD_SEALS, F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_SEAL) !=
            0) {
        return -1;
    }

    const char *sha256 = output_length == 128ULL * 1024 * 1024
                             ? "52bf16eff1aa2b93cf1c3b1064dd3b5d3e5521ef7219fbb1c4fa5dd043935498"
                         : output_length == 512ULL * 1024 * 1024
                             ? "41ff37e1922f3f5e1bd6035a17a5a56b0b893f1764ebb4b375f56049e53cea8d"
                         : wrong_hash
                             ? "4444444444444444444444444444444444444444444444444444444444444444"
                         : output_type == 3
                             ? "11fb54ee7207b6ac77fec9b9d0bef075b84e97961e3a83e3af8ce643fba1c3c3"
                             : "3e1f9b21a6bf5160e1861b6d0b4770fee29f64e01a0e7af8bd8bc1ed5bb91611";
    char record[512];
    int length = snprintf(record, sizeof(record),
                          "fe2o3-host-lld-result-v1\tplan=%s\tclosure=%s\tnonce=%s\tsha256=%s\t"
                          "length=%zu\tcopy=receiver-owned-memfd-v1\n",
                          plan, closure, nonce, sha256, output_length);
    if (length <= 0 || (size_t)length >= sizeof(record)) {
        return -1;
    }

    struct iovec iov = {.iov_base = record, .iov_len = (size_t)length};
    char control[CMSG_SPACE(sizeof(int))] = {0};
    struct msghdr message = {0};
    message.msg_iov = &iov;
    message.msg_iovlen = 1;
    message.msg_control = control;
    message.msg_controllen = sizeof(control);
    struct cmsghdr *header = CMSG_FIRSTHDR(&message);
    header->cmsg_level = SOL_SOCKET;
    header->cmsg_type = SCM_RIGHTS;
    header->cmsg_len = CMSG_LEN(sizeof(int));
    memcpy(CMSG_DATA(header), &output_fd, sizeof(output_fd));
    if (sendmsg(RESULT_FD, &message, 0) != length) {
        return -1;
    }
    if (mutate_mode_after_send) {
        pid_t mutator = fork();
        if (mutator < 0) {
            return -1;
        }
        if (mutator == 0) {
            close(RESULT_FD);
            usleep(300000);
            int result = fchmod(output_fd, 0444);
            close(output_fd);
            _exit(result == 0 ? 0 : 22);
        }
    }
    if (second_packet && sendmsg(RESULT_FD, &message, 0) != length) {
        return -1;
    }
    close(output_fd);
    return shutdown(RESULT_FD, SHUT_WR);
}

int main(int argc, char **argv) {
    if (argc < 5 || strcmp(argv[1], "--fe2o3-host-lld-elf-v2") != 0) {
        return 64;
    }
    int inputs[MAX_INPUTS];
    size_t input_count = 0;
    if (validate_stdio() != 0 || validate_signal_state() != 0 ||
        parse_inputs(argc, argv, inputs, &input_count) != 0 ||
        validate_descriptor_table(inputs, input_count) != 0 ||
        validate_result_socket(find_prefix(argc, argv, "--fe2o3-result-socket-v1=")) != 0) {
        return 65;
    }
    if (has_argument(argc, argv, "--fatal-warnings")) {
        return 17;
    }
    if (has_argument(argc, argv, "--discard-all")) {
        raise(SIGKILL);
        return 18;
    }
    if (has_argument(argc, argv, "--no-undefined")) {
        return 0;
    }
    if (has_argument(argc, argv, "--build-id=none")) {
        for (;;) {
            pause();
        }
    }
    if (has_argument(argc, argv, "--hash-style=gnu")) {
        raise(SIGSTOP);
        for (;;) {
            pause();
        }
    }
    if (has_argument(argc, argv, "--static")) {
        pid_t child = fork();
        if (child < 0) {
            return 19;
        }
        if (child > 0) {
            return 0;
        }
        usleep(10000);
        int result = send_output(find_prefix(argc, argv, "--fe2o3-request-v1="), 0, 0, 0, 0, 0,
                                 2, 0, 121);
        _exit(result == 0 ? 0 : 20);
    }
    int wrong_binding = has_argument(argc, argv, "-O0")   ? 1
                        : has_argument(argc, argv, "-O1") ? 2
                        : has_argument(argc, argv, "-O2") ? 3
                                                           : 0;
    size_t output_length = 121;
    if (has_argument(argc, argv, "--no-undefined-version")) {
        output_length = 128ULL * 1024 * 1024;
        usleep(29500000);
    } else if (has_argument(argc, argv, "--no-allow-shlib-undefined")) {
        output_length = 512ULL * 1024 * 1024;
        usleep(29500000);
    }
    int result = send_output(
        find_prefix(argc, argv, "--fe2o3-request-v1="),
        has_argument(argc, argv, "--strip-debug"), has_argument(argc, argv, "-Bstatic"),
        has_argument(argc, argv, "--discard-locals"),
        has_argument(argc, argv, "--no-dynamic-linker"), wrong_binding,
        has_argument(argc, argv, "-O3") ? 3 : 2,
        has_argument(argc, argv, "--gc-sections"), output_length);
    if (result != 0) {
        return 21;
    }
    if (has_argument(argc, argv, "--eh-frame-hdr")) {
        for (;;) {
            pause();
        }
    }
    return 0;
}
