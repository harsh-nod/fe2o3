#include "SecureProtocol.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <elf.h>
#include <fcntl.h>
#include <limits>
#include <linux/close_range.h>
#include <linux/memfd.h>
#include <poll.h>
#include <signal.h>
#include <string>
#include <string_view>
#include <sys/inotify.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>
#include <vector>

namespace {

using namespace fe2o3::host_lld;

constexpr char PlanDigest[] =
    "1111111111111111111111111111111111111111111111111111111111111111";
constexpr char ClosureDigest[] =
    "2222222222222222222222222222222222222222222222222222222222222222";
constexpr char NonceDigest[] =
    "3333333333333333333333333333333333333333333333333333333333333333";
constexpr int LinuxX86KernelRealtimeMinimum = 32;
constexpr int LinuxX86KernelSignalMaximum = 64;
constexpr size_t LinuxX86KernelSignalSetBytes = sizeof(uint64_t);

[[noreturn]] void fail(const char *message) {
  std::fprintf(stderr, "fd-link-harness: %s: %s\n", message,
               std::strerror(errno));
  std::exit(1);
}

[[noreturn]] void failMessage(const std::string &message) {
  std::fprintf(stderr, "fd-link-harness: %s\n", message.c_str());
  std::exit(1);
}

void closeAmbientDescriptors() {
#ifdef SYS_close_range
  if (::syscall(SYS_close_range, 3U, std::numeric_limits<unsigned>::max(),
                0U) == 0)
    return;
  if (errno != ENOSYS && errno != EINVAL)
    fail("close_range failed");
#endif
  rlimit limit{};
  if (::getrlimit(RLIMIT_NOFILE, &limit) != 0)
    fail("getrlimit failed");
  const rlim_t maximum =
      limit.rlim_cur == RLIM_INFINITY ? 65536 : limit.rlim_cur;
  for (rlim_t descriptor = 3; descriptor < maximum; ++descriptor)
    (void)::close(static_cast<int>(descriptor));
}

uint32_t rotateRight(uint32_t value, unsigned count) {
  return (value >> count) | (value << (32U - count));
}

class Sha256 {
public:
  void update(const uint8_t *bytes, size_t length) {
    Total += static_cast<uint64_t>(length);
    while (length != 0) {
      const size_t copied = std::min(length, Block.size() - Used);
      std::memcpy(Block.data() + Used, bytes, copied);
      Used += copied;
      bytes += copied;
      length -= copied;
      if (Used == Block.size()) {
        compress(Block.data());
        Used = 0;
      }
    }
  }

  std::array<uint8_t, 32> final() {
    const uint64_t bitLength = Total * 8U;
    Block[Used++] = 0x80;
    if (Used > 56) {
      std::fill(Block.begin() + static_cast<ptrdiff_t>(Used), Block.end(), 0);
      compress(Block.data());
      Used = 0;
    }
    std::fill(Block.begin() + static_cast<ptrdiff_t>(Used), Block.begin() + 56,
              0);
    for (unsigned index = 0; index != 8; ++index)
      Block[63U - index] =
          static_cast<uint8_t>(bitLength >> static_cast<unsigned>(index * 8U));
    compress(Block.data());
    std::array<uint8_t, 32> digest{};
    for (size_t index = 0; index != State.size(); ++index)
      for (unsigned byte = 0; byte != 4; ++byte)
        digest[index * 4U + byte] = static_cast<uint8_t>(
            State[index] >> static_cast<unsigned>(24U - byte * 8U));
    return digest;
  }

private:
  void compress(const uint8_t *block) {
    static constexpr std::array<uint32_t, 64> Constants = {
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
        0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U,
    };
    std::array<uint32_t, 64> words{};
    for (size_t index = 0; index != 16; ++index)
      words[index] = (static_cast<uint32_t>(block[index * 4U]) << 24U) |
                     (static_cast<uint32_t>(block[index * 4U + 1U]) << 16U) |
                     (static_cast<uint32_t>(block[index * 4U + 2U]) << 8U) |
                     static_cast<uint32_t>(block[index * 4U + 3U]);
    for (size_t index = 16; index != words.size(); ++index) {
      const uint32_t lower = rotateRight(words[index - 15U], 7U) ^
                             rotateRight(words[index - 15U], 18U) ^
                             (words[index - 15U] >> 3U);
      const uint32_t upper = rotateRight(words[index - 2U], 17U) ^
                             rotateRight(words[index - 2U], 19U) ^
                             (words[index - 2U] >> 10U);
      words[index] = words[index - 16U] + lower + words[index - 7U] + upper;
    }
    uint32_t a = State[0];
    uint32_t b = State[1];
    uint32_t c = State[2];
    uint32_t d = State[3];
    uint32_t e = State[4];
    uint32_t f = State[5];
    uint32_t g = State[6];
    uint32_t h = State[7];
    for (size_t index = 0; index != words.size(); ++index) {
      const uint32_t sum1 =
          rotateRight(e, 6U) ^ rotateRight(e, 11U) ^ rotateRight(e, 25U);
      const uint32_t choice = (e & f) ^ ((~e) & g);
      const uint32_t temporary1 =
          h + sum1 + choice + Constants[index] + words[index];
      const uint32_t sum0 =
          rotateRight(a, 2U) ^ rotateRight(a, 13U) ^ rotateRight(a, 22U);
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
    State[0] += a;
    State[1] += b;
    State[2] += c;
    State[3] += d;
    State[4] += e;
    State[5] += f;
    State[6] += g;
    State[7] += h;
  }

  std::array<uint32_t, 8> State = {0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U,
                                   0xa54ff53aU, 0x510e527fU, 0x9b05688cU,
                                   0x1f83d9abU, 0x5be0cd19U};
  std::array<uint8_t, 64> Block{};
  size_t Used = 0;
  uint64_t Total = 0;
};

std::string lowercaseHex(const std::array<uint8_t, 32> &bytes) {
  static constexpr char Digits[] = "0123456789abcdef";
  std::string result;
  result.reserve(64);
  for (uint8_t byte : bytes) {
    result.push_back(Digits[byte >> 4U]);
    result.push_back(Digits[byte & 0x0fU]);
  }
  return result;
}

std::string sha256(const std::vector<uint8_t> &bytes) {
  Sha256 hasher;
  hasher.update(bytes.data(), bytes.size());
  return lowercaseHex(hasher.final());
}

void appendArchiveField(std::vector<uint8_t> &archive, const std::string &value,
                        size_t width) {
  if (value.size() > width)
    failMessage("archive test field exceeds its canonical width");
  archive.insert(archive.end(), value.begin(), value.end());
  archive.insert(archive.end(), width - value.size(), ' ');
}

std::vector<uint8_t> archiveMemberFlood(const std::vector<uint8_t> &object) {
  constexpr size_t MemberCount = 262145;
  constexpr std::string_view Magic = "!<arch>\n";
  const size_t paddedObjectSize = object.size() + (object.size() & 1U);
  const size_t memberSize = 60U + paddedObjectSize;
  if (memberSize > (256ULL * 1024 * 1024 - Magic.size()) / MemberCount)
    failMessage("archive-member flood exceeds the per-input test bound");
  std::vector<uint8_t> archive;
  archive.reserve(Magic.size() + memberSize * MemberCount);
  archive.insert(archive.end(), Magic.begin(), Magic.end());
  for (size_t index = 0; index != MemberCount; ++index) {
    appendArchiveField(archive, "member.o/", 16);
    appendArchiveField(archive, "0", 12);
    appendArchiveField(archive, "0", 6);
    appendArchiveField(archive, "0", 6);
    appendArchiveField(archive, "100644", 8);
    appendArchiveField(archive, std::to_string(object.size()), 10);
    archive.push_back('`');
    archive.push_back('\n');
    archive.insert(archive.end(), object.begin(), object.end());
    if ((object.size() & 1U) != 0U)
      archive.push_back('\n');
  }
  return archive;
}

void corruptDependentSectionMetadata(std::vector<uint8_t> &object,
                                     const std::string &mode) {
  constexpr uint32_t DependentLibrariesType = 0x6fff4c04U;
  if (object.size() < sizeof(Elf64_Ehdr))
    failMessage("dependent-library fixture lacks an ELF header");
  Elf64_Ehdr header{};
  std::memcpy(&header, object.data(), sizeof(header));
  if (std::memcmp(header.e_ident, ELFMAG, SELFMAG) != 0 ||
      header.e_ident[EI_CLASS] != ELFCLASS64 ||
      header.e_shentsize != sizeof(Elf64_Shdr) || header.e_shnum == 0 ||
      header.e_shoff > object.size() ||
      static_cast<uint64_t>(header.e_shnum) >
          (object.size() - static_cast<size_t>(header.e_shoff)) /
              sizeof(Elf64_Shdr))
    failMessage("dependent-library fixture has invalid section metadata");
  bool found = false;
  for (uint16_t index = 0; index != header.e_shnum; ++index) {
    const size_t offset = static_cast<size_t>(header.e_shoff) +
                          static_cast<size_t>(index) * sizeof(Elf64_Shdr);
    Elf64_Shdr section{};
    std::memcpy(&section, object.data() + offset, sizeof(section));
    if (section.sh_type != DependentLibrariesType)
      continue;
    found = true;
    if (mode == "deplibs-malformed-name") {
      section.sh_name = std::numeric_limits<uint32_t>::max();
      std::memcpy(object.data() + offset, &section, sizeof(section));
    }
  }
  if (!found)
    failMessage("dependent-library fixture lacks the canonical section type");
  if (mode == "deplibs-malformed-shstr") {
    header.e_shstrndx = header.e_shnum;
    std::memcpy(object.data(), &header, sizeof(header));
  }
}

void writeAll(int fd, const uint8_t *bytes, size_t length) {
  while (length != 0) {
    const ssize_t written = ::write(fd, bytes, length);
    if (written < 0) {
      if (errno == EINTR)
        continue;
      fail("write failed");
    }
    if (written == 0)
      fail("write made no progress");
    bytes += static_cast<size_t>(written);
    length -= static_cast<size_t>(written);
  }
}

std::vector<uint8_t> readFile(const char *path) {
  const int fd = ::open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (fd < 0)
    fail("input open failed");
  struct stat status{};
  if (::fstat(fd, &status) != 0 || !S_ISREG(status.st_mode) ||
      status.st_size < 0 ||
      static_cast<uint64_t>(status.st_size) > 2ULL * 1024 * 1024 * 1024)
    fail("input is not a bounded regular file");
  std::vector<uint8_t> bytes(static_cast<size_t>(status.st_size));
  size_t offset = 0;
  while (offset != bytes.size()) {
    const ssize_t count =
        ::read(fd, bytes.data() + offset, bytes.size() - offset);
    if (count < 0) {
      if (errno == EINTR)
        continue;
      fail("input read failed");
    }
    if (count == 0)
      fail("input ended early");
    offset += static_cast<size_t>(count);
  }
  if (::close(fd) != 0)
    fail("input close failed");
  return bytes;
}

int sealedInput(const std::vector<uint8_t> &bytes, mode_t mode) {
  const int fd =
      static_cast<int>(::syscall(SYS_memfd_create, "fe2o3-host-lld-input",
                                 MFD_ALLOW_SEALING | MFD_CLOEXEC));
  if (fd < 0)
    fail("input memfd_create failed");
  writeAll(fd, bytes.data(), bytes.size());
  if (::fchmod(fd, mode) != 0 || ::lseek(fd, 0, SEEK_SET) != 0)
    fail("input mode or rewind failed");
  if (::fcntl(fd, F_ADD_SEALS, static_cast<int>(InputSeals)) != 0 ||
      ::fcntl(fd, F_GET_SEALS) != static_cast<int>(InputSeals))
    fail("input sealing failed");
  return fd;
}

void clearCloseOnExec(int fd) {
  const int flags = ::fcntl(fd, F_GETFD);
  if (flags < 0 || ::fcntl(fd, F_SETFD, flags & ~FD_CLOEXEC) != 0)
    fail("could not inherit descriptor");
}

void moveDescriptor(int source, int destination) {
  if (source != destination) {
    if (::dup2(source, destination) != destination)
      fail("descriptor placement failed");
    if (::close(source) != 0)
      fail("descriptor source close failed");
  }
  clearCloseOnExec(destination);
}

void setNonblocking(int fd, bool enabled) {
  const int flags = ::fcntl(fd, F_GETFL);
  if (flags < 0 ||
      ::fcntl(fd, F_SETFL,
              enabled ? flags | O_NONBLOCK : flags & ~O_NONBLOCK) != 0)
    fail("could not set descriptor blocking policy");
}

using KernelSignalHandler = void (*)(int);
using KernelSignalRestorer = void (*)();

struct LinuxX86KernelSigaction {
  KernelSignalHandler Handler;
  unsigned long Flags;
  KernelSignalRestorer Restorer;
  uint64_t Mask;
};

static_assert(sizeof(LinuxX86KernelSigaction) == 32);
static_assert(offsetof(LinuxX86KernelSigaction, Mask) == 24);

void installIgnoredKernelSignal(int signal) {
  const LinuxX86KernelSigaction action{SIG_IGN, 0, nullptr, 0};
  if (::syscall(SYS_rt_sigaction, signal, &action, nullptr,
                LinuxX86KernelSignalSetBytes) != 0)
    fail("hostile signal disposition setup failed");
}

uint64_t kernelSignalBit(int signal) {
  if (signal < 1 || signal > LinuxX86KernelSignalMaximum)
    failMessage("hostile signal is outside the Linux x86-64 kernel range");
  return UINT64_C(1) << static_cast<unsigned>(signal - 1);
}

int hostileSignalTarget(const std::string &mode) {
  if (mode == "hostile-signal-state" ||
      mode == "hostile-signal-state-kill")
    return SIGXCPU;
  if (mode == "hostile-signal-rtmin" ||
      mode == "hostile-signal-rtmin-kill")
    return SIGRTMIN;
  if (mode == "hostile-signal-rtmax" ||
      mode == "hostile-signal-rtmax-kill")
    return SIGRTMAX;
  if (mode == "hostile-signal-kernel-reserved-32" ||
      mode == "hostile-signal-kernel-reserved-32-kill")
    return LinuxX86KernelRealtimeMinimum;
  if (mode == "hostile-signal-kernel-reserved-33" ||
      mode == "hostile-signal-kernel-reserved-33-kill")
    return LinuxX86KernelRealtimeMinimum + 1;
  return 0;
}

bool hostileSignalKillMode(const std::string &mode) {
  return mode == "hostile-signal-state-kill" ||
         mode == "hostile-signal-rtmin-kill" ||
         mode == "hostile-signal-rtmax-kill" ||
         mode == "hostile-signal-kernel-reserved-32-kill" ||
         mode == "hostile-signal-kernel-reserved-33-kill";
}

void installHostileSignalState(const std::string &mode) {
  const int target = hostileSignalTarget(mode);
  if (target == 0)
    failMessage("unknown hostile signal profile");
  uint64_t blocked = kernelSignalBit(target);
  installIgnoredKernelSignal(target);
  if (target == SIGXCPU) {
    installIgnoredKernelSignal(SIGPIPE);
    blocked |= kernelSignalBit(SIGPIPE);
  }
  if (::syscall(SYS_rt_sigprocmask, SIG_SETMASK, &blocked, nullptr,
                LinuxX86KernelSignalSetBytes) != 0)
    fail("hostile signal mask setup failed");
}

std::string inputKind(const std::vector<uint8_t> &bytes,
                      const std::string &mode) {
  if (mode == "force-rlib" || mode == "deplibs-rlib-attack")
    return "rlib";
  if (bytes.size() >= SELFMAG &&
      std::memcmp(bytes.data(), ELFMAG, SELFMAG) == 0 &&
      bytes.size() >= sizeof(Elf64_Ehdr)) {
    Elf64_Ehdr header{};
    std::memcpy(&header, bytes.data(), sizeof(header));
    if (header.e_type == ET_DYN)
      return "elf-dso";
    return "elf-rel";
  }
  if (bytes.size() >= 8 && (std::memcmp(bytes.data(), "!<arch>\n", 8) == 0 ||
                            std::memcmp(bytes.data(), "!<thin>\n", 8) == 0))
    return mode == "force-rlib" || mode == "deplibs-rlib-attack" ? "rlib"
                                                                    : "archive";
  return "elf-rel";
}

struct ChildInvocation {
  pid_t Pid = -1;
  int ResultSocket = -1;
  int BlockedPipeReader = -1;
};

ChildInvocation launch(const char *tool, const std::vector<uint8_t> &bytes,
                       const std::string &mode,
                       const std::vector<uint8_t> *additionalRlib) {
  const mode_t inputMode = 0644;
  int input = sealedInput(bytes, inputMode);
  int rlib = -1;
  if (additionalRlib != nullptr)
    rlib = sealedInput(*additionalRlib, inputMode);
  std::vector<int> totalBoundInputs;
  if (mode == "total-input-metadata")
    for (size_t index = 0; index != 8; ++index)
      totalBoundInputs.push_back(sealedInput(bytes, inputMode));
  int sockets[2] = {-1, -1};
  int blockedPipe[2] = {-1, -1};
  if (mode == "stdio-blocked-pipe") {
    if (::pipe2(blockedPipe, O_CLOEXEC | O_NONBLOCK) != 0)
      fail("blocked standard-descriptor pipe creation failed");
    std::array<uint8_t, 4096> filler{};
    while (::write(blockedPipe[1], filler.data(), filler.size()) >= 0) {
    }
    if (errno != EAGAIN && errno != EWOULDBLOCK)
      fail("blocked standard-descriptor pipe prefill failed");
    setNonblocking(blockedPipe[1], false);
  }
  const int socketType =
      mode == "wrong-socket-type" ? SOCK_STREAM : SOCK_SEQPACKET;
  if (::socketpair(AF_UNIX, socketType | SOCK_CLOEXEC, 0, sockets) != 0)
    fail("result socketpair failed");
  setNonblocking(sockets[1], mode != "blocking-result-socket");
  if (mode == "prefilled-result-queue") {
    std::array<uint8_t, 512> filler{};
    while (::send(sockets[1], filler.data(), filler.size(),
                  MSG_DONTWAIT | MSG_NOSIGNAL) >= 0) {
    }
    if (errno != EAGAIN && errno != EWOULDBLOCK)
      fail("result queue prefill failed");
  }
  if (mode == "no-result-reader" && ::shutdown(sockets[0], SHUT_RD) != 0)
    fail("result reader shutdown failed");
  const pid_t child = ::fork();
  if (child < 0)
    fail("fork failed");
  const std::string inputHash = sha256(bytes);
  if (child == 0) {
    if (::close(sockets[0]) != 0)
      fail("parent socket close failed");
    moveDescriptor(input, FirstInputFd);
    if (rlib >= 0)
      moveDescriptor(rlib, FirstInputFd + 1);
    for (size_t index = 0; index != totalBoundInputs.size(); ++index)
      moveDescriptor(totalBoundInputs[index],
                     FirstInputFd + 1 + static_cast<int>(index));
    moveDescriptor(sockets[1], ResultSocketFd);
    if (mode == "stdio-result-alias") {
      for (int descriptor = STDIN_FILENO; descriptor <= STDERR_FILENO;
           ++descriptor)
        if (::dup2(ResultSocketFd, descriptor) != descriptor)
          fail("could not construct a standard-descriptor alias");
    }
    if (mode == "stdio-input-alias") {
      for (int descriptor = STDIN_FILENO; descriptor <= STDERR_FILENO;
           ++descriptor)
        if (::dup2(FirstInputFd, descriptor) != descriptor)
          fail("could not construct an input standard-descriptor alias");
    }
    if (mode == "stdio-blocked-pipe") {
      if (::close(blockedPipe[0]) != 0)
        fail("blocked pipe reader close failed");
      for (int descriptor = STDIN_FILENO; descriptor <= STDERR_FILENO;
           ++descriptor)
        if (::dup2(blockedPipe[1], descriptor) != descriptor)
          fail("could not construct blocked standard descriptors");
      if (::close(blockedPipe[1]) != 0)
        fail("blocked pipe writer close failed");
    }
    int unexpected = -1;
    if (mode == "extra-fd") {
      unexpected = ::open("/dev/null", O_RDONLY);
      if (unexpected < 0)
        fail("unexpected descriptor open failed");
      clearCloseOnExec(unexpected);
    }
    struct stat socketStatus{};
    if (::fstat(ResultSocketFd, &socketStatus) != 0)
      fail("result socket identity failed");
    uint64_t socketInode = static_cast<uint64_t>(socketStatus.st_ino);
    if (mode == "wrong-socket-identity")
      ++socketInode;
    std::string hash = inputHash;
    if (mode == "wrong-hash")
      hash[0] = hash[0] == '0' ? '1' : '0';
    uint64_t size = static_cast<uint64_t>(bytes.size());
    if (mode == "wrong-size")
      ++size;
    if (mode == "oversized-input-metadata")
      size = 268435457;
    if (mode == "total-input-metadata")
      size = 268435456;
    const std::string kind =
        mode == "wrong-kind" ? "elf-dso" : inputKind(bytes, mode);
    const std::string inputRecord =
        std::string(InputPrefix) + std::to_string(FirstInputFd) + ":" + kind +
        ":" + hash + ":" + std::to_string(size) + ":0644";
    std::string rlibRecord;
    if (additionalRlib != nullptr)
      rlibRecord = std::string(InputPrefix) + std::to_string(FirstInputFd + 1) +
                   ":rlib:" + sha256(*additionalRlib) + ":" +
                   std::to_string(additionalRlib->size()) + ":0644";
    const std::string socketRecord =
        std::string(ResultSocketPrefix) + std::to_string(ResultSocketFd) + ":" +
        std::to_string(static_cast<uint64_t>(socketStatus.st_dev)) + ":" +
        std::to_string(socketInode);
    const std::string requestRecord = std::string(RequestPrefix) + PlanDigest +
                                      ":" + ClosureDigest + ":" + NonceDigest;
    const std::string bareInput =
        "/proc/self/fd/" + std::to_string(FirstInputFd);
    std::string duplicateRecord;
    std::vector<std::string> totalBoundRecords;
    if (mode == "total-input-metadata") {
      for (size_t index = 0; index != totalBoundInputs.size(); ++index) {
        totalBoundRecords.push_back(
            std::string(InputPrefix) +
            std::to_string(FirstInputFd + 1 + static_cast<int>(index)) +
            ":elf-rel:" + inputHash + ":268435456:0644");
      }
    }
    std::vector<char *> arguments = {
        const_cast<char *>(tool),
        const_cast<char *>(ProtocolArgument),
        const_cast<char *>("-flavor"),
        const_cast<char *>("gnu"),
        const_cast<char *>("-m"),
        const_cast<char *>("elf_x86_64"),
        const_cast<char *>("-static"),
        const_cast<char *>("--build-id=none"),
        const_cast<char *>("--no-dynamic-linker"),
        const_cast<char *>("--fatal-warnings"),
        const_cast<char *>("-z"),
        const_cast<char *>("noexecstack"),
        const_cast<char *>(socketRecord.c_str()),
        const_cast<char *>(requestRecord.c_str()),
        const_cast<char *>(
            (mode == "bare-fd" ? bareInput : inputRecord).c_str()),
    };
    if (mode == "duplicate-input" || mode == "conflicting-duplicate") {
      duplicateRecord = inputRecord;
      if (mode == "conflicting-duplicate") {
        const size_t kindOffset = duplicateRecord.find(":elf-rel:");
        if (kindOffset == std::string::npos)
          failMessage("conflicting duplicate test requires an ELF object");
        duplicateRecord.replace(kindOffset + 1U, 7U, "elf-dso");
      }
      arguments.push_back(const_cast<char *>(duplicateRecord.c_str()));
    }
    if (additionalRlib != nullptr)
      arguments.push_back(const_cast<char *>(rlibRecord.c_str()));
    for (std::string &record : totalBoundRecords)
      arguments.push_back(record.data());
    if (mode == "user-mmap-option")
      arguments.push_back(const_cast<char *>("--mmap-output-file"));
    if (mode == "user-threads-option")
      arguments.push_back(const_cast<char *>("--threads=1"));
    if (mode == "caller-dependent-libraries")
      arguments.push_back(const_cast<char *>("--dependent-libraries"));
    if (mode == "caller-no-dependent-libraries")
      arguments.push_back(const_cast<char *>("--no-dependent-libraries"));
    arguments.push_back(nullptr);
    if (hostileSignalTarget(mode) != 0)
      installHostileSignalState(mode);
    char *const environment[] = {nullptr};
    ::execve(tool, arguments.data(), environment);
    fail("host LLD exec failed");
  }
  if (::close(input) != 0 || (rlib >= 0 && ::close(rlib) != 0) ||
      ::close(sockets[1]) != 0)
    fail("child descriptor close failed");
  if (blockedPipe[1] >= 0 && ::close(blockedPipe[1]) != 0)
    fail("parent blocked pipe writer close failed");
  for (int descriptor : totalBoundInputs)
    if (::close(descriptor) != 0)
      fail("total-bound descriptor close failed");
  return ChildInvocation{child, sockets[0], blockedPipe[0]};
}

int waitFor(pid_t child) {
  int status = 0;
  const auto deadline =
      std::chrono::steady_clock::now() + std::chrono::seconds(30);
  for (;;) {
    const pid_t waited = ::waitpid(child, &status, WNOHANG);
    if (waited == child)
      break;
    if (waited < 0 && errno != EINTR)
      fail("waitpid failed");
    if (std::chrono::steady_clock::now() >= deadline) {
      (void)::kill(child, SIGKILL);
      (void)::waitpid(child, &status, 0);
      failMessage("tool did not exit within the fixed harness deadline");
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  if (!WIFEXITED(status))
    return 70;
  return WEXITSTATUS(status);
}

void assertNoResultCapability(int socket) {
  for (;;) {
    std::array<char, 1024> bytes{};
    std::array<unsigned char, CMSG_SPACE(sizeof(int) * 2U)> control{};
    iovec vector{bytes.data(), bytes.size()};
    msghdr message{};
    message.msg_iov = &vector;
    message.msg_iovlen = 1;
    message.msg_control = control.data();
    message.msg_controllen = control.size();
    const ssize_t count =
        ::recvmsg(socket, &message, MSG_DONTWAIT | MSG_CMSG_CLOEXEC);
    if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK))
      return;
    if (count <= 0)
      return;
    for (cmsghdr *header = CMSG_FIRSTHDR(&message); header != nullptr;
         header = CMSG_NXTHDR(&message, header))
      if (header->cmsg_level == SOL_SOCKET && header->cmsg_type == SCM_RIGHTS)
        failMessage("failed tool transferred a result capability");
    const std::string_view record(bytes.data(), static_cast<size_t>(count));
    if (record.starts_with(ResultRecordPrefix))
      failMessage("failed tool emitted a result authorization record");
  }
}

int receiveResult(int socket, const char *retainedOutput) {
  std::array<char, 2048> bytes{};
  std::array<unsigned char, CMSG_SPACE(sizeof(int) * 2U)> control{};
  iovec vector{bytes.data(), bytes.size()};
  msghdr message{};
  message.msg_iov = &vector;
  message.msg_iovlen = 1;
  message.msg_control = control.data();
  message.msg_controllen = control.size();
  const ssize_t count = ::recvmsg(socket, &message, MSG_CMSG_CLOEXEC);
  if (count <= 0 || (message.msg_flags & (MSG_TRUNC | MSG_CTRUNC)) != 0)
    failMessage("missing, empty, or truncated result packet");
  int output = -1;
  size_t descriptorCount = 0;
  size_t controlCount = 0;
  for (cmsghdr *header = CMSG_FIRSTHDR(&message); header != nullptr;
       header = CMSG_NXTHDR(&message, header)) {
    ++controlCount;
    if (header->cmsg_level != SOL_SOCKET || header->cmsg_type != SCM_RIGHTS ||
        header->cmsg_len != CMSG_LEN(sizeof(int)))
      failMessage("result packet contains unexpected ancillary data");
    std::memcpy(&output, CMSG_DATA(header), sizeof(output));
    ++descriptorCount;
  }
  if (controlCount != 1 || descriptorCount != 1 || output < 0)
    failMessage("result packet must contain exactly one descriptor");
  struct stat status{};
  const int seals = ::fcntl(output, F_GET_SEALS);
  if (::fstat(output, &status) != 0 || !S_ISREG(status.st_mode) ||
      status.st_nlink != 0 || seals != static_cast<int>(InputSeals) ||
      status.st_size <= 0)
    failMessage("received output descriptor is not a sealed regular memfd");
  std::vector<uint8_t> outputBytes(static_cast<size_t>(status.st_size));
  size_t offset = 0;
  while (offset != outputBytes.size()) {
    const ssize_t read =
        ::pread(output, outputBytes.data() + offset,
                outputBytes.size() - offset, static_cast<off_t>(offset));
    if (read <= 0)
      fail("output descriptor read failed");
    offset += static_cast<size_t>(read);
  }
  const std::string outputHash = sha256(outputBytes);
  const std::string expected =
      std::string(ResultRecordPrefix) + "\tplan=" + PlanDigest +
      "\tclosure=" + ClosureDigest + "\tnonce=" + NonceDigest +
      "\tsha256=" + outputHash + "\tlength=" + std::to_string(status.st_size) +
      "\tcopy=" + ResultCopyPolicy + "\n";
  if (std::string(bytes.data(), static_cast<size_t>(count)) != expected)
    failMessage("result packet does not match its request or sealed output");
  if (::pwrite(output, "x", 1, 0) != -1 || errno != EPERM)
    failMessage("sealed output remains writable");
  // Mode is sender-owned metadata and is deliberately not part of admission.
  if (::fchmod(output, 0000) != 0)
    fail("could not perturb sender-owned output mode");
  const int owned = static_cast<int>(
      ::syscall(SYS_memfd_create, "fe2o3-host-lld-receiver-copy",
                MFD_ALLOW_SEALING | MFD_CLOEXEC));
  if (owned < 0)
    fail("receiver output memfd_create failed");
  writeAll(owned, outputBytes.data(), outputBytes.size());
  if (::fchmod(owned, OutputMode) != 0 ||
      ::fcntl(owned, F_ADD_SEALS, static_cast<int>(InputSeals)) != 0 ||
      ::fcntl(owned, F_GET_SEALS) != static_cast<int>(InputSeals))
    fail("receiver output copy finalization failed");
  struct stat ownedStatus{};
  if (::fstat(owned, &ownedStatus) != 0 || !S_ISREG(ownedStatus.st_mode) ||
      ownedStatus.st_nlink != 0 ||
      (ownedStatus.st_mode & 07777) != OutputMode ||
      ownedStatus.st_size != status.st_size)
    failMessage("receiver-owned output copy is not canonical");
  const int retained =
      ::open(retainedOutput, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0555);
  if (retained < 0)
    fail("retained output create failed");
  writeAll(retained, outputBytes.data(), outputBytes.size());
  if (::fchmod(retained, 0555) != 0 || ::fsync(retained) != 0 ||
      ::close(retained) != 0 || ::close(owned) != 0 || ::close(output) != 0)
    fail("retained output finalization failed");
  pollfd endpoint{socket, POLLIN | POLLHUP, 0};
  if (::poll(&endpoint, 1, 30000) <= 0)
    failMessage("result socket was not half-closed after one packet");
  char extra = 0;
  if (::recv(socket, &extra, sizeof(extra), 0) != 0)
    failMessage("result socket did not reach EOF after exactly one packet");
  return 0;
}

void verifyNondumpable(pid_t child) {
  const std::string fdDirectory = "/proc/" + std::to_string(child) + "/fd";
  bool denied = false;
  const auto deadline =
      std::chrono::steady_clock::now() + std::chrono::seconds(10);
  while (std::chrono::steady_clock::now() < deadline) {
    const int fd =
        ::open(fdDirectory.c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (fd < 0 && (errno == EACCES || errno == EPERM)) {
      denied = true;
      break;
    }
    if (fd >= 0)
      ::close(fd);
    if (::kill(child, 0) != 0)
      break;
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  if (!denied)
    failMessage("tool did not become nondumpable during the link");
  if (::kill(child, SIGSTOP) != 0)
    fail("could not pause nondumpable linker");
  int status = 0;
  while (::waitpid(child, &status, WUNTRACED) < 0)
    if (errno != EINTR)
      fail("paused-link wait failed");
  if (!WIFSTOPPED(status))
    failMessage("linker did not enter the deliberate paused state");
  const int procFd =
      ::open(fdDirectory.c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (procFd >= 0 || (errno != EACCES && errno != EPERM))
    failMessage("same-UID /proc/PID/fd access was not denied");
#ifdef SYS_pidfd_open
  const int pidfd = static_cast<int>(::syscall(SYS_pidfd_open, child, 0));
  if (pidfd < 0)
    fail("pidfd_open failed");
#ifdef SYS_pidfd_getfd
  const int stolen =
      static_cast<int>(::syscall(SYS_pidfd_getfd, pidfd, ResultSocketFd, 0));
  if (stolen >= 0 || (errno != EPERM && errno != EACCES))
    failMessage("pidfd_getfd bypassed nondumpable result isolation");
#endif
  if (::close(pidfd) != 0)
    fail("pidfd close failed");
#endif
  if (::kill(child, SIGCONT) != 0)
    fail("could not resume nondumpable linker");
}

bool expectsSuccess(const std::string &mode) {
  return mode == "normal" || mode == "proc-attack" ||
         mode == "replacement-race" || mode == "force-rlib" ||
         mode == "rust-link" || mode == "stdio-result-alias" ||
         mode == "stdio-input-alias" || mode == "stdio-blocked-pipe" ||
         mode == "blocking-result-socket" || mode == "hostile-signal-state" ||
         mode == "hostile-signal-rtmin" || mode == "hostile-signal-rtmax" ||
         mode == "hostile-signal-kernel-reserved-32" ||
         mode == "hostile-signal-kernel-reserved-33";
}

void enterTargetDirectory(const char *target) {
  const std::string path(target);
  const size_t separator = path.find_last_of('/');
  if (separator == std::string::npos)
    failMessage("relative dependent-library test requires an absolute oracle");
  const std::string directory = separator == 0 ? "/" : path.substr(0, separator);
  if (::chdir(directory.c_str()) != 0)
    fail("relative dependent-library test chdir failed");
}

} // namespace

int main(int argc, char **argv) {
  if (argc < 4 || argc > 6) {
    std::fprintf(stderr,
                 "usage: %s TOOL INPUT RETAINED_OUTPUT [MODE [AUXILIARY]]\n",
                 argv[0]);
    return 64;
  }
  closeAmbientDescriptors();
  const std::string mode = argc >= 5 ? argv[4] : "normal";
  std::vector<uint8_t> inputBytes = readFile(argv[2]);
  if (mode == "archive-member-flood")
    inputBytes = archiveMemberFlood(inputBytes);
  if (mode == "deplibs-malformed-name" ||
      mode == "deplibs-malformed-shstr")
    corruptDependentSectionMetadata(inputBytes, mode);
  std::vector<uint8_t> additionalRlib;
  const std::vector<uint8_t> *additionalRlibPointer = nullptr;
  if (mode == "rust-link") {
    if (argc != 6)
      failMessage("rust-link mode requires a pinned sysroot rlib path");
    additionalRlib = readFile(argv[5]);
    additionalRlibPointer = &additionalRlib;
  }
  if (mode == "proc-attack" || mode == "replacement-race" ||
      hostileSignalKillMode(mode))
    inputBytes.resize(inputBytes.size() + 128ULL * 1024 * 1024, 0);
  if (mode == "deplibs-relative-attack") {
    if (argc != 6)
      failMessage("relative dependent-library mode requires an oracle path");
    enterTargetDirectory(argv[5]);
  }
  int ambientWatch = -1;
  if (argc == 6 && mode != "rust-link") {
    ambientWatch = ::inotify_init1(IN_CLOEXEC | IN_NONBLOCK);
    if (ambientWatch < 0 ||
        ::inotify_add_watch(ambientWatch, argv[5], IN_OPEN | IN_ACCESS) < 0)
      fail("ambient target watch failed");
  }
  const ChildInvocation invocation =
      launch(argv[1], inputBytes, mode, additionalRlibPointer);
  if (mode == "proc-attack" || mode == "replacement-race" ||
      hostileSignalKillMode(mode))
    verifyNondumpable(invocation.Pid);
  if (hostileSignalKillMode(mode) &&
      ::syscall(SYS_kill, invocation.Pid, hostileSignalTarget(mode)) != 0)
    fail("could not deliver normalized hostile signal");

  if (expectsSuccess(mode)) {
    pollfd descriptor{invocation.ResultSocket, POLLIN | POLLHUP, 0};
    if (::poll(&descriptor, 1, 30000) <= 0)
      failMessage("timed out waiting for result capability");
    receiveResult(invocation.ResultSocket, argv[3]);
  }
  const int status = waitFor(invocation.Pid);
  if (ambientWatch >= 0) {
    std::array<uint8_t, 4096> events{};
    const ssize_t eventBytes =
        ::read(ambientWatch, events.data(), events.size());
    if (eventBytes > 0)
      failMessage("rejected descriptor content opened an ambient target path");
    if (eventBytes < 0 && errno != EAGAIN && errno != EWOULDBLOCK)
      fail("ambient target event read failed");
    if (::close(ambientWatch) != 0)
      fail("ambient target watch close failed");
  }
  if (expectsSuccess(mode)) {
    char extra = 0;
    const ssize_t extraBytes =
        ::recv(invocation.ResultSocket, &extra, sizeof(extra), MSG_DONTWAIT);
    if (status != 0 || extraBytes != 0)
      failMessage(
          "successful tool sent extra packets or failed after transfer");
  } else {
    assertNoResultCapability(invocation.ResultSocket);
  }
  if (::close(invocation.ResultSocket) != 0)
    fail("result socket close failed");
  if (invocation.BlockedPipeReader >= 0 &&
      ::close(invocation.BlockedPipeReader) != 0)
    fail("blocked pipe reader close failed");
  return status;
}
