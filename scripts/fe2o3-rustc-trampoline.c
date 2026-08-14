#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#ifndef AT_EMPTY_PATH
#define AT_EMPTY_PATH 0x1000
#endif
#ifndef F_ADD_SEALS
#define F_ADD_SEALS 1033
#endif
#ifndef F_GET_SEALS
#define F_GET_SEALS 1034
#endif
#ifndef F_SEAL_SEAL
#define F_SEAL_SEAL 0x0001
#endif
#ifndef F_SEAL_SHRINK
#define F_SEAL_SHRINK 0x0002
#endif
#ifndef F_SEAL_GROW
#define F_SEAL_GROW 0x0004
#endif
#ifndef F_SEAL_WRITE
#define F_SEAL_WRITE 0x0008
#endif
#ifndef MSG_CMSG_CLOEXEC
#define MSG_CMSG_CLOEXEC 0x40000000
#endif
#ifndef POLLRDHUP
#define POLLRDHUP 0x2000
#endif

#define FE2O3_BINDING_FD 189
#define FE2O3_BROKER_FD 190
#define FE2O3_WRAPPER_FD 191
#define FE2O3_BROKER_HEADER_LEN 24U
#define FE2O3_BINDING_LEN 336U
#define FE2O3_HELLO_PAYLOAD_LEN 384U
#define FE2O3_HELLO_FRAME_LEN 408U
#define FE2O3_BOOTSTRAP_PAYLOAD_LEN 96U
#define FE2O3_BOOTSTRAP_FRAME_LEN 120U
#define FE2O3_PROCESS_IDENTITY_LEN 16U
#define FE2O3_SHA256_LEN 32U
#define FE2O3_MAX_ARGUMENTS 4096U
#define FE2O3_MAX_ARGUMENT_BYTES 131072U
#define FE2O3_MAX_TOTAL_ARGUMENT_BYTES 1048576U
#define FE2O3_MAX_WRAPPER_BYTES (128U * 1024U * 1024U)
#define FE2O3_REQUIRED_SEALS                                                   \
  (F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE)

#if defined(FE2O3_RUSTC_TRAMPOLINE_TEST_ONLY)
#define FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS 350U
__attribute__((used, section(".rodata.fe2o3_test_marker"))) static const char
    fe2o3_test_only_marker[] = "FE2O3_RUSTC_TRAMPOLINE_TEST_ONLY_BUILD";
#else
#define FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS 5000U
#endif

__attribute__((used, section(".rodata.fe2o3_foundation_marker"))) static const
    char fe2o3_foundation_marker[] =
        "FE2O3_RUSTC_TRAMPOLINE_FOUNDATION_NON_AUTHORITATIVE";
__attribute__((used, section(".rodata.fe2o3_replay_gate_marker"))) static const
    char fe2o3_replay_gate_marker[] =
        "FE2O3_RUSTC_TRAMPOLINE_REPLAY_GATE_POST_EXEC_REQUIRED";
__attribute__((used, section(".rodata.fe2o3_dumpable_marker"))) static const char
    fe2o3_dumpable_marker[] =
        "FE2O3_RUSTC_TRAMPOLINE_DUMPABLE_NOT_PRESERVED_ACROSS_EXEC";
__attribute__((used, section(".rodata.fe2o3_production_blocker_marker"))) static
    const char fe2o3_production_blocker_marker[] =
        "FE2O3_RUSTC_TRAMPOLINE_PRODUCTION_BLOCKED_UNTIL_KERNEL_UNTRACEABLE_"
        "EXEC_BOUNDARY_OR_STATIC_BINDING_WRAPPER";

static const uint8_t fe2o3_broker_magic[8] = {'F', '2', 'A', 'U',
                                               'B', 'R', '3', 0};
static const uint8_t fe2o3_binding_identity_domain[] =
    "FE2O3/PROTECTED-AUTHORITY-BROKER-V3-BINDING\0";

struct sha256_context {
  uint32_t state[8];
  uint64_t bytes;
  uint8_t block[64];
  size_t used;
};

static void write_best_effort(int descriptor, const void *bytes, size_t length) {
  const ssize_t ignored = write(descriptor, bytes, length);
  (void)ignored;
}

static void fail_message(const char *message) {
  static const char prefix[] = "fe2o3-rustc-trampoline: ";
  const size_t length = strlen(message);
  write_best_effort(STDERR_FILENO, prefix, sizeof(prefix) - 1U);
  write_best_effort(STDERR_FILENO, message, length);
  write_best_effort(STDERR_FILENO, "\n", 1U);
}

static int fail(const char *message) {
  fail_message(message);
  return 125;
}

static uint32_t rotate_right(uint32_t value, unsigned int count) {
  return (value >> count) | (value << (32U - count));
}

static uint32_t load_be32(const uint8_t *input) {
  return ((uint32_t)input[0] << 24U) | ((uint32_t)input[1] << 16U) |
         ((uint32_t)input[2] << 8U) | (uint32_t)input[3];
}

static void store_be32(uint8_t *output, uint32_t value) {
  output[0] = (uint8_t)(value >> 24U);
  output[1] = (uint8_t)(value >> 16U);
  output[2] = (uint8_t)(value >> 8U);
  output[3] = (uint8_t)value;
}

static void sha256_transform(struct sha256_context *context,
                             const uint8_t block[64]) {
  static const uint32_t constants[64] = {
      0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U, 0x3956c25bU,
      0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U, 0xd807aa98U, 0x12835b01U,
      0x243185beU, 0x550c7dc3U, 0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U,
      0xc19bf174U, 0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU,
      0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU, 0x983e5152U,
      0xa831c66dU, 0xb00327c8U, 0xbf597fc7U, 0xc6e00bf3U, 0xd5a79147U,
      0x06ca6351U, 0x14292967U, 0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU,
      0x53380d13U, 0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
      0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U, 0xd192e819U,
      0xd6990624U, 0xf40e3585U, 0x106aa070U, 0x19a4c116U, 0x1e376c08U,
      0x2748774cU, 0x34b0bcb5U, 0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU,
      0x682e6ff3U, 0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U,
      0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U};
  uint32_t words[64];
  for (size_t index = 0; index < 16U; ++index) {
    words[index] = load_be32(&block[index * 4U]);
  }
  for (size_t index = 16U; index < 64U; ++index) {
    const uint32_t left = words[index - 15U];
    const uint32_t right = words[index - 2U];
    const uint32_t sigma0 =
        rotate_right(left, 7U) ^ rotate_right(left, 18U) ^ (left >> 3U);
    const uint32_t sigma1 = rotate_right(right, 17U) ^
                            rotate_right(right, 19U) ^ (right >> 10U);
    words[index] = words[index - 16U] + sigma0 + words[index - 7U] + sigma1;
  }

  uint32_t a = context->state[0];
  uint32_t b = context->state[1];
  uint32_t c = context->state[2];
  uint32_t d = context->state[3];
  uint32_t e = context->state[4];
  uint32_t f = context->state[5];
  uint32_t g = context->state[6];
  uint32_t h = context->state[7];
  for (size_t index = 0; index < 64U; ++index) {
    const uint32_t sum1 = rotate_right(e, 6U) ^ rotate_right(e, 11U) ^
                          rotate_right(e, 25U);
    const uint32_t choose = (e & f) ^ ((~e) & g);
    const uint32_t temporary1 =
        h + sum1 + choose + constants[index] + words[index];
    const uint32_t sum0 = rotate_right(a, 2U) ^ rotate_right(a, 13U) ^
                          rotate_right(a, 22U);
    const uint32_t majority = (a & b) ^ (a & c) ^ (b & c);
    const uint32_t temporary2 = sum0 + majority;
    h = g;
    g = f;
    f = e;
    e = d + temporary1;
    d = c;
    c = b;
    b = a;
    a = temporary1 + temporary2;
  }
  context->state[0] += a;
  context->state[1] += b;
  context->state[2] += c;
  context->state[3] += d;
  context->state[4] += e;
  context->state[5] += f;
  context->state[6] += g;
  context->state[7] += h;
  explicit_bzero(words, sizeof(words));
}

static void sha256_init(struct sha256_context *context) {
  const uint32_t initial[8] = {0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U,
                               0xa54ff53aU, 0x510e527fU, 0x9b05688cU,
                               0x1f83d9abU, 0x5be0cd19U};
  memcpy(context->state, initial, sizeof(initial));
  context->bytes = 0U;
  context->used = 0U;
  memset(context->block, 0, sizeof(context->block));
}

static int sha256_update(struct sha256_context *context, const void *data,
                         size_t length) {
  const uint8_t *input = data;
  if (length > UINT64_MAX - context->bytes) {
    return -1;
  }
  context->bytes += (uint64_t)length;
  while (length > 0U) {
    const size_t available = sizeof(context->block) - context->used;
    const size_t take = length < available ? length : available;
    memcpy(&context->block[context->used], input, take);
    context->used += take;
    input += take;
    length -= take;
    if (context->used == sizeof(context->block)) {
      sha256_transform(context, context->block);
      context->used = 0U;
    }
  }
  return 0;
}

static void sha256_finish(struct sha256_context *context,
                          uint8_t digest[FE2O3_SHA256_LEN]) {
  const uint64_t bit_length = context->bytes * 8U;
  context->block[context->used++] = 0x80U;
  if (context->used > 56U) {
    memset(&context->block[context->used], 0,
           sizeof(context->block) - context->used);
    sha256_transform(context, context->block);
    context->used = 0U;
  }
  memset(&context->block[context->used], 0, 56U - context->used);
  for (size_t index = 0; index < 8U; ++index) {
    context->block[63U - index] = (uint8_t)(bit_length >> (index * 8U));
  }
  sha256_transform(context, context->block);
  for (size_t index = 0; index < 8U; ++index) {
    store_be32(&digest[index * 4U], context->state[index]);
  }
  explicit_bzero(context, sizeof(*context));
}

static int sha256_fd(int descriptor, uint8_t digest[FE2O3_SHA256_LEN]) {
  struct sha256_context context;
  uint8_t buffer[32768];
  off_t offset = 0;
  sha256_init(&context);
  for (;;) {
    const ssize_t count = pread(descriptor, buffer, sizeof(buffer), offset);
    if (count < 0) {
      if (errno == EINTR) {
        continue;
      }
      explicit_bzero(buffer, sizeof(buffer));
      explicit_bzero(&context, sizeof(context));
      return -1;
    }
    if (count == 0) {
      break;
    }
    if (sha256_update(&context, buffer, (size_t)count) != 0) {
      explicit_bzero(buffer, sizeof(buffer));
      explicit_bzero(&context, sizeof(context));
      return -1;
    }
    if ((uint64_t)offset + (uint64_t)count > (uint64_t)INT64_MAX) {
      explicit_bzero(buffer, sizeof(buffer));
      explicit_bzero(&context, sizeof(context));
      return -1;
    }
    offset += count;
  }
  explicit_bzero(buffer, sizeof(buffer));
  sha256_finish(&context, digest);
  return 0;
}

static uint16_t read_le16(const uint8_t *input) {
  return (uint16_t)((uint16_t)input[0] | ((uint16_t)input[1] << 8U));
}

static uint32_t read_le32(const uint8_t *input) {
  return (uint32_t)input[0] | ((uint32_t)input[1] << 8U) |
         ((uint32_t)input[2] << 16U) | ((uint32_t)input[3] << 24U);
}

static void write_le16(uint8_t *output, uint16_t value) {
  output[0] = (uint8_t)value;
  output[1] = (uint8_t)(value >> 8U);
}

static void write_le32(uint8_t *output, uint32_t value) {
  output[0] = (uint8_t)value;
  output[1] = (uint8_t)(value >> 8U);
  output[2] = (uint8_t)(value >> 16U);
  output[3] = (uint8_t)(value >> 24U);
}

static void write_le64(uint8_t *output, uint64_t value) {
  for (size_t index = 0; index < 8U; ++index) {
    output[index] = (uint8_t)(value >> (index * 8U));
  }
}

static bool is_zero_identity(const uint8_t *identity) {
  uint8_t aggregate = 0U;
  for (size_t index = 0; index < FE2O3_SHA256_LEN; ++index) {
    aggregate |= identity[index];
  }
  return aggregate == 0U;
}

static int normalize_one_stdio(int descriptor, int required_access) {
  int flags = fcntl(descriptor, F_GETFL);
  if (flags >= 0 && (flags & O_PATH) == 0 &&
      ((flags & O_ACCMODE) == required_access ||
       (flags & O_ACCMODE) == O_RDWR)) {
    const int fd_flags = fcntl(descriptor, F_GETFD);
    if (fd_flags < 0 || fcntl(descriptor, F_SETFD, fd_flags & ~FD_CLOEXEC) != 0) {
      return -1;
    }
    return 0;
  }
  if (flags < 0 && errno != EBADF) {
    return -1;
  }
  const int replacement = (int)syscall(SYS_openat, AT_FDCWD, "/dev/null",
                                       required_access | O_CLOEXEC, 0U);
  if (replacement < 0) {
    return -1;
  }
  int result = descriptor;
  if (replacement != descriptor) {
    result = (int)syscall(SYS_dup3, replacement, descriptor, 0U);
  } else {
    const int descriptor_flags = fcntl(descriptor, F_GETFD);
    if (descriptor_flags < 0 ||
        fcntl(descriptor, F_SETFD, descriptor_flags & ~FD_CLOEXEC) != 0) {
      return -1;
    }
  }
  const int saved_errno = errno;
  if (replacement != descriptor) {
    (void)close(replacement);
  }
  errno = saved_errno;
  return result == descriptor ? 0 : -1;
}

static int normalize_process_state(void) {
  if (normalize_one_stdio(STDIN_FILENO, O_RDONLY) != 0 ||
      normalize_one_stdio(STDOUT_FILENO, O_WRONLY) != 0 ||
      normalize_one_stdio(STDERR_FILENO, O_WRONLY) != 0) {
    return -1;
  }
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_handler = SIG_DFL;
  if (sigemptyset(&action.sa_mask) != 0) {
    return -1;
  }
  for (int signal_number = 1; signal_number < NSIG; ++signal_number) {
    if (signal_number == SIGKILL || signal_number == SIGSTOP) {
      continue;
    }
    if (sigaction(signal_number, &action, NULL) != 0 && errno != EINVAL) {
      return -1;
    }
  }
  sigset_t empty_mask;
  if (sigemptyset(&empty_mask) != 0 ||
      sigprocmask(SIG_SETMASK, &empty_mask, NULL) != 0) {
    return -1;
  }
  const struct rlimit no_core = {.rlim_cur = 0, .rlim_max = 0};
  /* PR_SET_DUMPABLE protects this image only. Linux applies its dumpability
   * policy again during exec, so the dynamic wrapper must not treat this call
   * as a surviving anti-ptrace boundary. */
  if (setrlimit(RLIMIT_CORE, &no_core) != 0 ||
      prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0 ||
      prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) != 0 ||
      prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 ||
      prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1) {
    return -1;
  }
  return 0;
}

static int close_untrusted_descriptors(void) {
#if defined(SYS_close_range)
  if (syscall(SYS_close_range, 3U, FE2O3_BINDING_FD - 1U, 0U) != 0) {
    return -1;
  }
  if (syscall(SYS_close_range, FE2O3_BROKER_FD + 1U, UINT_MAX, 0U) != 0) {
    return -1;
  }
  return 0;
#else
  errno = ENOSYS;
  return -1;
#endif
}

static int validate_arguments(int argument_count, char *const arguments[]) {
  if (argument_count < 2 || (unsigned int)argument_count > FE2O3_MAX_ARGUMENTS) {
    return -1;
  }
  size_t total = 0U;
  for (int index = 0; index < argument_count; ++index) {
    if (arguments[index] == NULL) {
      return -1;
    }
    const size_t length =
        strnlen(arguments[index], FE2O3_MAX_ARGUMENT_BYTES + 1U);
    if (length == 0U || length > FE2O3_MAX_ARGUMENT_BYTES ||
        arguments[index][0] == '@' ||
        total > FE2O3_MAX_TOTAL_ARGUMENT_BYTES - length - 1U) {
      return -1;
    }
    total += length + 1U;
  }
  return 0;
}

static int validate_sealed_object(int descriptor, off_t expected_size,
                                  mode_t expected_permissions) {
  struct stat status;
  const int flags = fcntl(descriptor, F_GETFL);
  const int seals = fcntl(descriptor, F_GET_SEALS);
  if (flags < 0 || (flags & O_PATH) != 0 || (flags & O_ACCMODE) != O_RDONLY ||
      fstat(descriptor, &status) != 0 || !S_ISREG(status.st_mode) ||
      status.st_nlink != 0 || status.st_size != expected_size ||
      (status.st_mode & 07777) != expected_permissions || seals < 0 ||
      (seals & FE2O3_REQUIRED_SEALS) != FE2O3_REQUIRED_SEALS) {
    return -1;
  }
  return 0;
}

static int read_binding(uint8_t binding[FE2O3_BINDING_LEN]) {
  if (validate_sealed_object(FE2O3_BINDING_FD, (off_t)FE2O3_BINDING_LEN,
                             0444) != 0) {
    return -1;
  }
  size_t offset = 0U;
  while (offset < FE2O3_BINDING_LEN) {
    const ssize_t count = pread(FE2O3_BINDING_FD, &binding[offset],
                                FE2O3_BINDING_LEN - offset, (off_t)offset);
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

static int validate_binding(const uint8_t binding[FE2O3_BINDING_LEN]) {
  const size_t identity_offsets[] = {0U,   32U,  64U,  104U, 136U,
                                     168U, 200U, 232U, 264U};
  for (size_t index = 0;
       index < sizeof(identity_offsets) / sizeof(identity_offsets[0]); ++index) {
    if (is_zero_identity(&binding[identity_offsets[index]])) {
      return -1;
    }
  }
  if (read_le16(&binding[96]) != 1U ||
      (read_le16(&binding[98]) != 1U && read_le16(&binding[98]) != 2U) ||
      read_le32(&binding[100]) != 0U) {
    return -1;
  }
  for (size_t index = 297U; index < 304U; ++index) {
    if (binding[index] != 0U) {
      return -1;
    }
  }
  if (binding[296] == 0U) {
    if (!is_zero_identity(&binding[304])) {
      return -1;
    }
  } else if (binding[296] == 1U) {
    if (is_zero_identity(&binding[304])) {
      return -1;
    }
  } else {
    return -1;
  }
  return 0;
}

static int binding_identity(const uint8_t binding[FE2O3_BINDING_LEN],
                            uint8_t digest[FE2O3_SHA256_LEN]) {
  struct sha256_context context;
  const uint8_t encoded_length[8] = {0x50U, 0x01U, 0, 0, 0, 0, 0, 0};
  sha256_init(&context);
  if (sha256_update(&context, fe2o3_binding_identity_domain,
                    sizeof(fe2o3_binding_identity_domain) - 1U) != 0 ||
      sha256_update(&context, encoded_length, sizeof(encoded_length)) != 0 ||
      sha256_update(&context, binding, FE2O3_BINDING_LEN) != 0) {
    explicit_bzero(&context, sizeof(context));
    return -1;
  }
  sha256_finish(&context, digest);
  return 0;
}

static int current_process_identity(
    uint8_t process_identity[FE2O3_PROCESS_IDENTITY_LEN]) {
  char buffer[4096];
  const int descriptor = open("/proc/self/stat", O_RDONLY | O_CLOEXEC);
  if (descriptor < 0) {
    return -1;
  }
  ssize_t count;
  do {
    count = read(descriptor, buffer, sizeof(buffer) - 1U);
  } while (count < 0 && errno == EINTR);
  const int saved_errno = errno;
  (void)close(descriptor);
  errno = saved_errno;
  if (count <= 0 || (size_t)count >= sizeof(buffer) - 1U) {
    return -1;
  }
  buffer[count] = '\0';
  char *cursor = strrchr(buffer, ')');
  if (cursor == NULL || cursor[1] != ' ') {
    return -1;
  }
  cursor += 2;
  for (unsigned int field = 3U; field < 22U; ++field) {
    char *separator = strchr(cursor, ' ');
    if (separator == NULL) {
      return -1;
    }
    cursor = separator + 1;
  }
  errno = 0;
  char *end = NULL;
  const unsigned long long start_time = strtoull(cursor, &end, 10);
  if (errno != 0 || end == cursor ||
      (*end != ' ' && *end != '\n' && *end != '\0') || start_time == 0U) {
    return -1;
  }
  const pid_t process_id = getpid();
  if (process_id <= 0 || (uint64_t)process_id > UINT32_MAX) {
    return -1;
  }
  memset(process_identity, 0, FE2O3_PROCESS_IDENTITY_LEN);
  write_le32(process_identity, (uint32_t)process_id);
  write_le64(&process_identity[8], (uint64_t)start_time);
  explicit_bzero(buffer, sizeof(buffer));
  return 0;
}

static int current_executable_identity(uint8_t digest[FE2O3_SHA256_LEN]) {
  const int descriptor = open("/proc/self/exe", O_RDONLY | O_CLOEXEC);
  if (descriptor < 0) {
    return -1;
  }
  struct stat status;
  const int result =
      fstat(descriptor, &status) == 0 && S_ISREG(status.st_mode) &&
              status.st_size > 0 && sha256_fd(descriptor, digest) == 0
          ? 0
          : -1;
  const int saved_errno = errno;
  (void)close(descriptor);
  errno = saved_errno;
  return result;
}

static int validate_broker_socket(struct ucred *peer) {
  struct stat status;
  int socket_type = 0;
  int domain = 0;
  int accepting = 0;
  int pass_credentials = 0;
  socklen_t integer_length = (socklen_t)sizeof(int);
  socklen_t credential_length = (socklen_t)sizeof(*peer);
  struct sockaddr_storage local_address;
  struct sockaddr_storage peer_address;
  socklen_t local_length = (socklen_t)sizeof(local_address);
  socklen_t peer_length = (socklen_t)sizeof(peer_address);
  memset(peer, 0, sizeof(*peer));
  if (fstat(FE2O3_BROKER_FD, &status) != 0 || !S_ISSOCK(status.st_mode) ||
      getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_TYPE, &socket_type,
                 &integer_length) != 0 ||
      integer_length != sizeof(int) || socket_type != SOCK_SEQPACKET) {
    return -1;
  }
#ifdef SO_DOMAIN
  integer_length = (socklen_t)sizeof(int);
  if (getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_DOMAIN, &domain,
                 &integer_length) != 0 ||
      integer_length != sizeof(int) || domain != AF_UNIX) {
    return -1;
  }
#endif
  integer_length = (socklen_t)sizeof(int);
  if (getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_ACCEPTCONN, &accepting,
                 &integer_length) != 0 ||
      integer_length != sizeof(int) || accepting != 0) {
    return -1;
  }
#ifdef SO_PASSCRED
  integer_length = (socklen_t)sizeof(int);
  if (getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_PASSCRED, &pass_credentials,
                 &integer_length) != 0 ||
      integer_length != sizeof(int) || pass_credentials != 0) {
    return -1;
  }
#endif
  if (getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_PEERCRED, peer,
                 &credential_length) != 0 ||
      credential_length != sizeof(*peer) || peer->pid <= 0 ||
      peer->pid == getpid() || peer->uid != geteuid() ||
      peer->gid != getegid() ||
      getsockname(FE2O3_BROKER_FD, (struct sockaddr *)&local_address,
                  &local_length) != 0 ||
      getpeername(FE2O3_BROKER_FD, (struct sockaddr *)&peer_address,
                  &peer_length) != 0 ||
      local_length < sizeof(sa_family_t) || peer_length < sizeof(sa_family_t) ||
      local_address.ss_family != AF_UNIX || peer_address.ss_family != AF_UNIX) {
    return -1;
  }
  return 0;
}

static int wait_socket(short events) {
  struct pollfd descriptor = {.fd = FE2O3_BROKER_FD,
                              .events = events,
                              .revents = 0};
  int result;
  do {
    result = poll(&descriptor, 1, (int)FE2O3_HANDSHAKE_TIMEOUT_MILLISECONDS);
  } while (result < 0 && errno == EINTR);
  if (result != 1 || (descriptor.revents & events) == 0 ||
      (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL | POLLRDHUP)) != 0) {
    return -1;
  }
  return 0;
}

static void encode_header(uint8_t *frame, uint16_t kind, uint32_t payload_length,
                          uint32_t sequence) {
  memcpy(frame, fe2o3_broker_magic, sizeof(fe2o3_broker_magic));
  write_le16(&frame[8], 3U);
  write_le16(&frame[10], kind);
  write_le32(&frame[12], payload_length);
  write_le32(&frame[16], sequence);
  write_le32(&frame[20], 0U);
}

static int send_hello(const uint8_t process_identity[FE2O3_PROCESS_IDENTITY_LEN],
                      const uint8_t binding[FE2O3_BINDING_LEN],
                      const uint8_t trampoline_identity[FE2O3_SHA256_LEN]) {
  uint8_t frame[FE2O3_HELLO_FRAME_LEN];
  memset(frame, 0, sizeof(frame));
  encode_header(frame, 1U, FE2O3_HELLO_PAYLOAD_LEN, 0U);
  memcpy(&frame[FE2O3_BROKER_HEADER_LEN], process_identity,
         FE2O3_PROCESS_IDENTITY_LEN);
  memcpy(&frame[FE2O3_BROKER_HEADER_LEN + FE2O3_PROCESS_IDENTITY_LEN], binding,
         FE2O3_BINDING_LEN);
  memcpy(&frame[FE2O3_BROKER_HEADER_LEN + FE2O3_PROCESS_IDENTITY_LEN +
                FE2O3_BINDING_LEN],
         trampoline_identity, FE2O3_SHA256_LEN);
  if (wait_socket(POLLOUT) != 0) {
    return -1;
  }
  const ssize_t count = send(FE2O3_BROKER_FD, frame, sizeof(frame),
                             MSG_DONTWAIT | MSG_NOSIGNAL);
  explicit_bzero(frame, sizeof(frame));
  return count == (ssize_t)FE2O3_HELLO_FRAME_LEN ? 0 : -1;
}

static void close_received_rights(struct msghdr *message) {
  for (struct cmsghdr *control = CMSG_FIRSTHDR(message); control != NULL;
       control = CMSG_NXTHDR(message, control)) {
    if (control->cmsg_level != SOL_SOCKET || control->cmsg_type != SCM_RIGHTS ||
        control->cmsg_len < CMSG_LEN(0)) {
      continue;
    }
    const size_t bytes = control->cmsg_len - CMSG_LEN(0);
    const size_t count = bytes / sizeof(int);
    const int *descriptors = (const int *)CMSG_DATA(control);
    for (size_t index = 0; index < count; ++index) {
      if (descriptors[index] >= 0) {
        (void)close(descriptors[index]);
      }
    }
  }
}

static int receive_bootstrap(uint8_t frame[FE2O3_BOOTSTRAP_FRAME_LEN],
                             int *wrapper_descriptor) {
  uint8_t control_buffer[CMSG_SPACE(2U * sizeof(int))];
  struct iovec vector = {.iov_base = frame, .iov_len = FE2O3_BOOTSTRAP_FRAME_LEN};
  struct msghdr message;
  memset(&message, 0, sizeof(message));
  memset(control_buffer, 0, sizeof(control_buffer));
  message.msg_iov = &vector;
  message.msg_iovlen = 1U;
  message.msg_control = control_buffer;
  message.msg_controllen = sizeof(control_buffer);
  if (wait_socket(POLLIN) != 0) {
    return -1;
  }
  const ssize_t count = recvmsg(FE2O3_BROKER_FD, &message,
                                MSG_DONTWAIT | MSG_CMSG_CLOEXEC | MSG_TRUNC);
  if (count != (ssize_t)FE2O3_BOOTSTRAP_FRAME_LEN ||
      (message.msg_flags & (MSG_TRUNC | MSG_CTRUNC)) != 0) {
    close_received_rights(&message);
    return -1;
  }
  size_t descriptor_count = 0U;
  int received = -1;
  bool invalid_control = false;
  for (struct cmsghdr *control = CMSG_FIRSTHDR(&message); control != NULL;
       control = CMSG_NXTHDR(&message, control)) {
    if (control->cmsg_level != SOL_SOCKET || control->cmsg_type != SCM_RIGHTS ||
        control->cmsg_len < CMSG_LEN(0)) {
      invalid_control = true;
      continue;
    }
    const size_t bytes = control->cmsg_len - CMSG_LEN(0);
    if (bytes == 0U || bytes % sizeof(int) != 0U) {
      invalid_control = true;
      continue;
    }
    const size_t count_in_control = bytes / sizeof(int);
    const int *descriptors = (const int *)CMSG_DATA(control);
    for (size_t index = 0; index < count_in_control; ++index) {
      if (descriptor_count == 0U) {
        received = descriptors[index];
      } else if (descriptors[index] >= 0) {
        (void)close(descriptors[index]);
      }
      ++descriptor_count;
    }
  }
  if (invalid_control || descriptor_count != 1U || received < 0) {
    if (received >= 0) {
      (void)close(received);
    }
    return -1;
  }
  *wrapper_descriptor = received;
  return 0;
}

static int validate_bootstrap(
    const uint8_t frame[FE2O3_BOOTSTRAP_FRAME_LEN],
    const uint8_t process_identity[FE2O3_PROCESS_IDENTITY_LEN],
    const uint8_t expected_binding_identity[FE2O3_SHA256_LEN],
    uint8_t bootstrap_identity[FE2O3_SHA256_LEN]) {
  const uint8_t *payload = &frame[FE2O3_BROKER_HEADER_LEN];
  if (memcmp(frame, fe2o3_broker_magic, sizeof(fe2o3_broker_magic)) != 0 ||
      read_le16(&frame[8]) != 3U || read_le16(&frame[10]) != 2U ||
      read_le32(&frame[12]) != FE2O3_BOOTSTRAP_PAYLOAD_LEN ||
      read_le32(&frame[16]) != 1U || read_le32(&frame[20]) != 0U ||
      memcmp(payload, process_identity, FE2O3_PROCESS_IDENTITY_LEN) != 0 ||
      memcmp(&payload[16], expected_binding_identity, FE2O3_SHA256_LEN) != 0 ||
      is_zero_identity(&payload[48]) || read_le16(&payload[80]) != 1U ||
      read_le16(&payload[82]) != 1U || read_le16(&payload[84]) != 1U) {
    return -1;
  }
  for (size_t index = 86U; index < FE2O3_BOOTSTRAP_PAYLOAD_LEN; ++index) {
    if (payload[index] != 0U) {
      return -1;
    }
  }
  memcpy(bootstrap_identity, &payload[48], FE2O3_SHA256_LEN);
  return 0;
}

static int validate_wrapper(int descriptor,
                            const uint8_t expected_identity[FE2O3_SHA256_LEN]) {
  struct stat status;
  const int flags = fcntl(descriptor, F_GETFL);
  const int seals = fcntl(descriptor, F_GET_SEALS);
  uint8_t observed[FE2O3_SHA256_LEN];
  if (flags < 0 || (flags & O_PATH) != 0 || (flags & O_ACCMODE) != O_RDONLY ||
      fstat(descriptor, &status) != 0 || !S_ISREG(status.st_mode) ||
      status.st_nlink != 0 || status.st_size <= 0 ||
      (uint64_t)status.st_size > FE2O3_MAX_WRAPPER_BYTES ||
      (status.st_mode & 07777) != 0555 || seals < 0 ||
      (seals & FE2O3_REQUIRED_SEALS) != FE2O3_REQUIRED_SEALS ||
      sha256_fd(descriptor, observed) != 0) {
    return -1;
  }
  const int matches =
      memcmp(observed, expected_identity, FE2O3_SHA256_LEN) == 0 ? 0 : -1;
  explicit_bzero(observed, sizeof(observed));
  return matches;
}

/* This closes only the pre-exec queue window. The post-exec Broker V3 state
 * machine remains the authoritative replay and sequence gate. */
static int reject_pre_exec_queued_frame_or_dead_peer(
    const struct ucred *expected_peer) {
  struct pollfd descriptor = {.fd = FE2O3_BROKER_FD,
                              .events = POLLIN | POLLRDHUP,
                              .revents = 0};
  struct ucred observed;
  socklen_t length = (socklen_t)sizeof(observed);
  const int result = poll(&descriptor, 1, 0);
  if (result < 0 ||
      (result == 1 &&
       (descriptor.revents &
        (POLLIN | POLLERR | POLLHUP | POLLNVAL | POLLRDHUP)) != 0) ||
      getsockopt(FE2O3_BROKER_FD, SOL_SOCKET, SO_PEERCRED, &observed, &length) !=
          0 ||
      length != sizeof(observed) || observed.pid != expected_peer->pid ||
      observed.uid != expected_peer->uid || observed.gid != expected_peer->gid) {
    return -1;
  }
  return 0;
}

static void identity_hex(const uint8_t identity[FE2O3_SHA256_LEN],
                         char output[65]) {
  static const char digits[] = "0123456789abcdef";
  for (size_t index = 0; index < FE2O3_SHA256_LEN; ++index) {
    output[index * 2U] = digits[identity[index] >> 4U];
    output[index * 2U + 1U] = digits[identity[index] & 0x0fU];
  }
  output[64] = '\0';
}

static int set_identity_environment(char *destination, size_t capacity,
                                    const char *name,
                                    const uint8_t identity[FE2O3_SHA256_LEN]) {
  char hex[65];
  identity_hex(identity, hex);
  const int count = snprintf(destination, capacity, "%s=%s", name, hex);
  explicit_bzero(hex, sizeof(hex));
  return count > 0 && (size_t)count < capacity ? 0 : -1;
}

int main(int argument_count, char *arguments[]) {
  uint8_t binding[FE2O3_BINDING_LEN];
  uint8_t process_identity[FE2O3_PROCESS_IDENTITY_LEN];
  uint8_t trampoline_identity[FE2O3_SHA256_LEN];
  uint8_t expected_binding_identity[FE2O3_SHA256_LEN];
  uint8_t bootstrap_identity[FE2O3_SHA256_LEN];
  uint8_t bootstrap_frame[FE2O3_BOOTSTRAP_FRAME_LEN];
  struct ucred broker_peer;
  int received_wrapper = -1;

  if (clearenv() != 0) {
    return fail("cannot clear inherited environment");
  }
  if (normalize_process_state() != 0) {
    return fail("cannot normalize process state");
  }
  if (validate_arguments(argument_count, arguments) != 0) {
    return fail("invalid or ambiguous rustc argument vector");
  }
  if (close_untrusted_descriptors() != 0) {
    return fail("cannot close untrusted inherited descriptors");
  }
  if (read_binding(binding) != 0 || validate_binding(binding) != 0) {
    return fail("invalid sealed Broker V3 binding object");
  }
  if (current_process_identity(process_identity) != 0 ||
      current_executable_identity(trampoline_identity) != 0 ||
      memcmp(trampoline_identity, &binding[136], FE2O3_SHA256_LEN) != 0 ||
      binding_identity(binding, expected_binding_identity) != 0) {
    return fail("trampoline or Broker V3 binding identity mismatch");
  }
  if (validate_broker_socket(&broker_peer) != 0) {
    return fail("invalid Broker V3 peer socket");
  }
  if (close(FE2O3_BINDING_FD) != 0 ||
      send_hello(process_identity, binding, trampoline_identity) != 0) {
    return fail("cannot send canonical Broker V3 HELLO");
  }
  if (receive_bootstrap(bootstrap_frame, &received_wrapper) != 0) {
    return fail("missing or malformed Broker V3 BOOTSTRAP");
  }
  if (validate_bootstrap(bootstrap_frame, process_identity,
                         expected_binding_identity, bootstrap_identity) != 0) {
    (void)close(received_wrapper);
    return fail("Broker V3 BOOTSTRAP binding mismatch");
  }
  if (validate_wrapper(received_wrapper, &binding[168]) != 0) {
    (void)close(received_wrapper);
    return fail("invalid sealed cargo-fe2o3 wrapper executable");
  }
  if (reject_pre_exec_queued_frame_or_dead_peer(&broker_peer) != 0) {
    (void)close(received_wrapper);
    return fail("Broker V3 peer changed, exited, or queued a pre-exec frame");
  }
  if (dup3(received_wrapper, FE2O3_WRAPPER_FD, O_CLOEXEC) !=
      FE2O3_WRAPPER_FD) {
    (void)close(received_wrapper);
    return fail("cannot pin cargo-fe2o3 wrapper descriptor");
  }
  (void)close(received_wrapper);
  if (fcntl(FE2O3_BROKER_FD, F_SETFD, 0) != 0 ||
      syscall(SYS_close_range, 3U, FE2O3_BINDING_FD, 0U) != 0 ||
      syscall(SYS_close_range, FE2O3_WRAPPER_FD + 1U, UINT_MAX, 0U) != 0) {
    return fail("cannot seal wrapper descriptor table");
  }

  char binding_entry[104];
  char bootstrap_entry[106];
  char session_entry[105];
  char cargo_environment_entry[109];
  if (set_identity_environment(binding_entry, sizeof(binding_entry),
                               "FE2O3_BROKER_V3_BINDING_SHA256",
                               expected_binding_identity) != 0 ||
      set_identity_environment(bootstrap_entry, sizeof(bootstrap_entry),
                               "FE2O3_BROKER_V3_BOOTSTRAP_SHA256",
                               bootstrap_identity) != 0 ||
      set_identity_environment(session_entry, sizeof(session_entry),
                               "FE2O3_AUTHORITY_BUILD_SESSION_SHA256",
                               &binding[64]) != 0 ||
      set_identity_environment(cargo_environment_entry,
                               sizeof(cargo_environment_entry),
                               "FE2O3_AUTHORITY_CARGO_ENVIRONMENT_SHA256",
                               &binding[104]) != 0) {
    return fail("cannot encode Broker V3 wrapper environment");
  }
  char *environment[] = {
      "FE2O3_BROKER_V3_FD=190",
      binding_entry,
      bootstrap_entry,
      session_entry,
      cargo_environment_entry,
      "FE2O3_AUTHORITY_TARGET=gfx942:xnack-",
      read_le16(&binding[98]) == 1U
          ? "FE2O3_AUTHORITY_PIPELINE=collected-row-softmax-v1"
          : "FE2O3_AUTHORITY_PIPELINE=collected-tiled-gemm-v1",
      "FE2O3_TRAMPOLINE_PRE_EXEC_DUMPABLE=0",
      "FE2O3_TRAMPOLINE_PRODUCTION_STATUS=blocked-untraceable-exec-boundary-"
      "required",
      "HOME=/nonexistent",
      "LANG=C",
      "LC_ALL=C",
      "PATH=/usr/bin:/bin",
      "SOURCE_DATE_EPOCH=0",
      "TZ=UTC",
      NULL};
  char *wrapper_arguments[FE2O3_MAX_ARGUMENTS + 1U];
  wrapper_arguments[0] = (char *)"cargo-fe2o3";
  for (int index = 1; index < argument_count; ++index) {
    wrapper_arguments[index] = arguments[index];
  }
  wrapper_arguments[argument_count] = NULL;

  explicit_bzero(bootstrap_frame, sizeof(bootstrap_frame));
  explicit_bzero(trampoline_identity, sizeof(trampoline_identity));
  if (normalize_process_state() != 0) {
    return fail("cannot renormalize process state before wrapper exec");
  }
  (void)syscall(SYS_execveat, FE2O3_WRAPPER_FD, "", wrapper_arguments,
                environment, AT_EMPTY_PATH);
  return fail("execveat of sealed cargo-fe2o3 wrapper failed");
}
