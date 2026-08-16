#include <sys/inotify.h>
#include <sys/poll.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>

#include <fcntl.h>
#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/landlock.h>
#include <linux/seccomp.h>
#include <signal.h>
#include <unistd.h>

#include <algorithm>
#include <array>
#include <cerrno>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <dirent.h>
#include <limits>
#include <map>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

extern char **environ;

namespace {

constexpr uint32_t kMutationMask = IN_ATTRIB | IN_CLOSE_WRITE | IN_CREATE |
                                   IN_DELETE | IN_DELETE_SELF | IN_MODIFY |
                                   IN_MOVE_SELF | IN_MOVED_FROM | IN_MOVED_TO;
constexpr size_t kReadBufferBytes = 64 * 1024;
constexpr int kRequiredLandlockAbi = 4;
constexpr uint64_t kAllLandlockFilesystemRights =
    LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_WRITE_FILE |
    LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR |
    LANDLOCK_ACCESS_FS_REMOVE_DIR | LANDLOCK_ACCESS_FS_REMOVE_FILE |
    LANDLOCK_ACCESS_FS_MAKE_CHAR | LANDLOCK_ACCESS_FS_MAKE_DIR |
    LANDLOCK_ACCESS_FS_MAKE_REG | LANDLOCK_ACCESS_FS_MAKE_SOCK |
    LANDLOCK_ACCESS_FS_MAKE_FIFO | LANDLOCK_ACCESS_FS_MAKE_BLOCK |
    LANDLOCK_ACCESS_FS_MAKE_SYM | LANDLOCK_ACCESS_FS_REFER |
    LANDLOCK_ACCESS_FS_TRUNCATE;
constexpr uint64_t kLandlockReadDirectoryRights =
    LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;
constexpr uint64_t kLandlockReadFileRights = LANDLOCK_ACCESS_FS_READ_FILE;
constexpr uint64_t kLandlockReadWriteFileRights =
    LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_WRITE_FILE |
    LANDLOCK_ACCESS_FS_TRUNCATE;
constexpr uint64_t kLandlockWritableRootRights =
    kAllLandlockFilesystemRights & ~LANDLOCK_ACCESS_FS_MAKE_SYM;

[[noreturn]] void fail(const std::string &message) {
  throw std::runtime_error(message);
}

std::string errnoMessage(const std::string &operation) {
  return operation + ": " + std::strerror(errno);
}

class Fd {
public:
  Fd() = default;
  explicit Fd(int value) : value_(value) {}
  ~Fd() {
    if (value_ >= 0)
      ::close(value_);
  }
  Fd(const Fd &) = delete;
  Fd &operator=(const Fd &) = delete;
  Fd(Fd &&other) noexcept : value_(std::exchange(other.value_, -1)) {}
  Fd &operator=(Fd &&other) noexcept {
    if (this != &other) {
      if (value_ >= 0)
        ::close(value_);
      value_ = std::exchange(other.value_, -1);
    }
    return *this;
  }
  int get() const { return value_; }
  explicit operator bool() const { return value_ >= 0; }

private:
  int value_ = -1;
};

uint32_t rotateRight(uint32_t value, unsigned count) {
  return (value >> count) | (value << (32U - count));
}

class Sha256 {
public:
  void update(const void *data, size_t length) {
    const auto *bytes = static_cast<const uint8_t *>(data);
    totalBytes_ += length;
    while (length != 0) {
      const size_t copied = std::min(length, block_.size() - blockLength_);
      std::memcpy(block_.data() + blockLength_, bytes, copied);
      blockLength_ += copied;
      bytes += copied;
      length -= copied;
      if (blockLength_ == block_.size()) {
        transform(block_.data());
        blockLength_ = 0;
      }
    }
  }

  void update(std::string_view text) { update(text.data(), text.size()); }

  std::string finish() {
    const uint64_t bitLength = static_cast<uint64_t>(totalBytes_) * 8U;
    const uint8_t marker = 0x80;
    update(&marker, 1);
    const uint8_t zero = 0;
    while (blockLength_ != 56)
      update(&zero, 1);
    std::array<uint8_t, 8> lengthBytes{};
    for (size_t index = 0; index < lengthBytes.size(); ++index)
      lengthBytes[7 - index] =
          static_cast<uint8_t>(bitLength >> static_cast<unsigned>(index * 8));
    update(lengthBytes.data(), lengthBytes.size());

    static constexpr char kHex[] = "0123456789abcdef";
    std::string result;
    result.reserve(64);
    for (uint32_t word : state_) {
      for (int shift = 28; shift >= 0; shift -= 4)
        result.push_back(kHex[(word >> shift) & 0xfU]);
    }
    return result;
  }

private:
  void transform(const uint8_t *input) {
    static constexpr std::array<uint32_t, 64> kRound = {
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
    std::array<uint32_t, 64> words{};
    for (size_t index = 0; index < 16; ++index) {
      words[index] = static_cast<uint32_t>(input[index * 4]) << 24U |
                     static_cast<uint32_t>(input[index * 4 + 1]) << 16U |
                     static_cast<uint32_t>(input[index * 4 + 2]) << 8U |
                     static_cast<uint32_t>(input[index * 4 + 3]);
    }
    for (size_t index = 16; index < words.size(); ++index) {
      const uint32_t a = words[index - 15];
      const uint32_t b = words[index - 2];
      const uint32_t s0 = rotateRight(a, 7) ^ rotateRight(a, 18) ^ (a >> 3U);
      const uint32_t s1 = rotateRight(b, 17) ^ rotateRight(b, 19) ^ (b >> 10U);
      words[index] = words[index - 16] + s0 + words[index - 7] + s1;
    }

    uint32_t a = state_[0];
    uint32_t b = state_[1];
    uint32_t c = state_[2];
    uint32_t d = state_[3];
    uint32_t e = state_[4];
    uint32_t f = state_[5];
    uint32_t g = state_[6];
    uint32_t h = state_[7];
    for (size_t index = 0; index < words.size(); ++index) {
      const uint32_t s1 =
          rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const uint32_t choice = (e & f) ^ (~e & g);
      const uint32_t temporary1 =
          h + s1 + choice + kRound[index] + words[index];
      const uint32_t s0 =
          rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const uint32_t majority = (a & b) ^ (a & c) ^ (b & c);
      const uint32_t temporary2 = s0 + majority;
      h = g;
      g = f;
      f = e;
      e = d + temporary1;
      d = c;
      c = b;
      b = a;
      a = temporary1 + temporary2;
    }
    state_[0] += a;
    state_[1] += b;
    state_[2] += c;
    state_[3] += d;
    state_[4] += e;
    state_[5] += f;
    state_[6] += g;
    state_[7] += h;
  }

  std::array<uint32_t, 8> state_ = {0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U,
                                    0xa54ff53aU, 0x510e527fU, 0x9b05688cU,
                                    0x1f83d9abU, 0x5be0cd19U};
  std::array<uint8_t, 64> block_{};
  size_t blockLength_ = 0;
  size_t totalBytes_ = 0;
};

std::string sha256Fd(int fd, uint64_t expectedLength) {
  Sha256 digest;
  std::array<uint8_t, kReadBufferBytes> buffer{};
  uint64_t offset = 0;
  while (offset < expectedLength) {
    const size_t requested = static_cast<size_t>(
        std::min<uint64_t>(buffer.size(), expectedLength - offset));
    const ssize_t count =
        ::pread(fd, buffer.data(), requested, static_cast<off_t>(offset));
    if (count < 0)
      fail(errnoMessage("pread"));
    if (count == 0)
      fail("file shortened while hashing");
    digest.update(buffer.data(), static_cast<size_t>(count));
    offset += static_cast<uint64_t>(count);
  }
  return digest.finish();
}

std::string sha256Text(std::string_view text) {
  Sha256 digest;
  digest.update(text);
  return digest.finish();
}

std::string sha256FileAt(int directoryFd, const std::string &name,
                         const struct stat &before) {
  Fd fd(::openat(directoryFd, name.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW));
  if (!fd)
    fail(errnoMessage("openat regular file"));
  struct stat opened{};
  if (::fstat(fd.get(), &opened) != 0)
    fail(errnoMessage("fstat regular file"));
  if (!S_ISREG(opened.st_mode) || opened.st_dev != before.st_dev ||
      opened.st_ino != before.st_ino || opened.st_size != before.st_size)
    fail("regular file changed while opening");
  const std::string digest =
      sha256Fd(fd.get(), static_cast<uint64_t>(opened.st_size));
  struct stat after{};
  if (::fstat(fd.get(), &after) != 0)
    fail(errnoMessage("fstat regular file after hash"));
  if (after.st_dev != opened.st_dev || after.st_ino != opened.st_ino ||
      after.st_mode != opened.st_mode || after.st_size != opened.st_size ||
      after.st_mtim.tv_sec != opened.st_mtim.tv_sec ||
      after.st_mtim.tv_nsec != opened.st_mtim.tv_nsec ||
      after.st_ctim.tv_sec != opened.st_ctim.tv_sec ||
      after.st_ctim.tv_nsec != opened.st_ctim.tv_nsec)
    fail("regular file changed while hashing");
  return digest;
}

bool safeComponent(std::string_view value) {
  if (value.empty() || value == "." || value == "..")
    return false;
  for (char character : value) {
    const auto byte = static_cast<unsigned char>(character);
    if (byte == '/' || byte < 0x20 || byte == 0x7f)
      return false;
  }
  return true;
}

bool safeLabel(std::string_view value) {
  if (value.empty() || value.size() > 80)
    return false;
  for (char character : value) {
    const auto byte = static_cast<unsigned char>(character);
    if (!(byte >= 'a' && byte <= 'z') && !(byte >= 'A' && byte <= 'Z') &&
        !(byte >= '0' && byte <= '9') && byte != '.' && byte != '_' &&
        byte != '-')
      return false;
  }
  return true;
}

bool safeRuntimeName(std::string_view value) {
  if (value.empty() || value.size() > 80)
    return false;
  for (char character : value) {
    const auto byte = static_cast<unsigned char>(character);
    if (!(byte >= 'a' && byte <= 'z') && !(byte >= 'A' && byte <= 'Z') &&
        !(byte >= '0' && byte <= '9') && byte != '.' && byte != '_' &&
        byte != '-' && byte != '+')
      return false;
  }
  return true;
}

bool safePathText(std::string_view value) {
  return std::all_of(value.begin(), value.end(), [](char character) {
    const auto byte = static_cast<unsigned char>(character);
    return byte >= 0x20 && byte != 0x7f && byte != '\t';
  });
}

uint64_t parseUnsigned(const char *text, const char *label) {
  if (*text == '\0')
    fail(std::string(label) + " is empty");
  uint64_t value = 0;
  for (char character : std::string_view(text)) {
    const auto byte = static_cast<unsigned char>(character);
    if (byte < '0' || byte > '9')
      fail(std::string(label) + " is not decimal");
    const uint64_t digit = static_cast<uint64_t>(byte - '0');
    if (value > (std::numeric_limits<uint64_t>::max() - digit) / 10U)
      fail(std::string(label) + " overflows");
    value = value * 10U + digit;
  }
  return value;
}

uint64_t parseMode(const char *text) {
  if (*text == '\0')
    fail("file mode is empty");
  uint64_t value = 0;
  for (char character : std::string_view(text)) {
    const auto byte = static_cast<unsigned char>(character);
    if (byte < '0' || byte > '7')
      fail("file mode is not octal");
    value = value * 8U + static_cast<uint64_t>(byte - '0');
  }
  if (value > 07777)
    fail("file mode is out of range");
  return value;
}

std::string modeText(mode_t mode) {
  std::array<char, 8> buffer{};
  const int length = std::snprintf(buffer.data(), buffer.size(), "%o",
                                   static_cast<unsigned>(mode & 07777));
  if (length <= 0 || static_cast<size_t>(length) >= buffer.size())
    fail("could not format file mode");
  return std::string(buffer.data(), static_cast<size_t>(length));
}

bool canonicalDigest(std::string_view digest) {
  if (digest.size() != 64)
    return false;
  return std::all_of(digest.begin(), digest.end(), [](unsigned char byte) {
    return (byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f');
  });
}

std::pair<std::string, std::string> splitParent(const std::string &path) {
  if (path.empty() || path.front() != '/' || path.back() == '/' ||
      !safePathText(path))
    fail("input path is not canonical absolute syntax: " + path);
  const size_t separator = path.rfind('/');
  const std::string parent = separator == 0 ? "/" : path.substr(0, separator);
  const std::string base = path.substr(separator + 1);
  if (!safeComponent(base))
    fail("input basename is not canonical: " + path);
  char *resolved = ::realpath(path.c_str(), nullptr);
  if (resolved == nullptr)
    fail(errnoMessage("realpath input"));
  const std::string canonical(resolved);
  std::free(resolved);
  if (canonical != path)
    fail("input path is not canonical or is a symlink: " + path);
  return {parent, base};
}

std::pair<std::string, std::string>
splitNonexistentOutput(const std::string &path) {
  if (path.empty() || path.front() != '/' || path.back() == '/' ||
      !safePathText(path))
    fail("output path is not canonical absolute syntax: " + path);
  const size_t separator = path.rfind('/');
  const std::string parent = separator == 0 ? "/" : path.substr(0, separator);
  const std::string base = path.substr(separator + 1);
  if (!safeComponent(base))
    fail("output basename is not canonical: " + path);
  if (::faccessat(AT_FDCWD, path.c_str(), F_OK, AT_SYMLINK_NOFOLLOW) == 0 ||
      errno != ENOENT)
    fail("output path must not exist: " + path);
  char *resolved = ::realpath(parent.c_str(), nullptr);
  if (resolved == nullptr)
    fail(errnoMessage("realpath output parent"));
  const std::string canonicalParent(resolved);
  std::free(resolved);
  if (canonicalParent != parent)
    fail("output parent is not canonical: " + parent);
  return {parent, base};
}

std::string statIdentity(const struct stat &status) {
  return std::to_string(static_cast<uint64_t>(status.st_dev)) + ":" +
         std::to_string(static_cast<uint64_t>(status.st_ino));
}

struct Identity {
  uint64_t device = 0;
  uint64_t inode = 0;
  uint64_t mode = 0;
  uint64_t links = 0;
  uint64_t size = 0;
  int64_t mtimeSeconds = 0;
  int64_t mtimeNanos = 0;
  int64_t ctimeSeconds = 0;
  int64_t ctimeNanos = 0;
  std::string content;

  bool operator==(const Identity &) const = default;
};

Identity identityOf(const struct stat &status, std::string content = {}) {
  return Identity{static_cast<uint64_t>(status.st_dev),
                  static_cast<uint64_t>(status.st_ino),
                  static_cast<uint64_t>(status.st_mode),
                  static_cast<uint64_t>(status.st_nlink),
                  static_cast<uint64_t>(status.st_size),
                  static_cast<int64_t>(status.st_mtim.tv_sec),
                  static_cast<int64_t>(status.st_mtim.tv_nsec),
                  static_cast<int64_t>(status.st_ctim.tv_sec),
                  static_cast<int64_t>(status.st_ctim.tv_nsec),
                  std::move(content)};
}

struct DirectoryNode {
  std::string relative;
  Fd fd;
  Identity identity;
};

struct Root {
  std::string label;
  std::string path;
  std::string parentPath;
  std::string basename;
  std::string expectedDigest;
  uint64_t expectedManifestLength = 0;
  Fd parentFd;
  std::vector<DirectoryNode> directories;
  std::map<std::string, Identity> baselineEntries;
  std::string baselineDigest;
  uint64_t baselineManifestLength = 0;
  bool landlockReadable = true;
};

struct TrackedFile {
  std::string label;
  std::string path;
  std::string parentPath;
  std::string basename;
  std::string expectedDigest;
  uint64_t expectedLength = 0;
  uint64_t expectedMode = 0;
  Fd parentFd;
  Fd fd;
  Identity baselineIdentity;
};

struct WatchedDirectory {
  std::string label;
  std::string path;
  bool contentsStable = true;
  std::string parentPath;
  std::string basename;
  Fd parentFd;
  Fd fd;
  Identity baselineIdentity;
};

struct RuntimeInput {
  std::string name;
  std::string path;
};

struct Command {
  std::vector<std::string> arguments;
};

std::string normalizeAbsolutePath(std::string_view value);

enum class LandlockPathAccess {
  ReadOnly,
  ReadWriteFile,
  WritableRoot,
};

struct LandlockPath {
  std::string label;
  std::string path;
  LandlockPathAccess access = LandlockPathAccess::ReadOnly;
  std::string parentPath;
  std::string basename;
  Fd parentFd;
  Fd fd;
  Identity baselineIdentity;
  bool directory = false;
};

struct Options {
  std::string statusPath;
  std::string resolver;
  std::string tmpRedirectPath;
  std::string privateTmpPath;
  uint64_t maxEntries = 200000;
  uint64_t maxDepth = 128;
  uint64_t maxManifestBytes = 64ULL * 1024ULL * 1024ULL;
  std::vector<Root> roots;
  std::vector<WatchedDirectory> directories;
  std::vector<TrackedFile> files;
  std::vector<LandlockPath> landlockPaths;
  std::vector<RuntimeInput> runtimeInputs;
  std::vector<Command> commands;
};

Options parseOptions(int argc, char **argv) {
  Options options;
  for (int index = 1; index < argc;) {
    const std::string_view option(argv[index++]);
    auto require = [&](const char *label) -> const char * {
      if (index >= argc)
        fail(std::string("missing ") + label);
      return argv[index++];
    };
    if (option == "--status") {
      options.statusPath = require("status path");
    } else if (option == "--resolver") {
      options.resolver = require("resolver path");
    } else if (option == "--max-entries") {
      options.maxEntries = parseUnsigned(require("entry bound"), "entry bound");
    } else if (option == "--max-depth") {
      options.maxDepth = parseUnsigned(require("depth bound"), "depth bound");
    } else if (option == "--max-manifest-bytes") {
      options.maxManifestBytes =
          parseUnsigned(require("manifest bound"), "manifest bound");
    } else if (option == "--root" || option == "--absence-root") {
      Root root;
      root.landlockReadable = option == "--root";
      root.label = require("root label");
      root.path = require("root path");
      root.expectedDigest = require("root digest");
      root.expectedManifestLength = parseUnsigned(
          require("root manifest length"), "root manifest length");
      if (!safeLabel(root.label) || !canonicalDigest(root.expectedDigest))
        fail("root label or digest is not canonical");
      options.roots.push_back(std::move(root));
    } else if (option == "--file") {
      TrackedFile file;
      file.label = require("file label");
      file.path = require("file path");
      file.expectedDigest = require("file digest");
      file.expectedLength =
          parseUnsigned(require("file length"), "file length");
      file.expectedMode = parseMode(require("file mode"));
      if (!safeLabel(file.label) || !canonicalDigest(file.expectedDigest) ||
          file.expectedMode > 07777)
        fail("file label, digest, or mode is not canonical");
      options.files.push_back(std::move(file));
    } else if (option == "--directory" || option == "--ancestor") {
      WatchedDirectory directory;
      directory.label = require("directory label");
      directory.path = require("directory path");
      directory.contentsStable = option == "--directory";
      if (!safeLabel(directory.label))
        fail("directory label is not canonical");
      options.directories.push_back(std::move(directory));
    } else if (option == "--landlock-read-only") {
      LandlockPath path;
      path.label = require("Landlock path label");
      path.path = require("Landlock read-only path");
      path.access = LandlockPathAccess::ReadOnly;
      if (!safeLabel(path.label))
        fail("Landlock path label is not canonical");
      options.landlockPaths.push_back(std::move(path));
    } else if (option == "--tmp-redirect") {
      options.tmpRedirectPath = require("temporary-path redirect library");
      options.privateTmpPath = require("private temporary directory");
    } else if (option == "--landlock-read-write-file") {
      LandlockPath path;
      path.label = require("Landlock path label");
      path.path = require("Landlock read-write file");
      path.access = LandlockPathAccess::ReadWriteFile;
      if (!safeLabel(path.label))
        fail("Landlock path label is not canonical");
      options.landlockPaths.push_back(std::move(path));
    } else if (option == "--landlock-writable-root") {
      LandlockPath path;
      path.label = require("Landlock path label");
      path.path = require("Landlock writable root");
      path.access = LandlockPathAccess::WritableRoot;
      if (!safeLabel(path.label))
        fail("Landlock path label is not canonical");
      options.landlockPaths.push_back(std::move(path));
    } else if (option == "--runtime") {
      RuntimeInput runtime{require("runtime name"), require("runtime path")};
      if (!safeRuntimeName(runtime.name))
        fail("runtime name is not canonical");
      options.runtimeInputs.push_back(std::move(runtime));
    } else if (option == "--command") {
      const uint64_t count =
          parseUnsigned(require("command argument count"), "command count");
      if (count == 0 || count > 256 ||
          static_cast<uint64_t>(argc - index) < count)
        fail("command argument count is invalid");
      Command command;
      for (uint64_t item = 0; item < count; ++item)
        command.arguments.emplace_back(argv[index++]);
      options.commands.push_back(std::move(command));
    } else {
      fail("unknown option: " + std::string(option));
    }
  }
  if (options.statusPath.empty() || options.roots.empty() ||
      options.files.empty() || options.commands.empty())
    fail("status, roots, files, and commands are required");
  if (options.maxEntries == 0 || options.maxDepth == 0 ||
      options.maxManifestBytes < 1024 || options.roots.size() > 32 ||
      options.directories.size() > 32 || options.files.size() > 4096 ||
      options.landlockPaths.size() > 64 || options.commands.size() > 16 ||
      options.runtimeInputs.size() > 64)
    fail("guard bounds are invalid");
  if (options.runtimeInputs.empty() != options.resolver.empty())
    fail("resolver and runtime inputs must be supplied together");
  if (std::none_of(options.landlockPaths.begin(), options.landlockPaths.end(),
                   [](const LandlockPath &path) {
                     return path.access == LandlockPathAccess::WritableRoot;
                   }))
    fail("at least one Landlock writable root is required");
  if (options.tmpRedirectPath.empty() || options.privateTmpPath.empty() ||
      options.tmpRedirectPath.front() != '/' ||
      options.privateTmpPath.front() != '/' ||
      normalizeAbsolutePath(options.tmpRedirectPath) !=
          options.tmpRedirectPath ||
      normalizeAbsolutePath(options.privateTmpPath) != options.privateTmpPath)
    fail("temporary-path redirect configuration is not canonical");
  if (std::none_of(options.files.begin(), options.files.end(),
                   [&](const TrackedFile &file) {
                     return file.path == options.tmpRedirectPath;
                   }))
    fail("temporary-path redirect library is not a tracked file");
  if (std::none_of(options.landlockPaths.begin(), options.landlockPaths.end(),
                   [&](const LandlockPath &path) {
                     return path.access == LandlockPathAccess::WritableRoot &&
                            (options.privateTmpPath == path.path ||
                             options.privateTmpPath.starts_with(path.path +
                                                                "/"));
                   }))
    fail("private temporary directory is outside writable Landlock roots");
  std::vector<std::string> labels;
  for (const Root &root : options.roots) {
    if (std::find(labels.begin(), labels.end(), root.label) != labels.end())
      fail("duplicate guard label: " + root.label);
    labels.push_back(root.label);
  }
  for (const WatchedDirectory &directory : options.directories) {
    if (std::find(labels.begin(), labels.end(), directory.label) !=
        labels.end())
      fail("duplicate guard label: " + directory.label);
    labels.push_back(directory.label);
  }
  for (const TrackedFile &file : options.files) {
    if (std::find(labels.begin(), labels.end(), file.label) != labels.end())
      fail("duplicate guard label: " + file.label);
    labels.push_back(file.label);
  }
  for (const LandlockPath &path : options.landlockPaths) {
    if (std::find(labels.begin(), labels.end(), path.label) != labels.end())
      fail("duplicate guard label: " + path.label);
    labels.push_back(path.label);
    if (path.path.empty() || path.path.front() != '/' ||
        normalizeAbsolutePath(path.path) != path.path)
      fail("Landlock path is not a canonical absolute path: " + path.label);
  }
  for (size_t left = 0; left < options.roots.size(); ++left) {
    for (size_t right = left + 1; right < options.roots.size(); ++right) {
      const std::string leftPrefix = options.roots[left].path + "/";
      const std::string rightPrefix = options.roots[right].path + "/";
      if (options.roots[left].path == options.roots[right].path ||
          options.roots[left].path.starts_with(rightPrefix) ||
          options.roots[right].path.starts_with(leftPrefix)) {
        if (!options.roots[left].landlockReadable ||
            !options.roots[right].landlockReadable)
          continue;
        fail("guard roots must not overlap");
      }
    }
  }
  for (const Command &command : options.commands) {
    if (command.arguments.front().empty() ||
        command.arguments.front().front() != '/' ||
        std::find_if(options.files.begin(), options.files.end(),
                     [&](const TrackedFile &file) {
                       return file.path == command.arguments.front();
                     }) == options.files.end())
      fail("each command executable must be an exact tracked file");
    for (const std::string &argument : command.arguments) {
      if (argument.find('\n') != std::string::npos ||
          argument.find('\r') != std::string::npos ||
          argument.find('\0') != std::string::npos)
        fail("command contains a noncanonical argument");
    }
  }
  if (!options.resolver.empty() &&
      std::find_if(options.files.begin(), options.files.end(),
                   [&](const TrackedFile &file) {
                     return file.path == options.resolver;
                   }) == options.files.end())
    fail("runtime resolver must be an exact tracked file");
  for (const RuntimeInput &runtime : options.runtimeInputs) {
    if (std::find_if(options.files.begin(), options.files.end(),
                     [&](const TrackedFile &file) {
                       return file.path == runtime.path;
                     }) == options.files.end())
      fail("runtime input must be an exact tracked file: " + runtime.name);
  }
  return options;
}

struct WatchRule {
  bool anyName = false;
  std::string exactName;
  std::string label;
};

class Watches {
public:
  Watches() {
    const int fd = ::inotify_init1(IN_CLOEXEC | IN_NONBLOCK);
    if (fd < 0)
      fail(errnoMessage("inotify_init1"));
    fd_ = Fd(fd);
  }

  void addAny(int fd, const std::string &label) {
    add(fd, WatchRule{true, {}, label});
  }

  void addExact(int fd, const std::string &name, const std::string &label) {
    add(fd, WatchRule{false, name, label});
  }

  void requireQuiet(const std::string &phase) {
    alignas(struct inotify_event) std::array<char, 64 * 1024> buffer{};
    while (true) {
      const ssize_t count = ::read(fd_.get(), buffer.data(), buffer.size());
      if (count < 0) {
        if (errno == EAGAIN)
          return;
        if (errno == EINTR)
          continue;
        fail(errnoMessage("read inotify queue"));
      }
      if (count == 0)
        fail("inotify queue closed");
      size_t offset = 0;
      while (offset < static_cast<size_t>(count)) {
        if (static_cast<size_t>(count) - offset < sizeof(struct inotify_event))
          fail("truncated inotify event");
        const auto *event = reinterpret_cast<const struct inotify_event *>(
            buffer.data() + offset);
        const size_t eventSize = sizeof(struct inotify_event) + event->len;
        if (eventSize > static_cast<size_t>(count) - offset)
          fail("truncated inotify event name");
        if ((event->mask & IN_Q_OVERFLOW) != 0)
          fail("inotify queue overflow during " + phase);
        if ((event->mask & IN_IGNORED) != 0)
          fail("inotify watch was removed during " + phase);
        if ((event->mask & kMutationMask) != 0) {
          const std::string_view name = event->len == 0
                                            ? std::string_view{}
                                            : std::string_view(event->name);
          const auto found = rules_.find(event->wd);
          if (found == rules_.end())
            fail("event arrived for an unknown inotify watch");
          for (const WatchRule &rule : found->second) {
            if (rule.anyName || name == rule.exactName)
              fail("mutation observed for " + rule.label + " during " + phase);
          }
        }
        offset += eventSize;
      }
    }
  }

  void requireQuietWindow(const std::string &phase,
                          std::chrono::milliseconds duration) {
    requireQuiet(phase);
    const auto deadline = std::chrono::steady_clock::now() + duration;
    while (true) {
      const auto now = std::chrono::steady_clock::now();
      if (now >= deadline)
        return;
      const auto remaining =
          std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now);
      struct pollfd descriptor{fd_.get(), POLLIN, 0};
      const int ready =
          ::poll(&descriptor, 1, static_cast<int>(remaining.count()));
      if (ready < 0) {
        if (errno == EINTR)
          continue;
        fail(errnoMessage("poll inotify queue"));
      }
      if (ready == 0)
        return;
      if ((descriptor.revents & (POLLERR | POLLHUP | POLLNVAL)) != 0)
        fail("inotify descriptor failed during " + phase);
      requireQuiet(phase);
    }
  }

private:
  void add(int fd, WatchRule rule) {
    const std::string path = "/proc/self/fd/" + std::to_string(fd);
    const int watch =
        ::inotify_add_watch(fd_.get(), path.c_str(), kMutationMask);
    if (watch < 0)
      fail(errnoMessage("inotify_add_watch"));
    rules_[watch].push_back(std::move(rule));
  }

  Fd fd_;
  std::unordered_map<int, std::vector<WatchRule>> rules_;
};

void raiseDescriptorLimit() {
  struct rlimit limit{};
  if (::getrlimit(RLIMIT_NOFILE, &limit) != 0)
    fail(errnoMessage("getrlimit"));
  const rlim_t requested = std::min<rlim_t>(limit.rlim_max, 65536);
  if (limit.rlim_cur < requested) {
    limit.rlim_cur = requested;
    if (::setrlimit(RLIMIT_NOFILE, &limit) != 0)
      fail(errnoMessage("setrlimit"));
  }
  if (limit.rlim_cur < 4096)
    fail("RLIMIT_NOFILE is too small for recursive descriptor retention");
}

std::vector<std::string> directoryNames(int fd) {
  const int duplicate = ::fcntl(fd, F_DUPFD_CLOEXEC, 3);
  if (duplicate < 0)
    fail(errnoMessage("duplicate directory descriptor"));
  if (::lseek(duplicate, 0, SEEK_SET) < 0) {
    ::close(duplicate);
    fail(errnoMessage("rewind directory descriptor"));
  }
  DIR *directory = ::fdopendir(duplicate);
  if (directory == nullptr) {
    ::close(duplicate);
    fail(errnoMessage("fdopendir"));
  }
  std::vector<std::string> names;
  errno = 0;
  while (struct dirent *entry = ::readdir(directory)) {
    const std::string name(entry->d_name);
    if (name == "." || name == "..")
      continue;
    if (!safeComponent(name)) {
      ::closedir(directory);
      fail("directory contains a noncanonical name");
    }
    names.push_back(name);
    errno = 0;
  }
  if (errno != 0) {
    const std::string message = errnoMessage("readdir");
    ::closedir(directory);
    fail(message);
  }
  if (::closedir(directory) != 0)
    fail(errnoMessage("closedir"));
  std::sort(names.begin(), names.end());
  return names;
}

void setupDirectory(Root &root, Watches &watches, Fd fd,
                    const std::string &relative, uint64_t depth,
                    const Options &options, uint64_t &entries) {
  if (depth > options.maxDepth)
    fail("recursive guard depth bound exceeded for " + root.label);
  struct stat directoryStatus{};
  if (::fstat(fd.get(), &directoryStatus) != 0 ||
      !S_ISDIR(directoryStatus.st_mode))
    fail("guarded directory is unavailable: " + root.label + "/" + relative);
  watches.addAny(fd.get(), root.label + "/" + relative);
  root.directories.push_back(
      DirectoryNode{relative, std::move(fd), identityOf(directoryStatus)});
  const int directoryFd = root.directories.back().fd.get();
  for (const std::string &name : directoryNames(directoryFd)) {
    if (++entries > options.maxEntries)
      fail("recursive guard entry bound exceeded for " + root.label);
    struct stat status{};
    if (::fstatat(directoryFd, name.c_str(), &status, AT_SYMLINK_NOFOLLOW) != 0)
      fail(errnoMessage("fstatat guarded entry"));
    if (S_ISDIR(status.st_mode)) {
      Fd child(::openat(directoryFd, name.c_str(),
                        O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
      if (!child)
        fail(errnoMessage("openat guarded directory"));
      const std::string childRelative =
          relative == "." ? name : relative + "/" + name;
      setupDirectory(root, watches, std::move(child), childRelative, depth + 1,
                     options, entries);
    } else if (!S_ISREG(status.st_mode) && !S_ISLNK(status.st_mode)) {
      fail("special file in guarded root: " + root.label + "/" + name);
    }
  }
}

void setupRoot(Root &root, Watches &watches, const Options &options) {
  auto [parentPath, basename] = splitParent(root.path);
  root.parentPath = parentPath;
  root.basename = basename;
  root.parentFd = Fd(::open(parentPath.c_str(),
                            O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!root.parentFd)
    fail(errnoMessage("open root parent"));
  watches.addExact(root.parentFd.get(), basename, root.label + " root entry");
  Fd rootFd(::openat(root.parentFd.get(), basename.c_str(),
                     O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!rootFd)
    fail(errnoMessage("open guarded root"));
  uint64_t entries = 0;
  setupDirectory(root, watches, std::move(rootFd), ".", 0, options, entries);
}

struct ManifestBuilder {
  Sha256 digest;
  uint64_t length = 0;
  uint64_t maximum = 0;

  void append(std::string_view value) {
    if (value.size() > maximum - length)
      fail("root manifest byte bound exceeded");
    digest.update(value);
    length += value.size();
  }
};

std::string normalizeAbsolutePath(std::string_view value) {
  if (value.empty() || value.front() != '/' || !safePathText(value))
    return {};
  std::vector<std::string> components;
  size_t offset = 1;
  while (offset <= value.size()) {
    const size_t next = value.find('/', offset);
    const std::string_view component = value.substr(
        offset,
        next == std::string_view::npos ? value.size() - offset : next - offset);
    if (component.empty() && next != std::string_view::npos)
      return {};
    if (component == "..") {
      if (components.empty())
        return {};
      components.pop_back();
    } else if (component != ".") {
      if (!safeComponent(component))
        return {};
      components.emplace_back(component);
    }
    if (next == std::string_view::npos)
      break;
    offset = next + 1;
  }
  std::string result;
  for (const std::string &component : components)
    result += "/" + component;
  return result.empty() ? "/" : result;
}

bool symlinkStaysWithinGuardedRoots(const Options &options, const Root &root,
                                    std::string_view child,
                                    std::string_view target) {
  if (target.empty() || !safePathText(target))
    return false;
  const size_t separator = child.rfind('/');
  const std::string parent =
      separator == std::string_view::npos || child.substr(0, separator) == "."
          ? root.path
          : root.path + "/" + std::string(child.substr(0, separator));
  const std::string candidate = normalizeAbsolutePath(
      target.front() == '/' ? target : parent + "/" + std::string(target));
  if (candidate.empty())
    return false;
  return std::any_of(options.roots.begin(), options.roots.end(),
                     [&](const Root &guarded) {
                       return candidate == guarded.path ||
                              candidate.starts_with(guarded.path + "/");
                     });
}

std::string readSymlinkAt(int directoryFd, const std::string &name,
                          const std::string &relative, const Root &root,
                          const Options &options, const struct stat &status) {
  if (status.st_size < 0 || status.st_size > 4096)
    fail("symlink target length is invalid");
  std::array<char, 4097> target{};
  const ssize_t count =
      ::readlinkat(directoryFd, name.c_str(), target.data(), target.size() - 1);
  if (count < 0)
    fail(errnoMessage("readlinkat"));
  const std::string result(target.data(), static_cast<size_t>(count));
  if (!root.landlockReadable)
    return result;
  if (!symlinkStaysWithinGuardedRoots(options, root, relative, result))
    fail("guarded symlink escapes retained roots: " + root.label + "/" +
         relative + " -> " + result);
  return result;
}

DirectoryNode &findDirectory(Root &root, const std::string &relative) {
  const auto found = std::find_if(
      root.directories.begin(), root.directories.end(),
      [&](const DirectoryNode &node) { return node.relative == relative; });
  if (found == root.directories.end())
    fail("guarded directory set changed: " + root.label + "/" + relative);
  return *found;
}

void snapshotDirectory(Root &root, const std::string &relative,
                       ManifestBuilder &manifest,
                       std::map<std::string, Identity> &entries,
                       const Options &options, uint64_t depth,
                       uint64_t &entryCount) {
  if (depth > options.maxDepth)
    fail("snapshot depth bound exceeded for " + root.label);
  DirectoryNode &node = findDirectory(root, relative);
  struct stat directoryStatus{};
  if (::fstat(node.fd.get(), &directoryStatus) != 0 ||
      !S_ISDIR(directoryStatus.st_mode))
    fail("guarded directory descriptor changed");
  const std::string path = relative;
  entries.emplace(path, identityOf(directoryStatus));
  manifest.append("D\t" + path + "\t" + modeText(directoryStatus.st_mode) +
                  "\n");

  for (const std::string &name : directoryNames(node.fd.get())) {
    if (++entryCount > options.maxEntries)
      fail("snapshot entry bound exceeded for " + root.label);
    struct stat status{};
    if (::fstatat(node.fd.get(), name.c_str(), &status, AT_SYMLINK_NOFOLLOW) !=
        0)
      fail(errnoMessage("fstatat snapshot entry"));
    const std::string child = relative == "." ? name : relative + "/" + name;
    if (S_ISDIR(status.st_mode)) {
      DirectoryNode &childNode = findDirectory(root, child);
      struct stat opened{};
      if (::fstat(childNode.fd.get(), &opened) != 0 ||
          opened.st_dev != status.st_dev || opened.st_ino != status.st_ino)
        fail("guarded directory pathname was replaced");
      snapshotDirectory(root, child, manifest, entries, options, depth + 1,
                        entryCount);
    } else if (S_ISREG(status.st_mode)) {
      const std::string content = sha256FileAt(node.fd.get(), name, status);
      entries.emplace(child, identityOf(status, content));
      manifest.append("F\t" + child + "\t" + modeText(status.st_mode) + "\t" +
                      std::to_string(static_cast<uint64_t>(status.st_size)) +
                      "\t" + content + "\n");
    } else if (S_ISLNK(status.st_mode)) {
      const std::string target =
          readSymlinkAt(node.fd.get(), name, child, root, options, status);
      entries.emplace(child, identityOf(status, target));
      manifest.append("L\t" + child + "\t" + target + "\n");
    } else {
      fail("special file in guarded snapshot: " + root.label + "/" + child);
    }
  }
}

void snapshotRoot(Root &root, const Options &options, bool baseline) {
  ManifestBuilder manifest{Sha256{}, 0, options.maxManifestBytes};
  manifest.append("fe2o3-static-host-lld-build-guard-root-v1\n");
  manifest.append("label=" + root.label + "\n");
  std::map<std::string, Identity> entries;
  uint64_t entryCount = 0;
  snapshotDirectory(root, ".", manifest, entries, options, 0, entryCount);
  const uint64_t manifestLength = manifest.length;
  const std::string digest = manifest.digest.finish();
  if (baseline) {
    if (digest != root.expectedDigest ||
        manifestLength != root.expectedManifestLength)
      fail("reviewed root manifest pin mismatch for " + root.label +
           ": observed " + digest + "/" + std::to_string(manifestLength));
    root.baselineEntries = std::move(entries);
    root.baselineDigest = digest;
    root.baselineManifestLength = manifestLength;
  } else if (digest != root.baselineDigest ||
             manifestLength != root.baselineManifestLength ||
             entries != root.baselineEntries) {
    fail("guarded root changed during build: " + root.label);
  }
}

void setupWatchedDirectory(WatchedDirectory &directory, Watches &watches) {
  if (directory.path == "/") {
    directory.parentPath = "/";
    directory.basename = ".";
    directory.parentFd =
        Fd(::open("/", O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
    directory.fd =
        Fd(::open("/", O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
    if (!directory.parentFd || !directory.fd)
      fail(errnoMessage("open watched filesystem root"));
    if (directory.contentsStable)
      watches.addAny(directory.fd.get(), directory.label + " directory");
    return;
  }
  auto [parentPath, basename] = splitParent(directory.path);
  directory.parentPath = parentPath;
  directory.basename = basename;
  directory.parentFd = Fd(::open(
      parentPath.c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!directory.parentFd)
    fail(errnoMessage("open watched directory parent"));
  watches.addExact(directory.parentFd.get(), basename,
                   directory.label + " parent entry");
  directory.fd = Fd(::openat(directory.parentFd.get(), basename.c_str(),
                             O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!directory.fd)
    fail(errnoMessage("open watched directory"));
  if (directory.contentsStable)
    watches.addAny(directory.fd.get(), directory.label + " directory");
}

void verifyWatchedDirectory(WatchedDirectory &directory, bool baseline) {
  struct stat retained{};
  struct stat named{};
  if (::fstat(directory.fd.get(), &retained) != 0 || !S_ISDIR(retained.st_mode))
    fail("watched directory descriptor changed: " + directory.label);
  const int namedResult =
      directory.path == "/"
          ? ::stat(directory.path.c_str(), &named)
          : ::fstatat(directory.parentFd.get(), directory.basename.c_str(),
                      &named, AT_SYMLINK_NOFOLLOW);
  if (namedResult != 0 || !S_ISDIR(named.st_mode) ||
      retained.st_dev != named.st_dev || retained.st_ino != named.st_ino)
    fail("watched directory pathname was replaced: " + directory.label);
  const Identity identity = identityOf(retained);
  if (baseline)
    directory.baselineIdentity = identity;
  else if (directory.contentsStable &&
           !(identity == directory.baselineIdentity))
    fail("watched directory identity changed: " + directory.label);
}

void setupTrackedFile(TrackedFile &file, Watches &watches) {
  auto [parentPath, basename] = splitParent(file.path);
  file.parentPath = parentPath;
  file.basename = basename;
  file.parentFd = Fd(::open(parentPath.c_str(),
                            O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!file.parentFd)
    fail(errnoMessage("open tracked file parent"));
  watches.addExact(file.parentFd.get(), basename, file.label + " parent entry");
  file.fd = Fd(::openat(file.parentFd.get(), basename.c_str(),
                        O_RDONLY | O_CLOEXEC | O_NOFOLLOW));
  if (!file.fd)
    fail(errnoMessage("open tracked file"));
  watches.addAny(file.fd.get(), file.label + " inode");
}

Identity verifyTrackedFile(TrackedFile &file, bool baseline) {
  struct stat status{};
  if (::fstat(file.fd.get(), &status) != 0 || !S_ISREG(status.st_mode))
    fail("tracked input is not a regular file: " + file.label);
  const uint64_t length = static_cast<uint64_t>(status.st_size);
  const uint64_t mode = static_cast<uint64_t>(status.st_mode & 07777);
  const std::string digest = sha256Fd(file.fd.get(), length);
  struct stat after{};
  if (::fstat(file.fd.get(), &after) != 0 || after.st_dev != status.st_dev ||
      after.st_ino != status.st_ino || after.st_mode != status.st_mode ||
      after.st_size != status.st_size ||
      after.st_mtim.tv_sec != status.st_mtim.tv_sec ||
      after.st_mtim.tv_nsec != status.st_mtim.tv_nsec ||
      after.st_ctim.tv_sec != status.st_ctim.tv_sec ||
      after.st_ctim.tv_nsec != status.st_ctim.tv_nsec)
    fail("tracked input changed while hashing: " + file.label);
  struct stat named{};
  if (::fstatat(file.parentFd.get(), file.basename.c_str(), &named,
                AT_SYMLINK_NOFOLLOW) != 0 ||
      named.st_dev != status.st_dev || named.st_ino != status.st_ino)
    fail("tracked input pathname was replaced: " + file.label);
  if (length != file.expectedLength || mode != file.expectedMode ||
      digest != file.expectedDigest)
    fail("tracked input differs from pin: " + file.label);
  const Identity identity = identityOf(after, digest);
  if (baseline)
    file.baselineIdentity = identity;
  else if (!(identity == file.baselineIdentity))
    fail("tracked input identity changed during build: " + file.label);
  return identity;
}

void setupLandlockPath(LandlockPath &path) {
  char *resolved = ::realpath(path.path.c_str(), nullptr);
  if (resolved == nullptr)
    fail(errnoMessage("realpath Landlock path"));
  const std::string canonical(resolved);
  std::free(resolved);
  if (canonical != path.path)
    fail("Landlock path is not canonical: " + path.label);

  auto [parentPath, basename] = splitParent(path.path);
  path.parentPath = parentPath;
  path.basename = basename;
  path.parentFd = Fd(::open(parentPath.c_str(),
                            O_PATH | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (!path.parentFd)
    fail(errnoMessage("open Landlock path parent"));
  path.fd = Fd(::openat(path.parentFd.get(), basename.c_str(),
                        O_PATH | O_CLOEXEC | O_NOFOLLOW));
  if (!path.fd)
    fail(errnoMessage("open Landlock path"));
  struct stat status{};
  if (::fstat(path.fd.get(), &status) != 0 || S_ISLNK(status.st_mode))
    fail("Landlock path cannot be retained: " + path.label);
  path.directory = S_ISDIR(status.st_mode);
  if (path.access == LandlockPathAccess::WritableRoot && !path.directory)
    fail("Landlock writable root is not a directory: " + path.label);
  if (path.access == LandlockPathAccess::ReadWriteFile && path.directory)
    fail("Landlock read-write file is a directory: " + path.label);
  struct stat named{};
  if (::fstatat(path.parentFd.get(), path.basename.c_str(), &named,
                AT_SYMLINK_NOFOLLOW) != 0 ||
      named.st_dev != status.st_dev || named.st_ino != status.st_ino ||
      named.st_mode != status.st_mode)
    fail("Landlock path was replaced while opening: " + path.label);
  path.baselineIdentity = identityOf(status);
}

void verifyLandlockPath(LandlockPath &path) {
  struct stat retained{};
  struct stat named{};
  if (::fstat(path.fd.get(), &retained) != 0 ||
      ::fstatat(path.parentFd.get(), path.basename.c_str(), &named,
                AT_SYMLINK_NOFOLLOW) != 0 ||
      retained.st_dev != named.st_dev || retained.st_ino != named.st_ino ||
      retained.st_mode != named.st_mode)
    fail("Landlock path was replaced: " + path.label);
  if (path.access == LandlockPathAccess::WritableRoot)
    return;
  if (!(identityOf(retained) == path.baselineIdentity))
    fail("Landlock path identity changed: " + path.label);
}

class LandlockSandbox {
public:
  explicit LandlockSandbox(const Options &options) {
    errno = 0;
    const long abi = ::syscall(SYS_landlock_create_ruleset, nullptr, 0,
                               LANDLOCK_CREATE_RULESET_VERSION);
    if (abi != kRequiredLandlockAbi)
      fail("Landlock ABI must be exactly " +
           std::to_string(kRequiredLandlockAbi) + ", observed " +
           (abi < 0 ? errnoMessage("query failed") : std::to_string(abi)));
    abi_ = static_cast<int>(abi);

    struct landlock_ruleset_attr attributes{};
    attributes.handled_access_fs = kAllLandlockFilesystemRights;
    const long created = ::syscall(SYS_landlock_create_ruleset, &attributes,
                                   sizeof(attributes), 0);
    if (created < 0)
      fail(errnoMessage("landlock_create_ruleset"));
    ruleset_ = Fd(static_cast<int>(created));

    for (const Root &root : options.roots)
      if (root.landlockReadable)
        addRule(root.directories.front().fd.get(), kLandlockReadDirectoryRights,
                "guarded root " + root.label);
    for (const TrackedFile &file : options.files) {
      uint64_t rights = kLandlockReadFileRights;
      if ((file.expectedMode & 0111U) != 0)
        rights |= LANDLOCK_ACCESS_FS_EXECUTE;
      addRule(file.fd.get(), rights, "tracked file " + file.label);
    }
    for (const LandlockPath &path : options.landlockPaths) {
      uint64_t rights = 0;
      if (path.access == LandlockPathAccess::WritableRoot) {
        rights = kLandlockWritableRootRights;
      } else if (path.access == LandlockPathAccess::ReadWriteFile) {
        rights = kLandlockReadWriteFileRights;
      } else {
        rights = path.directory ? kLandlockReadDirectoryRights
                                : kLandlockReadFileRights;
      }
      addRule(path.fd.get(), rights, "modeled path " + path.label);
    }
  }

  int abi() const { return abi_; }

  void restrictChild() const {
    if (::prctl(PR_SET_NO_NEW_PRIVS, 1L, 0L, 0L, 0L) != 0)
      _exit(124);
    if (::syscall(SYS_landlock_restrict_self, ruleset_.get(), 0) != 0)
      _exit(125);
  }

private:
  void addRule(int parentFd, uint64_t rights, const std::string &label) {
    struct landlock_path_beneath_attr path{};
    path.allowed_access = rights;
    path.parent_fd = parentFd;
    if (::syscall(SYS_landlock_add_rule, ruleset_.get(),
                  LANDLOCK_RULE_PATH_BENEATH, &path, 0) != 0)
      fail(errnoMessage("landlock_add_rule for " + label));
  }

  Fd ruleset_;
  int abi_ = 0;
};

class ChildSeccompPolicy {
public:
  ChildSeccompPolicy() {
    filters_.push_back(BPF_STMT(BPF_LD | BPF_W | BPF_ABS,
                                offsetof(struct seccomp_data, arch)));
    filters_.push_back(
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_X86_64, 1, 0));
    filters_.push_back(BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS));
    filters_.push_back(
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)));
    filters_.push_back(BPF_JUMP(BPF_JMP | BPF_JSET | BPF_K, 0x40000000U, 0, 1));
    filters_.push_back(BPF_STMT(
        BPF_RET | BPF_K, SECCOMP_RET_ERRNO | static_cast<uint32_t>(EPERM)));
    for (int syscallNumber : deniedSyscalls()) {
      filters_.push_back(BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K,
                                  static_cast<uint32_t>(syscallNumber), 0, 1));
      filters_.push_back(BPF_STMT(
          BPF_RET | BPF_K, SECCOMP_RET_ERRNO | static_cast<uint32_t>(EPERM)));
    }
    filters_.push_back(BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW));
    if (filters_.size() > std::numeric_limits<unsigned short>::max())
      fail("child seccomp policy is too large");
  }

  void installInChild() const {
    struct sock_fprog program{
        static_cast<unsigned short>(filters_.size()),
        const_cast<struct sock_filter *>(filters_.data())};
    if (::prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program) != 0)
      _exit(119);
  }

  size_t deniedCount() const { return deniedSyscalls().size(); }

private:
  static const std::vector<int> &deniedSyscalls() {
    static const std::vector<int> denied = {
        SYS_socket,
        SYS_socketpair,
        SYS_connect,
        SYS_bind,
        SYS_listen,
        SYS_accept,
        SYS_accept4,
        SYS_getsockname,
        SYS_getpeername,
        SYS_sendto,
        SYS_recvfrom,
        SYS_setsockopt,
        SYS_getsockopt,
        SYS_shutdown,
        SYS_sendmsg,
        SYS_recvmsg,
        SYS_recvmmsg,
        SYS_sendmmsg,
        SYS_pidfd_open,
        SYS_pidfd_getfd,
        SYS_process_vm_readv,
        SYS_process_vm_writev,
        SYS_io_uring_setup,
        SYS_io_uring_enter,
        SYS_io_uring_register,
    };
    return denied;
  }

  std::vector<struct sock_filter> filters_;
};

class ChildIo {
public:
  explicit ChildIo(const Options &options) {
    const auto found =
        std::find_if(options.landlockPaths.begin(), options.landlockPaths.end(),
                     [](const LandlockPath &path) {
                       return path.path == "/dev/null" &&
                              path.access == LandlockPathAccess::ReadWriteFile;
                     });
    if (found == options.landlockPaths.end() || found->directory)
      fail("canonical read-write /dev/null is required for child stdio");
    nullFd_ = Fd(::openat(found->parentFd.get(), found->basename.c_str(),
                          O_RDWR | O_CLOEXEC | O_NOFOLLOW));
    if (!nullFd_)
      fail(errnoMessage("open child stdio /dev/null"));
    struct stat retained{};
    struct stat opened{};
    if (::fstat(found->fd.get(), &retained) != 0 ||
        ::fstat(nullFd_.get(), &opened) != 0 || !S_ISCHR(opened.st_mode) ||
        retained.st_dev != opened.st_dev || retained.st_ino != opened.st_ino)
      fail("child stdio source is not retained /dev/null");
    device_ = static_cast<uint64_t>(opened.st_dev);
    inode_ = static_cast<uint64_t>(opened.st_ino);
  }

  void verifySource() const {
    struct stat status{};
    if (::fstat(nullFd_.get(), &status) != 0 || !S_ISCHR(status.st_mode) ||
        static_cast<uint64_t>(status.st_dev) != device_ ||
        static_cast<uint64_t>(status.st_ino) != inode_)
      fail("child stdio /dev/null source changed");
  }

  void installInChild(int stdoutPipe = -1) const {
    const int safeNull = ::fcntl(nullFd_.get(), F_DUPFD_CLOEXEC, 10);
    if (safeNull < 0)
      _exit(120);
    int safeOutput = -1;
    struct stat outputStatus{};
    if (stdoutPipe >= 0) {
      safeOutput = ::fcntl(stdoutPipe, F_DUPFD_CLOEXEC, 10);
      if (safeOutput < 0 || ::fstat(safeOutput, &outputStatus) != 0 ||
          !S_ISFIFO(outputStatus.st_mode))
        _exit(121);
    }
    if (::dup2(safeNull, STDIN_FILENO) < 0 ||
        ::dup2(stdoutPipe >= 0 ? safeOutput : safeNull, STDOUT_FILENO) < 0 ||
        ::dup2(safeNull, STDERR_FILENO) < 0)
      _exit(122);
#ifdef SYS_close_range
    if (::syscall(SYS_close_range, 3U, std::numeric_limits<unsigned int>::max(),
                  2U) != 0)
      _exit(123);
#else
    _exit(123);
#endif
    struct stat input{};
    struct stat output{};
    struct stat error{};
    if (::fstat(STDIN_FILENO, &input) != 0 ||
        ::fstat(STDOUT_FILENO, &output) != 0 ||
        ::fstat(STDERR_FILENO, &error) != 0 || !S_ISCHR(input.st_mode) ||
        !S_ISCHR(error.st_mode) ||
        static_cast<uint64_t>(input.st_dev) != device_ ||
        static_cast<uint64_t>(input.st_ino) != inode_ ||
        static_cast<uint64_t>(error.st_dev) != device_ ||
        static_cast<uint64_t>(error.st_ino) != inode_ ||
        (stdoutPipe < 0 && (!S_ISCHR(output.st_mode) ||
                            static_cast<uint64_t>(output.st_dev) != device_ ||
                            static_cast<uint64_t>(output.st_ino) != inode_)) ||
        (stdoutPipe >= 0 &&
         (!S_ISFIFO(output.st_mode) || output.st_dev != outputStatus.st_dev ||
          output.st_ino != outputStatus.st_ino)))
      _exit(126);
  }

private:
  Fd nullFd_;
  uint64_t device_ = 0;
  uint64_t inode_ = 0;
};

void applyCommandEnvironment(const Options &options) {
  if (::setenv("LD_PRELOAD", options.tmpRedirectPath.c_str(), 1) != 0 ||
      ::setenv("FE2O3_PRIVATE_TMP", options.privateTmpPath.c_str(), 1) != 0)
    _exit(124);
}

std::string runResolver(const Options &options, const std::string &name,
                        const LandlockSandbox &sandbox,
                        const ChildSeccompPolicy &seccomp,
                        const ChildIo &childIo) {
  int pipeFds[2];
  if (::pipe2(pipeFds, O_CLOEXEC) != 0)
    fail(errnoMessage("pipe2 resolver"));
  Fd readEnd(pipeFds[0]);
  Fd writeEnd(pipeFds[1]);
  struct stat writeStatus{};
  if (::fstat(writeEnd.get(), &writeStatus) != 0 ||
      !S_ISFIFO(writeStatus.st_mode))
    fail("resolver output pipe is not a pipe");
  childIo.verifySource();
  const pid_t child = ::fork();
  if (child < 0)
    fail(errnoMessage("fork resolver"));
  if (child == 0) {
    const std::string argument = "-print-file-name=" + name;
    char *const arguments[] = {const_cast<char *>(options.resolver.c_str()),
                               const_cast<char *>(argument.c_str()), nullptr};
    applyCommandEnvironment(options);
    sandbox.restrictChild();
    seccomp.installInChild();
    childIo.installInChild(writeEnd.get());
    ::execve(options.resolver.c_str(), arguments, environ);
    _exit(127);
  }
  writeEnd = Fd();
  std::string output;
  std::array<char, 4096> buffer{};
  while (true) {
    const ssize_t count = ::read(readEnd.get(), buffer.data(), buffer.size());
    if (count < 0) {
      if (errno == EINTR)
        continue;
      fail(errnoMessage("read resolver output"));
    }
    if (count == 0)
      break;
    output.append(buffer.data(), static_cast<size_t>(count));
    if (output.size() > 4096)
      fail("runtime resolver output is too long");
  }
  int status = 0;
  while (::waitpid(child, &status, 0) < 0) {
    if (errno != EINTR)
      fail(errnoMessage("waitpid resolver"));
  }
  if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
    fail("runtime resolver failed for " + name);
  while (!output.empty() && (output.back() == '\n' || output.back() == '\r'))
    output.pop_back();
  char *resolved = ::realpath(output.c_str(), nullptr);
  if (resolved == nullptr)
    fail(errnoMessage("realpath resolver output"));
  const std::string canonical(resolved);
  std::free(resolved);
  return canonical;
}

void verifyRuntimeResolution(const Options &options,
                             const LandlockSandbox &sandbox,
                             const ChildSeccompPolicy &seccomp,
                             const ChildIo &childIo) {
  for (const RuntimeInput &runtime : options.runtimeInputs) {
    if (runResolver(options, runtime.name, sandbox, seccomp, childIo) !=
        runtime.path)
      fail("runtime resolver changed for " + runtime.name);
  }
}

void runCommand(const Options &options, const Command &command,
                const LandlockSandbox &sandbox,
                const ChildSeccompPolicy &seccomp, const ChildIo &childIo) {
  std::vector<char *> arguments;
  arguments.reserve(command.arguments.size() + 1);
  for (const std::string &argument : command.arguments)
    arguments.push_back(const_cast<char *>(argument.c_str()));
  arguments.push_back(nullptr);
  childIo.verifySource();
  const pid_t child = ::fork();
  if (child < 0)
    fail(errnoMessage("fork build command"));
  if (child == 0) {
    applyCommandEnvironment(options);
    sandbox.restrictChild();
    seccomp.installInChild();
    childIo.installInChild();
    ::execve(arguments[0], arguments.data(), environ);
    _exit(127);
  }
  int status = 0;
  while (::waitpid(child, &status, 0) < 0) {
    if (errno != EINTR)
      fail(errnoMessage("waitpid build command"));
  }
  if (!WIFEXITED(status))
    fail("build command terminated by a signal");
  if (WEXITSTATUS(status) != 0)
    fail("build command failed with exit " +
         std::to_string(WEXITSTATUS(status)) + ": " +
         command.arguments.front());
}

void verifyRootPath(Root &root) {
  struct stat retained{};
  struct stat named{};
  if (::fstat(root.directories.front().fd.get(), &retained) != 0 ||
      ::fstatat(root.parentFd.get(), root.basename.c_str(), &named,
                AT_SYMLINK_NOFOLLOW) != 0 ||
      !S_ISDIR(named.st_mode) || retained.st_dev != named.st_dev ||
      retained.st_ino != named.st_ino)
    fail("guarded root pathname was replaced: " + root.label);
}

std::string landlockAccessText(LandlockPathAccess access) {
  switch (access) {
  case LandlockPathAccess::ReadOnly:
    return "read-only";
  case LandlockPathAccess::ReadWriteFile:
    return "read-write-file";
  case LandlockPathAccess::WritableRoot:
    return "writable-root-symlink-creation-denied";
  }
  fail("unknown Landlock path access");
}

std::string buildStatus(const Options &options, const LandlockSandbox &sandbox,
                        const ChildSeccompPolicy &seccomp) {
  std::string status;
  status += "FORMAT=fe2o3-static-host-lld-build-guard-status-v1\n";
  status += "STATUS=passed\n";
  status += "SCOPE=measured-build-closure-integrity-with-landlock-filesystem-"
            "enforcement-and-observational-input-admission\n";
  status += "LANDLOCK_FILESYSTEM_ENFORCEMENT=passed\n";
  status += "LANDLOCK_ABI=" + std::to_string(sandbox.abi()) + "\n";
  status += "LANDLOCK_HANDLED_FS_RIGHTS=0x7fff\n";
  status += "LANDLOCK_REFER=handled\n";
  status += "LANDLOCK_TRUNCATE=handled\n";
  status += "LANDLOCK_MAKE_SYM=handled-and-denied-in-writable-roots\n";
  status += "NETWORK_IPC_ISOLATION=provided-by-seccomp-deny-policy-v1\n";
  status += "SECCOMP_X32_TAGGED_SYSCALLS=denied-with-EPERM-before-table-v1\n";
  status += "NETWORK_NAMESPACE_ISOLATION=not_provided\n";
  status +=
      "SECCOMP_DENIED_SYSCALLS=" + std::to_string(seccomp.deniedCount()) + "\n";
  status += "SECCOMP_FD_TRANSFER=socket-and-pidfd-paths-denied\n";
  status += "SECCOMP_ASYNC_IO_BYPASS=io-uring-denied\n";
  status += "SECCOMP_PTRACE=allowed-for-retained-strace-launcher\n";
  status += "PROCESS_ISOLATION=not_provided\n";
  status += "PROCESS_CREATION=allowed-required-subprocesses-inherit-policy\n";
  status += "CHILD_FD_TABLE=stdio-dev-null-with-resolver-stdout-pipe-only\n";
  status += "INHERITED_AMBIENT_DESCRIPTORS=closed-before-child-exec\n";
  status += "AMBIENT_TMP_ACCESS=landlock-open-denied-with-partial-libc-"
            "metadata-redirect\n";
  status += "TMP_METADATA_REDIRECT=partial-reviewed-libc-symbol-"
            "interposition\n";
  status += "DIRECT_TMP_SYSCALL_REDIRECTION=not_provided\n";
  status += "GLOBAL_TMP_FILE_OPEN=landlock-denied\n";
  status += "GLOBAL_TMP_METADATA_SYSCALLS=observational-only\n";
  status += "STATUS_PUBLICATION=descriptor-bound-unprotected-measurement\n";
  status += "POST_PUBLICATION_MUTATION_CHECK=bounded-inotify-quiet-window\n";
  status += "PROTECTED_PUBLICATION=absent\n";
  status += "TMP_REDIRECT_LIBRARY=" + options.tmpRedirectPath + "\n";
  status += "PRIVATE_TMP_PATH=" + options.privateTmpPath + "\n";
  status += "INOTIFY_OVERFLOW=absent\n";
  status += "MUTATION_EVENTS=0\n";
  status += "COMMANDS=" + std::to_string(options.commands.size()) + "\n";
  status += "ROOTS=" + std::to_string(options.roots.size()) + "\n";
  status += "DIRECTORIES=" + std::to_string(options.directories.size()) + "\n";
  status += "FILES=" + std::to_string(options.files.size()) + "\n";
  status +=
      "LANDLOCK_MODELED_PATHS=" + std::to_string(options.landlockPaths.size()) +
      "\n";
  for (const Root &root : options.roots) {
    struct stat rootStatus{};
    if (::fstat(root.directories.front().fd.get(), &rootStatus) != 0)
      fail(errnoMessage("fstat root for status"));
    const std::string prefix = "ROOT_" + root.label + "_";
    status += prefix + "PATH=" + root.path + "\n";
    status += prefix + "LANDLOCK_ACCESS=" +
              (root.landlockReadable ? "read-only" : "none-absence-only") +
              "\n";
    status += prefix + "IDENTITY=" + statIdentity(rootStatus) + "\n";
    status += prefix + "MANIFEST_SHA256=" + root.baselineDigest + "\n";
    status += prefix +
              "MANIFEST_LENGTH=" + std::to_string(root.baselineManifestLength) +
              "\n";
  }
  for (const WatchedDirectory &directory : options.directories) {
    struct stat directoryStatus{};
    if (::fstat(directory.fd.get(), &directoryStatus) != 0)
      fail(errnoMessage("fstat watched directory for status"));
    const std::string prefix = "DIRECTORY_" + directory.label + "_";
    status += prefix + "PATH=" + directory.path + "\n";
    status += prefix + "SCOPE=" +
              (directory.contentsStable ? "contents-and-entry"
                                        : "ancestor-entry-only") +
              "\n";
    status += prefix + "IDENTITY=" + statIdentity(directoryStatus) + "\n";
  }
  for (const TrackedFile &file : options.files) {
    const std::string prefix = "FILE_" + file.label + "_";
    status += prefix + "PATH=" + file.path + "\n";
    status += prefix + "SHA256=" + file.expectedDigest + "\n";
    status += prefix + "LENGTH=" + std::to_string(file.expectedLength) + "\n";
    status += prefix +
              "MODE=" + modeText(static_cast<mode_t>(file.expectedMode)) + "\n";
  }
  for (const LandlockPath &path : options.landlockPaths) {
    struct stat pathStatus{};
    if (::fstat(path.fd.get(), &pathStatus) != 0)
      fail(errnoMessage("fstat Landlock path for status"));
    const std::string prefix = "LANDLOCK_PATH_" + path.label + "_";
    status += prefix + "PATH=" + path.path + "\n";
    status += prefix + "ACCESS=" + landlockAccessText(path.access) + "\n";
    status += prefix + "TYPE=" + (path.directory ? "directory" : "file") + "\n";
    status += prefix + "IDENTITY=" + statIdentity(pathStatus) + "\n";
  }
  status += "STATUS_TERMINAL=fe2o3-static-host-lld-build-guard-status-v1-end\n";
  return status;
}

class StatusPublication {
public:
  explicit StatusPublication(const std::string &path) {
    auto [parent, base] = splitNonexistentOutput(path);
    parentPath_ = std::move(parent);
    basename_ = std::move(base);
    parentFd_ = Fd(::open(parentPath_.c_str(),
                          O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
    if (!parentFd_)
      fail(errnoMessage("open retained status parent"));
    struct stat parentStatus{};
    if (::fstat(parentFd_.get(), &parentStatus) != 0 ||
        !S_ISDIR(parentStatus.st_mode))
      fail("retained status parent is not a directory");
    parentDevice_ = static_cast<uint64_t>(parentStatus.st_dev);
    parentInode_ = static_cast<uint64_t>(parentStatus.st_ino);
  }

  void publish(const std::string &status, Watches &watches) const {
    const std::string expectedDigest = sha256Text(status);
    verifyParent();
    Fd output(::openat(parentFd_.get(), basename_.c_str(),
                       O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW,
                       0600));
    if (!output)
      fail(errnoMessage("create guard status through retained parent"));
    try {
      size_t offset = 0;
      while (offset < status.size()) {
        const ssize_t count = ::write(output.get(), status.data() + offset,
                                      status.size() - offset);
        if (count < 0) {
          if (errno == EINTR)
            continue;
          fail(errnoMessage("write guard status"));
        }
        if (count == 0)
          fail("guard status write made no progress");
        offset += static_cast<size_t>(count);
      }
      if (::fsync(output.get()) != 0)
        fail(errnoMessage("fsync guard status"));
      verifyOutput(output.get(), status.size(), expectedDigest);
      verifyParent();
      watches.requireQuietWindow("after status publication",
                                 std::chrono::milliseconds(100));
      verifyOutput(output.get(), status.size(), expectedDigest);
      verifyParent();
    } catch (...) {
      (void)::unlinkat(parentFd_.get(), basename_.c_str(), 0);
      throw;
    }
  }

private:
  void verifyParent() const {
    Fd named(::open(parentPath_.c_str(),
                    O_PATH | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
    struct stat status{};
    if (!named || ::fstat(named.get(), &status) != 0 ||
        !S_ISDIR(status.st_mode) ||
        static_cast<uint64_t>(status.st_dev) != parentDevice_ ||
        static_cast<uint64_t>(status.st_ino) != parentInode_)
      fail("status parent pathname was replaced");
  }

  void verifyOutput(int outputFd, size_t length,
                    const std::string &expectedDigest) const {
    struct stat retained{};
    struct stat named{};
    if (::fstat(outputFd, &retained) != 0 ||
        ::fstatat(parentFd_.get(), basename_.c_str(), &named,
                  AT_SYMLINK_NOFOLLOW) != 0 ||
        !S_ISREG(retained.st_mode) || !S_ISREG(named.st_mode) ||
        retained.st_dev != named.st_dev || retained.st_ino != named.st_ino ||
        retained.st_mode != named.st_mode || retained.st_nlink != 1 ||
        retained.st_size < 0 ||
        static_cast<uint64_t>(retained.st_size) !=
            static_cast<uint64_t>(length))
      fail("guard status pathname or descriptor identity changed");
    const Identity before = identityOf(retained);
    const std::string observedDigest =
        sha256Fd(outputFd, static_cast<uint64_t>(length));
    struct stat after{};
    struct stat namedAfter{};
    if (::fstat(outputFd, &after) != 0 ||
        ::fstatat(parentFd_.get(), basename_.c_str(), &namedAfter,
                  AT_SYMLINK_NOFOLLOW) != 0 ||
        !(identityOf(after) == before) || after.st_dev != namedAfter.st_dev ||
        after.st_ino != namedAfter.st_ino || observedDigest != expectedDigest)
      fail("guard status content changed during publication");
  }

  std::string parentPath_;
  std::string basename_;
  Fd parentFd_;
  uint64_t parentDevice_ = 0;
  uint64_t parentInode_ = 0;
};

int guardMain(int argc, char **argv) {
  Options options = parseOptions(argc, argv);
  raiseDescriptorLimit();
  const StatusPublication statusPublication(options.statusPath);
  Watches watches;
  for (Root &root : options.roots)
    setupRoot(root, watches, options);
  for (WatchedDirectory &directory : options.directories)
    setupWatchedDirectory(directory, watches);
  for (TrackedFile &file : options.files)
    setupTrackedFile(file, watches);
  for (LandlockPath &path : options.landlockPaths)
    setupLandlockPath(path);
  watches.requireQuiet("watch setup");

  for (Root &root : options.roots)
    snapshotRoot(root, options, true);
  for (WatchedDirectory &directory : options.directories)
    verifyWatchedDirectory(directory, true);
  for (TrackedFile &file : options.files)
    verifyTrackedFile(file, true);
  for (LandlockPath &path : options.landlockPaths)
    verifyLandlockPath(path);
  const LandlockSandbox sandbox(options);
  const ChildSeccompPolicy seccomp;
  const ChildIo childIo(options);
  verifyRuntimeResolution(options, sandbox, seccomp, childIo);
  watches.requireQuiet("measured baseline");

  for (size_t index = 0; index < options.commands.size(); ++index) {
    verifyRuntimeResolution(options, sandbox, seccomp, childIo);
    for (WatchedDirectory &directory : options.directories)
      verifyWatchedDirectory(directory, false);
    for (TrackedFile &file : options.files)
      verifyTrackedFile(file, false);
    for (LandlockPath &path : options.landlockPaths)
      verifyLandlockPath(path);
    watches.requireQuiet("before command " + std::to_string(index + 1));
    runCommand(options, options.commands[index], sandbox, seccomp, childIo);
    verifyRuntimeResolution(options, sandbox, seccomp, childIo);
    for (WatchedDirectory &directory : options.directories)
      verifyWatchedDirectory(directory, false);
    for (TrackedFile &file : options.files)
      verifyTrackedFile(file, false);
    for (LandlockPath &path : options.landlockPaths)
      verifyLandlockPath(path);
    watches.requireQuiet("after command " + std::to_string(index + 1));
  }

  for (Root &root : options.roots) {
    verifyRootPath(root);
    snapshotRoot(root, options, false);
  }
  for (WatchedDirectory &directory : options.directories)
    verifyWatchedDirectory(directory, false);
  for (TrackedFile &file : options.files)
    verifyTrackedFile(file, false);
  for (LandlockPath &path : options.landlockPaths)
    verifyLandlockPath(path);
  verifyRuntimeResolution(options, sandbox, seccomp, childIo);
  const std::string status = buildStatus(options, sandbox, seccomp);
  watches.requireQuiet("final revalidation");
  statusPublication.publish(status, watches);
  return 0;
}

} // namespace

int main(int argc, char **argv) {
  try {
    return guardMain(argc, argv);
  } catch (const std::exception &error) {
    std::fprintf(stderr, "fe2o3-static-host-lld-build-guard: %s\n",
                 error.what());
    return 70;
  }
}
