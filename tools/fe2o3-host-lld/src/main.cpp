#include "BuildConfig.h"
#include "SecureProtocol.h"

#include "lld/Common/Driver.h"
#include "llvm/ADT/ArrayRef.h"
#include "llvm/ADT/StringRef.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/Magic.h"
#include "llvm/Object/Archive.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/MemoryBufferRef.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/raw_ostream.h"

#include <algorithm>
#include <array>
#include <cctype>
#include <cerrno>
#include <climits>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <dirent.h>
#include <elf.h>
#include <fcntl.h>
#include <limits>
#include <linux/capability.h>
#include <linux/magic.h>
#include <linux/memfd.h>
#include <optional>
#include <signal.h>
#include <string>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>
#include <utility>
#include <vector>

using llvm::StringRef;

LLD_HAS_DRIVER(elf)

extern char **environ;

namespace {

using namespace fe2o3::host_lld;

constexpr size_t MaxArgumentCount = 4096;
constexpr size_t MaxArgumentBytes = 4096;
constexpr size_t MaxTotalArgumentBytes = 1024 * 1024;
constexpr size_t MaxDiagnosticBytes = 64 * 1024;
constexpr size_t MaxUniqueInputCount = 2048;
constexpr uint64_t MaxInputBytes = 256ULL * 1024 * 1024;
constexpr uint64_t MaxTotalInputBytes = 2ULL * 1024 * 1024 * 1024;
constexpr uint64_t MaxOutputBytes = 512ULL * 1024 * 1024;
constexpr uint64_t MaxAddressSpaceBytes = 4ULL * 1024 * 1024 * 1024;
constexpr uint64_t MaxArchiveMembers = 262144;
constexpr rlim_t MaxCpuSeconds = 60;
constexpr StringRef OutputMemfdName = "fe2o3-host-lld-output";
constexpr int LinuxX86KernelSignalMaximum = 64;
constexpr size_t LinuxX86KernelSignalSetBytes = sizeof(uint64_t);

enum class ExitCode : int {
  Success = 0,
  Usage = 64,
  Environment = 65,
  LinkFailure = 66,
  Internal = 70,
};

enum class InputKind { ElfRel, Archive, Rlib };

struct KernelObjectIdentity {
  dev_t Device = 0;
  ino_t Inode = 0;
  mode_t Mode = 0;
};

bool measureKernelObject(int fd, KernelObjectIdentity &identity) {
  struct stat status{};
  if (::fstat(fd, &status) != 0)
    return false;
  identity.Device = status.st_dev;
  identity.Inode = status.st_ino;
  identity.Mode = status.st_mode;
  return true;
}

bool sameKernelObject(const KernelObjectIdentity &left,
                      const KernelObjectIdentity &right) {
  return left.Device == right.Device && left.Inode == right.Inode &&
         (left.Mode & S_IFMT) == (right.Mode & S_IFMT);
}

bool isNullDevice(int fd) {
  struct stat status{};
  return ::fstat(fd, &status) == 0 && S_ISCHR(status.st_mode) &&
         major(status.st_rdev) == 1U && minor(status.st_rdev) == 3U;
}

bool canonicalizeStandardDescriptors() {
  int nullFd = ::open("/dev/null", O_RDWR | O_CLOEXEC | O_NOCTTY | O_NONBLOCK);
  if (nullFd < 0 || !isNullDevice(nullFd)) {
    if (nullFd >= 0)
      (void)::close(nullFd);
    return false;
  }
  if (nullFd <= STDERR_FILENO) {
    const int duplicate = ::fcntl(nullFd, F_DUPFD_CLOEXEC, 3);
    const int closeStatus = ::close(nullFd);
    if (duplicate < 0 || closeStatus != 0)
      return false;
    nullFd = duplicate;
  }
  for (int target = STDIN_FILENO; target <= STDERR_FILENO; ++target) {
    if (::dup3(nullFd, target, 0) != target || !isNullDevice(target)) {
      (void)::close(nullFd);
      return false;
    }
  }
  return ::close(nullFd) == 0;
}

class BoundedRawStream final : public llvm::raw_ostream {
public:
  explicit BoundedRawStream(size_t limit) : Limit(limit) { SetUnbuffered(); }

  StringRef str() const { return Buffer; }
  bool truncated() const { return Truncated; }

private:
  void write_impl(const char *pointer, size_t size) override {
    const size_t available = Buffer.size() < Limit ? Limit - Buffer.size() : 0;
    const size_t copied = std::min(size, available);
    Buffer.append(pointer, copied);
    Truncated = Truncated || copied != size;
    const uint64_t increment = static_cast<uint64_t>(size);
    Position = increment > std::numeric_limits<uint64_t>::max() - Position
                   ? std::numeric_limits<uint64_t>::max()
                   : Position + increment;
  }

  uint64_t current_pos() const override { return Position; }

  size_t Limit;
  std::string Buffer;
  bool Truncated = false;
  uint64_t Position = 0;
};

[[noreturn]] void immediateExit(ExitCode code) {
  llvm::outs().flush();
  llvm::errs().flush();
  _exit(static_cast<int>(code));
}

int fail(ExitCode code, StringRef message) {
  llvm::errs() << "fe2o3-host-lld: " << message << '\n';
  return static_cast<int>(code);
}

bool hasForbiddenEnvironmentName(StringRef name) {
  static constexpr StringRef Allowed[] = {
      "LC_ALL",
      "LANG",
      "TZ",
      "SOURCE_DATE_EPOCH",
  };
  for (StringRef candidate : Allowed)
    if (name == candidate)
      return false;
  return true;
}

bool canonicalEnvironmentValue(StringRef name, StringRef value) {
  if (name == "LC_ALL" || name == "LANG")
    return value == "C";
  if (name == "TZ")
    return value == "UTC";
  if (name == "SOURCE_DATE_EPOCH")
    return value == "0";
  return true;
}

int sanitizeEnvironment() {
  for (char **entry = environ; entry != nullptr && *entry != nullptr; ++entry) {
    StringRef assignment(*entry);
    const size_t separator = assignment.find('=');
    if (separator == StringRef::npos)
      return fail(ExitCode::Environment, "malformed process environment");
    const StringRef name = assignment.take_front(separator);
    const StringRef value = assignment.drop_front(separator + 1);
    if (hasForbiddenEnvironmentName(name))
      return fail(ExitCode::Environment,
                  "environment-based search or configuration is forbidden");
    if (!canonicalEnvironmentValue(name, value))
      return fail(
          ExitCode::Environment,
          "locale, time, and reproducibility environment is noncanonical");
  }
  if (::clearenv() != 0 || ::setenv("LC_ALL", "C", 1) != 0 ||
      ::setenv("LANG", "C", 1) != 0 || ::setenv("TZ", "UTC", 1) != 0 ||
      ::setenv("SOURCE_DATE_EPOCH", "0", 1) != 0)
    return fail(ExitCode::Environment,
                "could not establish the canonical process environment");
  ::umask(077);
  return static_cast<int>(ExitCode::Success);
}

using KernelSignalHandler = void (*)(int);
using KernelSignalRestorer = void (*)();

// Linux x86-64 passes this layout and an eight-byte signal set directly to
// rt_sigaction. The libc wrapper deliberately hides its reserved RT signals.
struct LinuxX86KernelSigaction {
  KernelSignalHandler Handler;
  unsigned long Flags;
  KernelSignalRestorer Restorer;
  uint64_t Mask;
};

static_assert(sizeof(void *) == 8 && sizeof(unsigned long) == 8);
static_assert(sizeof(LinuxX86KernelSigaction) == 32);
static_assert(offsetof(LinuxX86KernelSigaction, Handler) == 0);
static_assert(offsetof(LinuxX86KernelSigaction, Flags) == 8);
static_assert(offsetof(LinuxX86KernelSigaction, Restorer) == 16);
static_assert(offsetof(LinuxX86KernelSigaction, Mask) == 24);

bool setKernelSignalDisposition(int signal, KernelSignalHandler handler) {
  const LinuxX86KernelSigaction action{handler, 0, nullptr, 0};
  return ::syscall(SYS_rt_sigaction, signal, &action, nullptr,
                   LinuxX86KernelSignalSetBytes) == 0;
}

bool emptyKernelSignalMask() {
  const uint64_t emptyMask = 0;
  return ::syscall(SYS_rt_sigprocmask, SIG_SETMASK, &emptyMask, nullptr,
                   LinuxX86KernelSignalSetBytes) == 0;
}

int normalizeInheritedSignalState() {
  for (int signal = 1; signal <= LinuxX86KernelSignalMaximum; ++signal) {
    if (signal == SIGKILL || signal == SIGSTOP)
      continue;
    if (!setKernelSignalDisposition(signal, SIG_DFL))
      return fail(ExitCode::Internal,
                  "could not reset an inherited signal disposition");
  }
  if (!emptyKernelSignalMask())
    return fail(ExitCode::Internal,
                "could not establish the canonical empty signal mask");
  return static_cast<int>(ExitCode::Success);
}

bool lowerResourceLimit(int resource, rlim_t maximum) {
  rlimit limit{};
  if (::getrlimit(resource, &limit) != 0)
    return false;
  if (limit.rlim_max == RLIM_INFINITY || limit.rlim_max > maximum)
    limit.rlim_max = maximum;
  if (limit.rlim_cur == RLIM_INFINITY || limit.rlim_cur > limit.rlim_max)
    limit.rlim_cur = limit.rlim_max;
  return ::setrlimit(resource, &limit) == 0;
}

int establishResourceBounds() {
  if (!setKernelSignalDisposition(SIGXFSZ, SIG_IGN) ||
      !lowerResourceLimit(RLIMIT_FSIZE, static_cast<rlim_t>(MaxOutputBytes)) ||
      !lowerResourceLimit(RLIMIT_AS,
                          static_cast<rlim_t>(MaxAddressSpaceBytes)) ||
      !lowerResourceLimit(RLIMIT_CPU, MaxCpuSeconds))
    return fail(ExitCode::Internal,
                "could not establish fixed process resource bounds");
  return static_cast<int>(ExitCode::Success);
}

struct ProcessBoundary {
  int ProcFdDirectory = -1;
  int MountNamespace = -1;
  KernelObjectIdentity ProcFdIdentity;
  KernelObjectIdentity MountNamespaceIdentity;
};

bool procFilesystem(int fd) {
  struct statfs filesystem{};
  return ::fstatfs(fd, &filesystem) == 0 &&
         static_cast<unsigned long>(filesystem.f_type) ==
             static_cast<unsigned long>(PROC_SUPER_MAGIC);
}

bool openProcessBoundary(ProcessBoundary &boundary, std::string &error) {
  boundary.ProcFdDirectory =
      ::open("/proc/self/fd", O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
  if (boundary.ProcFdDirectory < 0 ||
      !procFilesystem(boundary.ProcFdDirectory) ||
      !measureKernelObject(boundary.ProcFdDirectory, boundary.ProcFdIdentity) ||
      !S_ISDIR(boundary.ProcFdIdentity.Mode)) {
    error = "the retained descriptor directory is not authentic procfs";
    return false;
  }
  boundary.MountNamespace = ::open("/proc/self/ns/mnt", O_RDONLY | O_CLOEXEC);
  struct statfs namespaceFilesystem{};
  if (boundary.MountNamespace < 0 ||
      ::fstatfs(boundary.MountNamespace, &namespaceFilesystem) != 0 ||
      static_cast<unsigned long>(namespaceFilesystem.f_type) !=
          static_cast<unsigned long>(NSFS_MAGIC) ||
      !measureKernelObject(boundary.MountNamespace,
                           boundary.MountNamespaceIdentity)) {
    error = "the retained mount namespace identity is invalid";
    return false;
  }
  return true;
}

bool verifyProcessBoundary(const ProcessBoundary &boundary,
                           std::string &error) {
  if (!procFilesystem(boundary.ProcFdDirectory)) {
    error = "the retained procfs descriptor changed identity";
    return false;
  }
  const int currentProc =
      ::open("/proc/self/fd", O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
  const int currentNamespace =
      ::open("/proc/self/ns/mnt", O_RDONLY | O_CLOEXEC);
  KernelObjectIdentity procIdentity;
  KernelObjectIdentity namespaceIdentity;
  const bool valid =
      currentProc >= 0 && currentNamespace >= 0 &&
      procFilesystem(currentProc) &&
      measureKernelObject(currentProc, procIdentity) &&
      measureKernelObject(currentNamespace, namespaceIdentity) &&
      sameKernelObject(boundary.ProcFdIdentity, procIdentity) &&
      sameKernelObject(boundary.MountNamespaceIdentity, namespaceIdentity);
  if (currentProc >= 0)
    (void)::close(currentProc);
  if (currentNamespace >= 0)
    (void)::close(currentNamespace);
  if (!valid)
    error = "procfs or the process mount namespace changed identity";
  return valid;
}

bool hasProcessCapabilities() {
#ifdef SYS_capget
  __user_cap_header_struct header{};
  std::array<__user_cap_data_struct, 2> data{};
  header.version = _LINUX_CAPABILITY_VERSION_3;
  header.pid = 0;
  if (::syscall(SYS_capget, &header, data.data()) != 0)
    return true;
  for (const __user_cap_data_struct &word : data)
    if (word.effective != 0U || word.permitted != 0U || word.inheritable != 0U)
      return true;
#else
  return true;
#endif
  return false;
}

int establishPrivilegeBoundary() {
  if (::getuid() != ::geteuid() || ::getgid() != ::getegid() ||
      hasProcessCapabilities())
    return fail(ExitCode::Environment,
                "set-id or capability-bearing execution is forbidden");
  if (::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0)
    return fail(ExitCode::Internal, "could not disable privilege acquisition");
  return static_cast<int>(ExitCode::Success);
}

bool parseCanonicalUnsigned(StringRef value, uint64_t &result) {
  if (value.empty() || value.size() > 20 ||
      (value.size() > 1 && value.front() == '0'))
    return false;
  uint64_t parsed = 0;
  for (char digit : value) {
    if (digit < '0' || digit > '9')
      return false;
    const uint64_t decimal = static_cast<uint64_t>(digit - '0');
    if (parsed > (std::numeric_limits<uint64_t>::max() - decimal) / 10)
      return false;
    parsed = parsed * 10 + decimal;
  }
  result = parsed;
  return true;
}

bool parseCanonicalMode(StringRef value, mode_t &mode) {
  if (value.size() != 4 || value.front() != '0')
    return false;
  mode_t parsed = 0;
  for (char digit : value) {
    if (digit < '0' || digit > '7')
      return false;
    parsed =
        static_cast<mode_t>((parsed << 3U) | static_cast<mode_t>(digit - '0'));
  }
  if ((parsed & 07000) != 0 || (parsed & 0400) == 0)
    return false;
  mode = parsed;
  return true;
}

bool canonicalSha256(StringRef value) {
  if (value.size() != Sha256HexLength)
    return false;
  for (char byte : value)
    if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f')))
      return false;
  return true;
}

std::string lowercaseHex(const std::array<uint8_t, 32> &bytes) {
  static constexpr char Digits[] = "0123456789abcdef";
  std::string result;
  result.reserve(bytes.size() * 2);
  for (uint8_t byte : bytes) {
    result.push_back(Digits[byte >> 4]);
    result.push_back(Digits[byte & 0x0f]);
  }
  return result;
}

bool safeSymbol(StringRef value) {
  if (value.empty() || value.size() > 512)
    return false;
  for (char byte : value)
    if (!(std::isalnum(static_cast<unsigned char>(byte)) || byte == '_' ||
          byte == '.' || byte == '$' || byte == '@'))
      return false;
  return true;
}

bool exactFlag(StringRef value) {
  static constexpr StringRef Allowed[] = {
      "-static",
      "--static",
      "-Bstatic",
      "--build-id=none",
      "--no-dynamic-linker",
      "--fatal-warnings",
      "--no-undefined",
      "--gc-sections",
      "--eh-frame-hdr",
      "--hash-style=gnu",
      "--strip-debug",
      "--discard-all",
      "--discard-locals",
      "-O0",
      "-O1",
      "-O2",
      "-O3",
      "--no-undefined-version",
      "--no-allow-shlib-undefined",
      "--start-group",
      "--end-group",
      "-(",
      "-)",
      "--whole-archive",
      "--no-whole-archive",
      "--start-lib",
      "--end-lib",
  };
  for (StringRef candidate : Allowed)
    if (value == candidate)
      return true;
  return false;
}

bool safeZOption(StringRef value) {
  static constexpr StringRef Allowed[] = {
      "noexecstack",           "relro", "now",
      "separate-code",         "defs",  "max-page-size=4096",
      "common-page-size=4096",
  };
  for (StringRef candidate : Allowed)
    if (value == candidate)
      return true;
  return false;
}

struct FileSnapshot {
  dev_t Device = 0;
  ino_t Inode = 0;
  mode_t Mode = 0;
  uid_t Uid = 0;
  gid_t Gid = 0;
  nlink_t Links = 0;
  off_t Size = 0;
  timespec Modified{};
  timespec Changed{};
  int Seals = 0;
  std::array<uint8_t, 32> Sha256{};
};

bool sameSnapshot(const FileSnapshot &left, const FileSnapshot &right) {
  return left.Device == right.Device && left.Inode == right.Inode &&
         left.Mode == right.Mode && left.Uid == right.Uid &&
         left.Gid == right.Gid && left.Links == right.Links &&
         left.Size == right.Size &&
         left.Modified.tv_sec == right.Modified.tv_sec &&
         left.Modified.tv_nsec == right.Modified.tv_nsec &&
         left.Changed.tv_sec == right.Changed.tv_sec &&
         left.Changed.tv_nsec == right.Changed.tv_nsec &&
         left.Seals == right.Seals && left.Sha256 == right.Sha256;
}

bool isMemfd(int procFdDirectory, int fd) {
  const std::string descriptor = std::to_string(fd);
  std::array<char, 512> target{};
  const ssize_t length = ::readlinkat(procFdDirectory, descriptor.c_str(),
                                      target.data(), target.size());
  if (length <= 0 || static_cast<size_t>(length) == target.size())
    return false;
  const StringRef name(target.data(), static_cast<size_t>(length));
  return name.starts_with("/memfd:") && name.ends_with(" (deleted)");
}

bool hashFd(int fd, uint64_t length, std::array<uint8_t, 32> &digest,
            std::vector<uint8_t> *retainedBytes) {
  llvm::SHA256 hasher;
  std::array<uint8_t, 64 * 1024> buffer{};
  if (retainedBytes != nullptr) {
    if (length > static_cast<uint64_t>(std::numeric_limits<size_t>::max()))
      return false;
    retainedBytes->clear();
    retainedBytes->reserve(static_cast<size_t>(length));
  }
  uint64_t offset = 0;
  while (offset < length) {
    const size_t requested =
        static_cast<size_t>(std::min<uint64_t>(buffer.size(), length - offset));
    const ssize_t count =
        ::pread(fd, buffer.data(), requested, static_cast<off_t>(offset));
    if (count <= 0)
      return false;
    const size_t bytes = static_cast<size_t>(count);
    hasher.update(llvm::ArrayRef<uint8_t>(buffer.data(), bytes));
    if (retainedBytes != nullptr)
      retainedBytes->insert(retainedBytes->end(), buffer.begin(),
                            buffer.begin() + static_cast<ptrdiff_t>(bytes));
    offset += static_cast<uint64_t>(bytes);
  }
  uint8_t extra = 0;
  if (::pread(fd, &extra, 1, static_cast<off_t>(length)) != 0)
    return false;
  digest = hasher.final();
  return true;
}

bool snapshotFd(int fd, uint64_t maxBytes, FileSnapshot &snapshot,
                std::vector<uint8_t> *retainedBytes) {
  struct stat status{};
  if (::fstat(fd, &status) != 0 || !S_ISREG(status.st_mode) ||
      status.st_size < 0 || static_cast<uint64_t>(status.st_size) > maxBytes)
    return false;
  const int seals = ::fcntl(fd, F_GET_SEALS);
  if (seals < 0)
    return false;
  snapshot.Device = status.st_dev;
  snapshot.Inode = status.st_ino;
  snapshot.Mode = status.st_mode;
  snapshot.Uid = status.st_uid;
  snapshot.Gid = status.st_gid;
  snapshot.Links = status.st_nlink;
  snapshot.Size = status.st_size;
  snapshot.Modified = status.st_mtim;
  snapshot.Changed = status.st_ctim;
  snapshot.Seals = seals;
  return hashFd(fd, static_cast<uint64_t>(status.st_size), snapshot.Sha256,
                retainedBytes);
}

bool validateElf(StringRef bytes, InputKind expected, std::string &error) {
  const llvm::file_magic magic = llvm::identify_magic(bytes);
  const bool wantsRel = expected == InputKind::ElfRel;
  if (!wantsRel || magic != llvm::file_magic::elf_relocatable) {
    error = "input is not an ELF relocatable object";
    return false;
  }
  llvm::MemoryBufferRef memory(bytes, "sealed-fe2o3-input");
  auto objectOrError = llvm::object::ObjectFile::createObjectFile(memory);
  if (!objectOrError) {
    error =
        "malformed ELF object: " + llvm::toString(objectOrError.takeError());
    return false;
  }
  const auto *elf =
      llvm::dyn_cast<llvm::object::ELFObjectFileBase>(objectOrError->get());
  if (elf == nullptr || elf->getEMachine() != EM_X86_64 ||
      elf->getEType() != ET_REL ||
      objectOrError->get()->getArch() != llvm::Triple::x86_64) {
    error = "ELF input is not the required x86-64 type";
    return false;
  }
  for (const llvm::object::SectionRef &section :
       objectOrError->get()->sections()) {
    const llvm::object::ELFSectionRef elfSection(section);
    if (elfSection.getType() == llvm::ELF::SHT_LLVM_DEPENDENT_LIBRARIES) {
      error = "LLVM dependent-library sections are forbidden";
      return false;
    }
    auto nameOrError = section.getName();
    if (!nameOrError) {
      error = "malformed ELF section name: " +
              llvm::toString(nameOrError.takeError());
      return false;
    }
    const StringRef name = *nameOrError;
    const bool embeddedBitcode = name == ".llvmbc" || name == ".llvm.lto" ||
                                 name == ".llvm_bc" ||
                                 name.starts_with(".gnu.lto_");
    if (embeddedBitcode) {
      const uint64_t flags = elfSection.getFlags();
      constexpr uint64_t ActiveFlags = SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR;
      if (!wantsRel || (flags & SHF_EXCLUDE) == 0 ||
          (flags & ActiveFlags) != 0) {
        error = "active or non-relocatable embedded bitcode is forbidden";
        return false;
      }
    }
  }
  return true;
}

bool safeArchiveMemberName(StringRef name) {
  if (name.empty() || name.size() > 1024 || name == "." || name == "..")
    return false;
  for (char byte : name)
    if (!(std::isalnum(static_cast<unsigned char>(byte)) || byte == '.' ||
          byte == '_' || byte == '-' || byte == '$'))
      return false;
  return true;
}

bool validateArchive(StringRef bytes, InputKind expected,
                     uint64_t &cumulativeMemberCount, std::string &error) {
  if (llvm::identify_magic(bytes) != llvm::file_magic::archive) {
    error = "input is not a regular archive";
    return false;
  }
  llvm::MemoryBufferRef memory(bytes, "sealed-fe2o3-archive");
  llvm::Error archiveError = llvm::Error::success();
  llvm::object::Archive archive(memory, archiveError);
  if (archiveError) {
    error = "malformed archive: " + llvm::toString(std::move(archiveError));
    return false;
  }
  if (archive.isThin()) {
    error = "thin archives are forbidden";
    return false;
  }
  bool sawObject = false;
  bool sawRustMetadata = false;
  llvm::Error childError = llvm::Error::success();
  for (const llvm::object::Archive::Child &child :
       archive.children(childError, true)) {
    if (cumulativeMemberCount == MaxArchiveMembers) {
      error = "cumulative archive member count exceeds the fixed bound";
      return false;
    }
    ++cumulativeMemberCount;
    auto nameOrError = child.getName();
    if (!nameOrError) {
      error = "malformed archive member name: " +
              llvm::toString(nameOrError.takeError());
      return false;
    }
    const StringRef name = *nameOrError;
    if (!safeArchiveMemberName(name)) {
      error = "archive member names may not contain paths or control bytes";
      return false;
    }
    auto bufferOrError = child.getMemoryBufferRef();
    if (!bufferOrError) {
      error = "archive member is external or malformed: " +
              llvm::toString(bufferOrError.takeError());
      return false;
    }
    const StringRef memberBytes = bufferOrError->getBuffer();
    const llvm::file_magic memberMagic = llvm::identify_magic(memberBytes);
    if (memberMagic == llvm::file_magic::archive) {
      error = "nested and thin archive members are forbidden";
      return false;
    }
    if (memberMagic == llvm::file_magic::bitcode) {
      error = "bitcode and LTO archive members are forbidden";
      return false;
    }
    std::string memberError;
    if (!validateElf(memberBytes, InputKind::ElfRel, memberError)) {
      error = "archive member '" + name.str() + "' rejected: " + memberError;
      return false;
    }
    if (name == "lib.rmeta") {
      if (expected != InputKind::Rlib || sawRustMetadata) {
        error = "lib.rmeta is allowed exactly once and only in an rlib";
        return false;
      }
      // Rust metadata is accepted only as a parsed ET_REL object. It can never
      // enter LLD's linker-script content auto-detection path.
      sawRustMetadata = true;
    } else {
      sawObject = true;
    }
  }
  if (childError) {
    error = "malformed archive member sequence: " +
            llvm::toString(std::move(childError));
    return false;
  }
  if (!sawObject) {
    error = "archive contains no linkable ELF object member";
    return false;
  }
  return true;
}

struct InputRecord {
  int Fd = -1;
  InputKind Kind = InputKind::ElfRel;
  std::string Sha256;
  uint64_t Size = 0;
  mode_t Mode = 0;
  FileSnapshot Before;
  std::string LldPath;
  std::string CanonicalRecord;
};

bool parseInputKind(StringRef value, InputKind &kind) {
  if (value == "elf-rel")
    kind = InputKind::ElfRel;
  else if (value == "archive")
    kind = InputKind::Archive;
  else if (value == "rlib")
    kind = InputKind::Rlib;
  else
    return false;
  return true;
}

bool takeField(StringRef &record, StringRef &field) {
  const size_t separator = record.find(':');
  if (separator == StringRef::npos)
    return false;
  field = record.take_front(separator);
  record = record.drop_front(separator + 1);
  return true;
}

bool parseInputRecord(StringRef argument, InputRecord &input,
                      std::string &error) {
  if (!argument.starts_with(InputPrefix))
    return false;
  StringRef record = argument.drop_front(std::strlen(InputPrefix));
  StringRef fdText;
  StringRef kindText;
  StringRef hashText;
  StringRef sizeText;
  if (!takeField(record, fdText) || !takeField(record, kindText) ||
      !takeField(record, hashText) || !takeField(record, sizeText) ||
      record.contains(':')) {
    error = "input record must contain exactly fd:kind:sha256:size:mode";
    return false;
  }
  uint64_t fd = 0;
  if (!parseCanonicalUnsigned(fdText, fd) || fd < FirstInputFd ||
      fd > static_cast<uint64_t>(INT_MAX) ||
      !parseInputKind(kindText, input.Kind) || !canonicalSha256(hashText) ||
      !parseCanonicalUnsigned(sizeText, input.Size) ||
      input.Size > MaxInputBytes || !parseCanonicalMode(record, input.Mode)) {
    error = "input record contains a noncanonical or unsupported field";
    return false;
  }
  input.Fd = static_cast<int>(fd);
  input.Sha256 = hashText.str();
  input.LldPath = "/proc/self/fd/" + std::to_string(input.Fd);
  input.CanonicalRecord = argument.str();
  return true;
}

struct RequestIdentity {
  std::string Plan;
  std::string Closure;
  std::string Nonce;
};

bool parseRequest(StringRef argument, RequestIdentity &request) {
  if (!argument.starts_with(RequestPrefix))
    return false;
  StringRef record = argument.drop_front(std::strlen(RequestPrefix));
  StringRef plan;
  StringRef closure;
  if (!takeField(record, plan) || !takeField(record, closure) ||
      record.contains(':') || !canonicalSha256(plan) ||
      !canonicalSha256(closure) || !canonicalSha256(record))
    return false;
  request.Plan = plan.str();
  request.Closure = closure.str();
  request.Nonce = record.str();
  return true;
}

struct ResultSocketIdentity {
  uint64_t Device = 0;
  uint64_t Inode = 0;
};

bool parseResultSocket(StringRef argument, ResultSocketIdentity &identity) {
  if (!argument.starts_with(ResultSocketPrefix))
    return false;
  StringRef record = argument.drop_front(std::strlen(ResultSocketPrefix));
  StringRef fdText;
  StringRef deviceText;
  if (!takeField(record, fdText) || !takeField(record, deviceText) ||
      record.contains(':'))
    return false;
  uint64_t fd = 0;
  return parseCanonicalUnsigned(fdText, fd) &&
         fd == static_cast<uint64_t>(ResultSocketFd) &&
         parseCanonicalUnsigned(deviceText, identity.Device) &&
         parseCanonicalUnsigned(record, identity.Inode);
}

bool validateResultSocket(const ResultSocketIdentity &identity,
                          std::string &error) {
  const int descriptorFlags = ::fcntl(ResultSocketFd, F_GETFD);
  int statusFlags = ::fcntl(ResultSocketFd, F_GETFL);
  struct stat status{};
  if (descriptorFlags < 0 || (descriptorFlags & FD_CLOEXEC) != 0 ||
      statusFlags < 0 || ::fstat(ResultSocketFd, &status) != 0 ||
      !S_ISSOCK(status.st_mode) ||
      static_cast<uint64_t>(status.st_dev) != identity.Device ||
      static_cast<uint64_t>(status.st_ino) != identity.Inode) {
    error = "result socket descriptor identity is invalid";
    return false;
  }
  if ((statusFlags & O_NONBLOCK) == 0) {
    if (::fcntl(ResultSocketFd, F_SETFL, statusFlags | O_NONBLOCK) != 0) {
      error = "result socket could not be made nonblocking";
      return false;
    }
    statusFlags = ::fcntl(ResultSocketFd, F_GETFL);
    struct stat after{};
    if (statusFlags < 0 || (statusFlags & O_NONBLOCK) == 0 ||
        ::fstat(ResultSocketFd, &after) != 0 || after.st_dev != status.st_dev ||
        after.st_ino != status.st_ino || after.st_mode != status.st_mode) {
      error = "result socket did not retain identity and nonblocking mode";
      return false;
    }
  }
  for (int fd = STDIN_FILENO; fd <= STDERR_FILENO; ++fd) {
    if (!isNullDevice(fd)) {
      error = "standard descriptors are not isolated from the result socket";
      return false;
    }
  }
  int type = 0;
  int domain = 0;
  int listening = 0;
  socklen_t optionLength = sizeof(int);
  if (::getsockopt(ResultSocketFd, SOL_SOCKET, SO_TYPE, &type, &optionLength) !=
          0 ||
      optionLength != sizeof(int) || type != SOCK_SEQPACKET) {
    error = "result descriptor is not SOCK_SEQPACKET";
    return false;
  }
  optionLength = sizeof(int);
  if (::getsockopt(ResultSocketFd, SOL_SOCKET, SO_DOMAIN, &domain,
                   &optionLength) != 0 ||
      optionLength != sizeof(int) || domain != AF_UNIX) {
    error = "result descriptor is not AF_UNIX";
    return false;
  }
  optionLength = sizeof(int);
  if (::getsockopt(ResultSocketFd, SOL_SOCKET, SO_ACCEPTCONN, &listening,
                   &optionLength) != 0 ||
      optionLength != sizeof(int) || listening != 0) {
    error = "result descriptor must be a connected socketpair endpoint";
    return false;
  }
  sockaddr_un local{};
  sockaddr_un peer{};
  socklen_t localLength = sizeof(local);
  socklen_t peerLength = sizeof(peer);
  if (::getsockname(ResultSocketFd, reinterpret_cast<sockaddr *>(&local),
                    &localLength) != 0 ||
      ::getpeername(ResultSocketFd, reinterpret_cast<sockaddr *>(&peer),
                    &peerLength) != 0 ||
      local.sun_family != AF_UNIX || peer.sun_family != AF_UNIX) {
    error = "result socket endpoint or peer is invalid";
    return false;
  }
  ucred credentials{};
  socklen_t credentialsLength = sizeof(credentials);
  if (::getsockopt(ResultSocketFd, SOL_SOCKET, SO_PEERCRED, &credentials,
                   &credentialsLength) != 0 ||
      credentialsLength != sizeof(credentials) ||
      credentials.pid != ::getppid() || credentials.uid != ::geteuid() ||
      credentials.gid != ::getegid()) {
    error = "result socket peer credentials do not match the direct parent";
    return false;
  }
  uint8_t pending = 0;
  const ssize_t pendingBytes = ::recv(ResultSocketFd, &pending, sizeof(pending),
                                      MSG_PEEK | MSG_DONTWAIT);
  if (pendingBytes != -1 || (errno != EAGAIN && errno != EWOULDBLOCK)) {
    error = "result socket must be open and have an empty receive queue";
    return false;
  }
  return true;
}

bool onlyExpectedDescriptors(const std::vector<InputRecord> &inputs,
                             const ProcessBoundary &boundary,
                             std::string &error) {
  const int listing = ::fcntl(boundary.ProcFdDirectory, F_DUPFD_CLOEXEC, 3);
  if (listing < 0) {
    error = "could not enumerate inherited descriptors";
    return false;
  }
  DIR *directory = ::fdopendir(listing);
  if (directory == nullptr) {
    ::close(listing);
    error = "could not enumerate inherited descriptors";
    return false;
  }
  bool valid = true;
  errno = 0;
  while (dirent *entry = ::readdir(directory)) {
    StringRef name(entry->d_name);
    uint64_t fd = 0;
    if (name == "." || name == "..")
      continue;
    if (!parseCanonicalUnsigned(name, fd)) {
      valid = false;
      break;
    }
    if (fd <= 2 || fd == static_cast<uint64_t>(listing) ||
        fd == static_cast<uint64_t>(ResultSocketFd) ||
        fd == static_cast<uint64_t>(boundary.ProcFdDirectory) ||
        fd == static_cast<uint64_t>(boundary.MountNamespace))
      continue;
    const bool expected = std::any_of(
        inputs.begin(), inputs.end(), [fd](const InputRecord &input) {
          return static_cast<uint64_t>(input.Fd) == fd;
        });
    if (!expected) {
      valid = false;
      break;
    }
  }
  if (errno != 0)
    valid = false;
  if (::closedir(directory) != 0)
    valid = false;
  if (!valid)
    error = "unexpected inherited descriptor is open";
  return valid;
}

bool validateInput(InputRecord &input, int procFdDirectory,
                   uint64_t &archiveMemberCount, std::string &error) {
  const int descriptorFlags = ::fcntl(input.Fd, F_GETFD);
  if (descriptorFlags < 0 || (descriptorFlags & FD_CLOEXEC) != 0) {
    error = "input descriptor is missing or close-on-exec";
    return false;
  }
  std::vector<uint8_t> bytes;
  if (!snapshotFd(input.Fd, MaxInputBytes, input.Before, &bytes) ||
      !isMemfd(procFdDirectory, input.Fd) || input.Before.Links != 0 ||
      input.Before.Uid != ::geteuid() ||
      (input.Before.Mode & 07777) != input.Mode ||
      input.Before.Seals != static_cast<int>(InputSeals) ||
      static_cast<uint64_t>(input.Before.Size) != input.Size ||
      lowercaseHex(input.Before.Sha256) != input.Sha256) {
    error = "input descriptor identity, seals, mode, size, or digest mismatch";
    return false;
  }
  const StringRef content(reinterpret_cast<const char *>(bytes.data()),
                          bytes.size());
  if (input.Kind == InputKind::ElfRel)
    return validateElf(content, input.Kind, error);
  return validateArchive(content, input.Kind, archiveMemberCount, error);
}

bool revalidateInputs(const std::vector<InputRecord> &inputs,
                      std::string &error) {
  for (const InputRecord &input : inputs) {
    FileSnapshot after;
    if (!snapshotFd(input.Fd, MaxInputBytes, after, nullptr) ||
        !sameSnapshot(input.Before, after)) {
      error = "input descriptor changed while LLD was running";
      return false;
    }
  }
  return true;
}

bool procReopenMatches(int procFdDirectory, int fd, int accessMode,
                       std::string &error) {
  struct stat original{};
  if (::fstat(fd, &original) != 0) {
    error = "could not measure a synthesized descriptor path";
    return false;
  }
  const int originalSeals = ::fcntl(fd, F_GET_SEALS);
  const std::string descriptor = std::to_string(fd);
  const int reopened =
      ::openat(procFdDirectory, descriptor.c_str(), accessMode | O_CLOEXEC);
  struct stat observed{};
  const int observedSeals = reopened >= 0 ? ::fcntl(reopened, F_GET_SEALS) : -1;
  const bool valid = reopened >= 0 && ::fstat(reopened, &observed) == 0 &&
                     original.st_dev == observed.st_dev &&
                     original.st_ino == observed.st_ino &&
                     original.st_mode == observed.st_mode &&
                     original.st_uid == observed.st_uid &&
                     original.st_gid == observed.st_gid &&
                     original.st_nlink == observed.st_nlink &&
                     original.st_size == observed.st_size &&
                     original.st_mtim.tv_sec == observed.st_mtim.tv_sec &&
                     original.st_mtim.tv_nsec == observed.st_mtim.tv_nsec &&
                     original.st_ctim.tv_sec == observed.st_ctim.tv_sec &&
                     original.st_ctim.tv_nsec == observed.st_ctim.tv_nsec &&
                     originalSeals >= 0 && originalSeals == observedSeals;
  if (reopened >= 0)
    (void)::close(reopened);
  if (!valid)
    error = "a synthesized proc-fd path did not reopen the exact descriptor";
  return valid;
}

bool verifyLldDescriptorPaths(const ProcessBoundary &boundary,
                              const std::vector<InputRecord> &inputs,
                              int outputFd, std::string &error) {
  if (!verifyProcessBoundary(boundary, error))
    return false;
  for (const InputRecord &input : inputs)
    if (!procReopenMatches(boundary.ProcFdDirectory, input.Fd, O_RDONLY, error))
      return false;
  return procReopenMatches(boundary.ProcFdDirectory, outputFd, O_RDWR, error);
}

struct ParsedArguments {
  std::vector<std::string> Owned;
  std::vector<const char *> Pointers;
  std::vector<InputRecord> Inputs;
  RequestIdentity Request;
  ResultSocketIdentity ResultSocket;
};

bool takeValue(int argc, char **argv, int &index, StringRef option,
               StringRef &value, std::string &error) {
  if (index + 1 >= argc) {
    error = option.str() + " requires a value";
    return false;
  }
  value = argv[++index];
  return true;
}

bool parseArguments(int argc, char **argv, const ProcessBoundary &boundary,
                    ParsedArguments &parsed, std::string &error) {
  if (argc < 5 || StringRef(argv[1]) != ProtocolArgument) {
    error = "the first argument must be --fe2o3-host-lld-elf-v2";
    return false;
  }
  if (static_cast<size_t>(argc) > MaxArgumentCount) {
    error = "argument count exceeds the fixed bound";
    return false;
  }
  size_t totalBytes = 0;
  for (int index = 1; index < argc; ++index) {
    const StringRef argument(argv[index]);
    if (argument.empty() || argument.size() > MaxArgumentBytes ||
        argument.size() > MaxTotalArgumentBytes - totalBytes) {
      error = "argument bytes exceed the fixed bound";
      return false;
    }
    totalBytes += argument.size();
    for (char byte : argument)
      if (static_cast<unsigned char>(byte) < 0x20 || byte == 0x7f) {
        error = "arguments must contain printable canonical bytes";
        return false;
      }
  }

  parsed.Owned.emplace_back("ld.lld");
  parsed.Owned.emplace_back("-static");
  parsed.Owned.emplace_back("--no-dynamic-linker");
  parsed.Owned.emplace_back("--build-id=none");
  parsed.Owned.emplace_back("--fatal-warnings");
  parsed.Owned.emplace_back("--no-undefined");
  parsed.Owned.emplace_back("--no-dependent-libraries");
  parsed.Owned.emplace_back("-z");
  parsed.Owned.emplace_back("noexecstack");
  bool sawRequest = false;
  bool sawResultSocket = false;
  uint64_t totalInputBytes = 0;
  for (int index = 2; index < argc; ++index) {
    const StringRef argument(argv[index]);
    if (argument.starts_with("@")) {
      error = "response files are forbidden; the closure must expand them";
      return false;
    }
    if (argument.starts_with(InputPrefix)) {
      InputRecord input;
      if (!parseInputRecord(argument, input, error))
        return false;
      const auto duplicate =
          std::find_if(parsed.Inputs.begin(), parsed.Inputs.end(),
                       [&input](const InputRecord &candidate) {
                         return candidate.Fd == input.Fd;
                       });
      if (duplicate != parsed.Inputs.end()) {
        if (duplicate->CanonicalRecord != input.CanonicalRecord) {
          error = "repeated input descriptor metadata must be byte-identical";
          return false;
        }
        parsed.Owned.emplace_back(duplicate->LldPath);
        continue;
      }
      if (parsed.Inputs.size() == MaxUniqueInputCount) {
        error = "unique input count exceeds the fixed bound";
        return false;
      }
      if (input.Size > MaxTotalInputBytes - totalInputBytes) {
        error = "total unique input size exceeds the fixed bound";
        return false;
      }
      totalInputBytes += input.Size;
      parsed.Owned.emplace_back(input.LldPath);
      parsed.Inputs.push_back(std::move(input));
      continue;
    }
    if (argument.starts_with(ResultSocketPrefix)) {
      if (sawResultSocket ||
          !parseResultSocket(argument, parsed.ResultSocket)) {
        error = "result socket control is duplicate or noncanonical";
        return false;
      }
      sawResultSocket = true;
      continue;
    }
    if (argument.starts_with(RequestPrefix)) {
      if (sawRequest || !parseRequest(argument, parsed.Request)) {
        error = "request control is duplicate or noncanonical";
        return false;
      }
      sawRequest = true;
      continue;
    }
    if (argument.starts_with("--fe2o3-")) {
      error = "unknown fe2o3 protocol control";
      return false;
    }
    if (argument == "-flavor") {
      StringRef value;
      if (!takeValue(argc, argv, index, argument, value, error) ||
          value != "gnu") {
        error = "only the GNU ELF flavor is supported";
        return false;
      }
      continue;
    }
    if (argument.starts_with("--flavor=")) {
      if (argument != "--flavor=gnu") {
        error = "only the GNU ELF flavor is supported";
        return false;
      }
      continue;
    }
    if (argument == "-m") {
      StringRef value;
      if (!takeValue(argc, argv, index, argument, value, error) ||
          value != "elf_x86_64") {
        error = "only the elf_x86_64 emulation is supported";
        return false;
      }
      parsed.Owned.emplace_back("-m");
      parsed.Owned.emplace_back("elf_x86_64");
      continue;
    }
    if (argument == "-z") {
      StringRef value;
      if (!takeValue(argc, argv, index, argument, value, error) ||
          !safeZOption(value)) {
        error = "unsupported -z policy";
        return false;
      }
      parsed.Owned.emplace_back("-z");
      parsed.Owned.emplace_back(value.str());
      continue;
    }
    if (argument == "-u") {
      StringRef value;
      if (!takeValue(argc, argv, index, argument, value, error) ||
          !safeSymbol(value)) {
        error = "invalid undefined-symbol request";
        return false;
      }
      parsed.Owned.emplace_back("-u");
      parsed.Owned.emplace_back(value.str());
      continue;
    }
    if (argument.starts_with("--undefined=")) {
      if (!safeSymbol(argument.drop_front(std::strlen("--undefined=")))) {
        error = "invalid undefined-symbol request";
        return false;
      }
      parsed.Owned.emplace_back(argument.str());
      continue;
    }
    if (argument == "--dependent-libraries" ||
        argument == "--no-dependent-libraries" ||
        argument.starts_with("--dependent-libraries=") ||
        argument.starts_with("--no-dependent-libraries=")) {
      error = "dependent-library processing policy is internal";
      return false;
    }
    if (exactFlag(argument)) {
      parsed.Owned.emplace_back(argument.str());
      continue;
    }
    if (argument == "-o" || argument.starts_with("--output") ||
        argument == "--mmap-output-file" ||
        argument == "--no-mmap-output-file" ||
        argument.starts_with("--threads") || argument.starts_with("-l") ||
        argument.starts_with("-L") || argument.starts_with("--library") ||
        argument.starts_with("--library-path")) {
      error = "output policy, linker threads, and library search are internal";
      return false;
    }
    if (argument.starts_with("--plugin") || argument.starts_with("--lto") ||
        argument.starts_with("--thinlto") ||
        argument.starts_with("--reproduce") ||
        argument.starts_with("--dependency-file") ||
        argument.starts_with("--sysroot") ||
        argument.starts_with("--version-script") ||
        argument.starts_with("--dynamic-list") ||
        argument.starts_with("--script") || argument == "-T" ||
        argument.starts_with("-Wl,")) {
      error = "external configuration, scripts, and plugins are forbidden";
      return false;
    }
    error = "unsupported linker argument or bare input descriptor";
    return false;
  }
  if (parsed.Inputs.empty() || !sawRequest || !sawResultSocket) {
    error = "typed inputs, request identity, and result socket are required";
    return false;
  }
  if (!validateResultSocket(parsed.ResultSocket, error) ||
      !onlyExpectedDescriptors(parsed.Inputs, boundary, error))
    return false;
  uint64_t archiveMemberCount = 0;
  for (InputRecord &input : parsed.Inputs)
    if (!validateInput(input, boundary.ProcFdDirectory, archiveMemberCount,
                       error))
      return false;
  return true;
}

int createOutputMemfd() {
  const int fd =
      static_cast<int>(::syscall(SYS_memfd_create, OutputMemfdName.data(),
                                 MFD_CLOEXEC | MFD_ALLOW_SEALING));
  if (fd < 0 || ::fchmod(fd, S_IRUSR | S_IWUSR) != 0) {
    if (fd >= 0)
      ::close(fd);
    return -1;
  }
  return fd;
}

bool validateOutputElf(StringRef bytes, std::string &error) {
  if (bytes.size() < sizeof(Elf64_Ehdr)) {
    error = "LLD output is shorter than an ELF header";
    return false;
  }
  Elf64_Ehdr header{};
  std::memcpy(&header, bytes.data(), sizeof(header));
  if (std::memcmp(header.e_ident, ELFMAG, SELFMAG) != 0 ||
      header.e_ident[EI_CLASS] != ELFCLASS64 ||
      header.e_ident[EI_DATA] != ELFDATA2LSB ||
      header.e_ident[EI_VERSION] != EV_CURRENT ||
      header.e_version != EV_CURRENT || header.e_machine != EM_X86_64 ||
      header.e_type != ET_EXEC || header.e_phentsize != sizeof(Elf64_Phdr) ||
      header.e_phnum == PN_XNUM || header.e_phoff > bytes.size() ||
      static_cast<uint64_t>(header.e_phnum) >
          (bytes.size() - static_cast<size_t>(header.e_phoff)) /
              sizeof(Elf64_Phdr)) {
    error = "LLD output is not a canonical x86-64 ET_EXEC";
    return false;
  }
  for (uint16_t index = 0; index < header.e_phnum; ++index) {
    Elf64_Phdr program{};
    const size_t offset = static_cast<size_t>(header.e_phoff) +
                          static_cast<size_t>(index) * sizeof(program);
    std::memcpy(&program, bytes.data() + offset, sizeof(program));
    if (program.p_type == PT_INTERP || program.p_type == PT_DYNAMIC) {
      error = "static ET_EXEC output contains a runtime-loader segment";
      return false;
    }
    if (program.p_type == PT_LOAD &&
        (program.p_flags & (PF_W | PF_X)) == (PF_W | PF_X)) {
      error = "static ET_EXEC output contains a writable executable segment";
      return false;
    }
    if (program.p_type == PT_GNU_STACK && (program.p_flags & PF_X) != 0) {
      error = "static ET_EXEC output requests an executable stack";
      return false;
    }
  }
  llvm::MemoryBufferRef memory(bytes, "private-fe2o3-output");
  auto objectOrError = llvm::object::ObjectFile::createObjectFile(memory);
  if (!objectOrError) {
    error =
        "malformed output ELF: " + llvm::toString(objectOrError.takeError());
    return false;
  }
  const auto *elf =
      llvm::dyn_cast<llvm::object::ELFObjectFileBase>(objectOrError->get());
  return elf != nullptr && elf->getEMachine() == EM_X86_64 &&
         elf->getEType() == ET_EXEC &&
         objectOrError->get()->getArch() == llvm::Triple::x86_64;
}

bool sealAndMeasureOutput(int fd, int procFdDirectory,
                          FileSnapshot &measurement, std::string &error) {
  FileSnapshot unsealed;
  std::vector<uint8_t> bytes;
  if (!snapshotFd(fd, MaxOutputBytes, unsealed, &bytes) ||
      !isMemfd(procFdDirectory, fd) || unsealed.Links != 0 ||
      unsealed.Uid != ::geteuid() || (unsealed.Mode & 07777) != 0600 ||
      unsealed.Size < static_cast<off_t>(sizeof(Elf64_Ehdr)) ||
      unsealed.Seals != 0 ||
      !validateOutputElf(
          StringRef(reinterpret_cast<const char *>(bytes.data()), bytes.size()),
          error)) {
    if (error.empty())
      error = "unsealed output identity or content is invalid";
    return false;
  }
  if (::fchmod(fd, static_cast<mode_t>(OutputMode)) != 0 ||
      ::fcntl(fd, F_ADD_SEALS, static_cast<int>(InputSeals)) != 0) {
    error = "could not canonicalize and seal output memfd";
    return false;
  }
  if (!snapshotFd(fd, MaxOutputBytes, measurement, nullptr) ||
      measurement.Links != 0 || measurement.Uid != ::geteuid() ||
      measurement.Device != unsealed.Device ||
      measurement.Inode != unsealed.Inode ||
      (measurement.Mode & 07777) != static_cast<mode_t>(OutputMode) ||
      measurement.Seals != static_cast<int>(InputSeals) ||
      measurement.Size != unsealed.Size ||
      measurement.Sha256 != unsealed.Sha256) {
    error = "sealed output failed exact post-seal revalidation";
    return false;
  }
  return true;
}

bool sendResult(int outputFd, const RequestIdentity &request,
                const FileSnapshot &measurement, std::string &error) {
  const std::string record =
      std::string(ResultRecordPrefix) + "\tplan=" + request.Plan +
      "\tclosure=" + request.Closure + "\tnonce=" + request.Nonce +
      "\tsha256=" + lowercaseHex(measurement.Sha256) +
      "\tlength=" + std::to_string(measurement.Size) +
      "\tcopy=" + ResultCopyPolicy + "\n";
  if (record.size() > 512) {
    error = "canonical result record exceeds the fixed bound";
    return false;
  }
  iovec vector{const_cast<char *>(record.data()), record.size()};
  std::array<unsigned char, CMSG_SPACE(sizeof(int))> control{};
  msghdr message{};
  message.msg_iov = &vector;
  message.msg_iovlen = 1;
  message.msg_control = control.data();
  message.msg_controllen = control.size();
  cmsghdr *header = CMSG_FIRSTHDR(&message);
  if (header == nullptr) {
    error = "could not construct result capability message";
    return false;
  }
  header->cmsg_level = SOL_SOCKET;
  header->cmsg_type = SCM_RIGHTS;
  header->cmsg_len = CMSG_LEN(sizeof(int));
  std::memcpy(CMSG_DATA(header), &outputFd, sizeof(outputFd));
  message.msg_controllen = control.size();
  const ssize_t sent =
      ::sendmsg(ResultSocketFd, &message, MSG_DONTWAIT | MSG_NOSIGNAL);
  if (sent != static_cast<ssize_t>(record.size())) {
    error = "sealed result capability transfer failed";
    return false;
  }
  // A successful sequenced-packet send is the result commit point. Do not
  // turn a peer-close race during the subsequent half-close into a reported
  // failure after the capability has already been transferred.
  (void)::shutdown(ResultSocketFd, SHUT_WR);
  return true;
}

void printIdentity() {
  llvm::outs() << "format=fe2o3-host-lld-identity-v1\n"
               << "authority=none\n"
               << "flavor=gnu-elf\n"
               << "protocol=fe2o3-host-lld-elf-v2\n"
               << "input_protocol=fe2o3-input-v1\n"
               << "result_protocol=fe2o3-host-lld-result-v1\n"
               << "result_socket_fd=91\n"
               << "output_staging=tool-owned-sealed-memfd-v1\n"
               << "result_copy=" << ResultCopyPolicy << '\n'
               << "max_argument_count=4096\n"
               << "max_argument_bytes=4096\n"
               << "max_total_argument_bytes=1048576\n"
               << "max_input_count=2048\n"
               << "max_input_bytes=268435456\n"
               << "max_total_input_bytes=2147483648\n"
               << "max_output_bytes=536870912\n"
               << "max_address_space_bytes=4294967296\n"
               << "max_archive_members=262144\n"
               << "max_cpu_seconds=60\n"
               << "dependent_libraries=forbidden\n"
               << "signal_state=linux-x86_64-kernel-1-64-main-v2\n"
               << "llvm_version=" FE2O3_HOST_LLD_LLVM_VERSION "\n"
               << "llvm_build_identity=" FE2O3_HOST_LLD_LLVM_BUILD_ID "\n"
               << "llvm_source_commit=" FE2O3_HOST_LLD_LLVM_SOURCE_COMMIT "\n"
               << "llvm_source_tree=" FE2O3_HOST_LLD_LLVM_SOURCE_TREE "\n"
               << "elf_class=ELF64\n"
               << "elf_machine=Advanced Micro Devices X86-64\n";
}

} // namespace

int main(int argc, char **argv) {
  const bool identityRequest =
      argc == 2 && StringRef(argv[1]) == "--fe2o3-identity-v1";
  if (!identityRequest && !canonicalizeStandardDescriptors())
    _exit(static_cast<int>(ExitCode::Internal));
  const int signalStatus = normalizeInheritedSignalState();
  if (signalStatus != static_cast<int>(ExitCode::Success))
    return signalStatus;
  if (::prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0)
    return fail(ExitCode::Internal, "could not disable process dumpability");

  const int resourceStatus = establishResourceBounds();
  if (resourceStatus != static_cast<int>(ExitCode::Success))
    return resourceStatus;

  const int environmentStatus = sanitizeEnvironment();
  if (environmentStatus != static_cast<int>(ExitCode::Success))
    return environmentStatus;

  if (identityRequest) {
    printIdentity();
    return static_cast<int>(ExitCode::Success);
  }

  ProcessBoundary boundary;
  std::string boundaryError;
  if (!openProcessBoundary(boundary, boundaryError))
    return fail(ExitCode::Environment, boundaryError);
  const int privilegeStatus = establishPrivilegeBoundary();
  if (privilegeStatus != static_cast<int>(ExitCode::Success))
    return privilegeStatus;

  ParsedArguments parsed;
  std::string parseError;
  if (!parseArguments(argc, argv, boundary, parsed, parseError))
    return fail(ExitCode::Usage, parseError);

  const int outputFd = createOutputMemfd();
  if (outputFd < 0)
    return fail(ExitCode::Internal, "could not create private output memfd");
  parsed.Owned.emplace_back("--threads=1");
  parsed.Owned.emplace_back("--mmap-output-file");
  parsed.Owned.emplace_back("-o");
  parsed.Owned.emplace_back("/proc/self/fd/" + std::to_string(outputFd));
  if (!verifyLldDescriptorPaths(boundary, parsed.Inputs, outputFd,
                                parseError)) {
    ::close(outputFd);
    return fail(ExitCode::Environment, parseError);
  }
  parsed.Pointers.reserve(parsed.Owned.size());
  for (const std::string &argument : parsed.Owned)
    parsed.Pointers.push_back(argument.c_str());

  BoundedRawStream output(MaxDiagnosticBytes);
  BoundedRawStream error(MaxDiagnosticBytes);
  const lld::Result result = lld::lldMain(parsed.Pointers, output, error,
                                          {{lld::Gnu, &lld::elf::link}});
  output.flush();
  error.flush();
  llvm::outs() << output.str();
  llvm::errs() << error.str();
  if (output.truncated())
    llvm::outs() << "fe2o3-host-lld: stdout diagnostics truncated\n";
  if (error.truncated())
    llvm::errs() << "fe2o3-host-lld: stderr diagnostics truncated\n";
  if (!result.canRunAgain) {
    ::close(outputFd);
    immediateExit(ExitCode::Internal);
  }
  if (result.retCode != 0) {
    ::close(outputFd);
    return static_cast<int>(ExitCode::LinkFailure);
  }

  std::string validationError;
  if (!verifyProcessBoundary(boundary, validationError) ||
      !revalidateInputs(parsed.Inputs, validationError)) {
    ::close(outputFd);
    return fail(ExitCode::Internal, validationError);
  }
  FileSnapshot measurement;
  if (!sealAndMeasureOutput(outputFd, boundary.ProcFdDirectory, measurement,
                            validationError) ||
      !sendResult(outputFd, parsed.Request, measurement, validationError)) {
    ::close(outputFd);
    return fail(ExitCode::Internal, validationError);
  }
  (void)::close(outputFd);
  return static_cast<int>(ExitCode::Success);
}
