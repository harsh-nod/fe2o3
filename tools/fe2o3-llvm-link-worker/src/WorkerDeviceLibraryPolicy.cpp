#include "WorkerDeviceLibraryPolicy.h"
#include "WorkerBuildConfig.h"

#include "llvm/ADT/STLExtras.h"
#include "llvm/Support/SHA256.h"

#include <cerrno>
#include <climits>
#include <cstring>
#include <fcntl.h>
#include <limits>
#include <optional>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>

using namespace llvm;

namespace fe2o3::worker {
namespace {

constexpr std::array<StringLiteral, 7> SupportedOcmlImports = {
    "__ocml_cos_f32",  "__ocml_exp2_f32", "__ocml_exp_f32", "__ocml_log10_f32",
    "__ocml_log2_f32", "__ocml_log_f32",  "__ocml_sin_f32",
};

constexpr std::array<StringLiteral, 4> RequiredProviderFiles = {
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
};

Error policyError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(), Message);
}

class FileDescriptor {
public:
  explicit FileDescriptor(int Value = -1) : Value(Value) {}
  FileDescriptor(const FileDescriptor &) = delete;
  FileDescriptor &operator=(const FileDescriptor &) = delete;
  FileDescriptor(FileDescriptor &&Other) noexcept
      : Value(std::exchange(Other.Value, -1)) {}
  ~FileDescriptor() {
    if (Value >= 0)
      while (::close(Value) < 0 && errno == EINTR) {
      }
  }

  int get() const { return Value; }

private:
  int Value;
};

Expected<std::array<uint8_t, 32>> parseDigest(StringRef Hex) {
  if (Hex.size() != 64)
    return policyError("measured device-library digest has the wrong length");
  std::array<uint8_t, 32> Result{};
  for (size_t I = 0; I < Result.size(); ++I) {
    auto Nibble = [](char Value) -> std::optional<uint8_t> {
      if (Value >= '0' && Value <= '9')
        return static_cast<uint8_t>(Value - '0');
      if (Value >= 'a' && Value <= 'f')
        return static_cast<uint8_t>(Value - 'a' + 10);
      return std::nullopt;
    };
    std::optional<uint8_t> High = Nibble(Hex[I * 2]);
    std::optional<uint8_t> Low = Nibble(Hex[I * 2 + 1]);
    if (!High || !Low)
      return policyError("measured device-library digest is noncanonical");
    Result[I] = static_cast<uint8_t>((*High << 4) | *Low);
  }
  return Result;
}

Error validatePolicy(const Gfx942DeviceLibraryPolicy &Policy) {
  if (Policy.Directory.empty() || Policy.Directory.size() > PATH_MAX)
    return policyError("invalid gfx942 device-library directory");
  if (Policy.Files.size() != RequiredProviderFiles.size())
    return policyError("gfx942 device-library policy has the wrong file set");
  for (size_t I = 0; I < Policy.Files.size(); ++I) {
    const PinnedDeviceLibraryFile &File = Policy.Files[I];
    if (File.Basename != RequiredProviderFiles[I])
      return policyError("gfx942 device-library policy is noncanonical");
    if (File.MaxBytes == 0 || File.MaxBytes > MaxDeviceLibraryFileBytes)
      return policyError("gfx942 device-library file bound is invalid");
    if (llvm::all_of(File.Digest, [](uint8_t Byte) { return Byte == 0; }))
      return policyError("gfx942 device-library digest is zero");
  }
  return Error::success();
}

bool sameFileState(const struct stat &Before, const struct stat &After) {
  return Before.st_dev == After.st_dev && Before.st_ino == After.st_ino &&
         Before.st_mode == After.st_mode && Before.st_nlink == After.st_nlink &&
         Before.st_size == After.st_size &&
         Before.st_mtim.tv_sec == After.st_mtim.tv_sec &&
         Before.st_mtim.tv_nsec == After.st_mtim.tv_nsec &&
         Before.st_ctim.tv_sec == After.st_ctim.tv_sec &&
         Before.st_ctim.tv_nsec == After.st_ctim.tv_nsec;
}

Expected<Input> readPinnedFile(int Directory,
                               const PinnedDeviceLibraryFile &Pin) {
  FileDescriptor File(::openat(Directory, Pin.Basename.c_str(),
                               O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK));
  if (File.get() < 0)
    return policyError(Twine("cannot open pinned gfx942 device library '") +
                       Pin.Basename + "': " + std::strerror(errno));

  struct stat Before{};
  if (::fstat(File.get(), &Before) < 0)
    return policyError(Twine("cannot inspect pinned gfx942 device library '") +
                       Pin.Basename + "': " + std::strerror(errno));
  if (!S_ISREG(Before.st_mode) || Before.st_nlink != 1 || Before.st_size <= 0 ||
      static_cast<uint64_t>(Before.st_size) > Pin.MaxBytes)
    return policyError(Twine("pinned gfx942 device library '") + Pin.Basename +
                       "' violates its file contract");

  size_t Size = static_cast<size_t>(Before.st_size);
  std::vector<uint8_t> Bytes(Size);
  size_t Offset = 0;
  while (Offset != Bytes.size()) {
    ssize_t Read =
        ::read(File.get(), Bytes.data() + Offset,
               std::min(Bytes.size() - Offset, static_cast<size_t>(SSIZE_MAX)));
    if (Read < 0 && errno == EINTR)
      continue;
    if (Read <= 0)
      return policyError(Twine("cannot read pinned gfx942 device library '") +
                         Pin.Basename + "'");
    Offset += static_cast<size_t>(Read);
  }
  uint8_t Extra = 0;
  ssize_t ExtraRead;
  do {
    ExtraRead = ::read(File.get(), &Extra, 1);
  } while (ExtraRead < 0 && errno == EINTR);
  if (ExtraRead != 0)
    return policyError(Twine("pinned gfx942 device library '") + Pin.Basename +
                       "' changed size while being read");

  struct stat After{};
  if (::fstat(File.get(), &After) < 0 || !sameFileState(Before, After))
    return policyError(Twine("pinned gfx942 device library '") + Pin.Basename +
                       "' changed while being read");
  std::array<uint8_t, 32> Digest = SHA256::hash(Bytes);
  if (Digest != Pin.Digest)
    return policyError(Twine("pinned gfx942 device library '") + Pin.Basename +
                       "' digest does not match the worker measurement");
  return Input{InputKind::LlvmBitcode, Digest, std::move(Bytes)};
}

} // namespace

bool isSupportedGfx942OcmlImport(StringRef Symbol) {
  return llvm::is_contained(SupportedOcmlImports, Symbol);
}

bool isOcmlImportNamespace(StringRef Symbol) {
  return Symbol.starts_with("__ocml_");
}

Expected<Gfx942DeviceLibraryPolicy> measuredGfx942DeviceLibraryPolicy() {
#if FE2O3_GFX942_DEVICE_LIBS_ENABLED
  auto Ocml = parseDigest(FE2O3_GFX942_OCML_SHA256);
  if (!Ocml)
    return Ocml.takeError();
  auto Isa = parseDigest(FE2O3_GFX942_ISA_SHA256);
  if (!Isa)
    return Isa.takeError();
  auto UnsafeMath = parseDigest(FE2O3_GFX942_UNSAFE_MATH_SHA256);
  if (!UnsafeMath)
    return UnsafeMath.takeError();
  auto FiniteOnly = parseDigest(FE2O3_GFX942_FINITE_ONLY_SHA256);
  if (!FiniteOnly)
    return FiniteOnly.takeError();
  Gfx942DeviceLibraryPolicy Result{
      FE2O3_GFX942_DEVICE_LIB_DIR,
      {{"ocml.bc", *Ocml, MaxDeviceLibraryFileBytes},
       {"oclc_isa_version_942.bc", *Isa, MaxDeviceLibraryFileBytes},
       {"oclc_unsafe_math_off.bc", *UnsafeMath, MaxDeviceLibraryFileBytes},
       {"oclc_finite_only_off.bc", *FiniteOnly, MaxDeviceLibraryFileBytes}}};
  if (Error E = validatePolicy(Result))
    return E;
  return Result;
#else
  return policyError(
      "worker was built without measured gfx942 device libraries");
#endif
}

Expected<std::vector<Input>>
loadGfx942DeviceLibraries(ArrayRef<std::string> Imports,
                          const Gfx942DeviceLibraryPolicy &Policy) {
  bool Required = false;
  for (StringRef Import : Imports) {
    if (isSupportedGfx942OcmlImport(Import)) {
      Required = true;
      continue;
    }
    if (isOcmlImportNamespace(Import))
      return policyError(Twine("unsupported gfx942 OCML import: ") + Import);
  }
  if (!Required)
    return std::vector<Input>{};
  if (Error E = validatePolicy(Policy))
    return E;

  FileDescriptor Directory(
      ::open(Policy.Directory.c_str(),
             O_RDONLY | O_CLOEXEC | O_DIRECTORY | O_NOFOLLOW | O_NONBLOCK));
  if (Directory.get() < 0)
    return policyError(Twine("cannot open gfx942 device-library directory: ") +
                       std::strerror(errno));
  struct stat DirectoryStatus{};
  if (::fstat(Directory.get(), &DirectoryStatus) < 0 ||
      !S_ISDIR(DirectoryStatus.st_mode))
    return policyError("gfx942 device-library path is not a directory");

  std::vector<Input> Result;
  Result.reserve(Policy.Files.size());
  for (const PinnedDeviceLibraryFile &Pin : Policy.Files) {
    auto File = readPinnedFile(Directory.get(), Pin);
    if (!File)
      return File.takeError();
    Result.push_back(std::move(*File));
  }
  return Result;
}

} // namespace fe2o3::worker
