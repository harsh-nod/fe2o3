#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <optional>
#include <string>
#include <sys/resource.h>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

namespace {

constexpr rlim_t MachineEffectAddressSpaceBytes = 4ULL * 1024 * 1024 * 1024;
constexpr rlim_t MachineEffectDataBytes = 2ULL * 1024 * 1024 * 1024;
constexpr rlim_t MachineEffectFileBytes = 16ULL * 1024 * 1024;
constexpr char MachineEffectControlPrefix[] = "--fe2o3-control-challenge=";
constexpr char MachineEffectRequestBytesPrefix[] = "--fe2o3-request-bytes=";
constexpr char MachineEffectReadyDomain[] =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-READY/V1";
constexpr char MachineEffectDoneDomain[] =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-DONE/V1";
constexpr char MachineEffectAckDomain[] =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-ACK/V1";

bool setBoundedLimit(int Resource, rlim_t Bound) {
  struct rlimit Existing{};
  if (::getrlimit(Resource, &Existing) != 0)
    return false;
  auto Clamp = [Bound](rlim_t Value) {
    return Value == RLIM_INFINITY || Value > Bound ? Bound : Value;
  };
  struct rlimit Limited{Clamp(Existing.rlim_cur), Clamp(Existing.rlim_max)};
  if (Limited.rlim_cur > Limited.rlim_max)
    Limited.rlim_cur = Limited.rlim_max;
  return ::setrlimit(Resource, &Limited) == 0;
}

bool installMachineEffectResourceLimits() {
  return setBoundedLimit(RLIMIT_AS, MachineEffectAddressSpaceBytes) &&
         setBoundedLimit(RLIMIT_DATA, MachineEffectDataBytes) &&
         setBoundedLimit(RLIMIT_FSIZE, MachineEffectFileBytes) &&
         setBoundedLimit(RLIMIT_CORE, 0) && setBoundedLimit(RLIMIT_NPROC, 0);
}

bool readBoundedStdin(std::vector<uint8_t> &Bytes) {
  std::array<uint8_t, 16 * 1024> Buffer{};
  while (true) {
    size_t Count = std::fread(Buffer.data(), 1, Buffer.size(), stdin);
    if (Count != 0) {
      if (Bytes.size() > MaxRequestBytes - Count)
        return false;
      Bytes.insert(Bytes.end(), Buffer.begin(), Buffer.begin() + Count);
    }
    if (Count != Buffer.size())
      return std::feof(stdin) != 0 && std::ferror(stdin) == 0;
  }
}

bool readExactStdin(std::vector<uint8_t> &Bytes, size_t Size) {
  if (Size == 0 || Size > MaxRequestBytes)
    return false;
  Bytes.resize(Size);
  size_t Read = 0;
  while (Read != Size) {
    size_t Count = std::fread(Bytes.data() + Read, 1, Size - Read, stdin);
    if (Count == 0)
      return false;
    Read += Count;
  }
  return true;
}

std::optional<std::array<uint8_t, 32>>
parseControlChallenge(const char *Value) {
  llvm::StringRef Text(Value);
  if (!Text.consume_front(MachineEffectControlPrefix) || Text.size() != 64)
    return std::nullopt;
  std::array<uint8_t, 32> Result{};
  for (size_t Index = 0; Index != Result.size(); ++Index) {
    unsigned Byte = 0;
    llvm::StringRef Pair = Text.slice(Index * 2, Index * 2 + 2);
    if (Pair.getAsInteger(16, Byte) || Byte > 0xff)
      return std::nullopt;
    Result[Index] = static_cast<uint8_t>(Byte);
  }
  return Result;
}

std::optional<size_t> parseRequestBytes(const char *Value) {
  llvm::StringRef Text(Value);
  if (!Text.consume_front(MachineEffectRequestBytesPrefix) || Text.empty())
    return std::nullopt;
  uint64_t Result = 0;
  if (Text.getAsInteger(10, Result) || Result == 0 ||
      Result > MaxRequestBytes || Result > std::numeric_limits<size_t>::max())
    return std::nullopt;
  return static_cast<size_t>(Result);
}

template <size_t DomainBytes>
bool writeControl(const char (&Domain)[DomainBytes],
                  const std::array<uint8_t, 32> &Challenge) {
  return std::fwrite(Domain, 1, DomainBytes, stderr) == DomainBytes &&
         std::fwrite(Challenge.data(), 1, Challenge.size(), stderr) ==
             Challenge.size() &&
         std::fflush(stderr) == 0;
}

bool awaitControlAck(const std::array<uint8_t, 32> &Challenge) {
  std::array<uint8_t, sizeof(MachineEffectAckDomain) + 32> Ack{};
  size_t Read = 0;
  while (Read != Ack.size()) {
    size_t Count = std::fread(Ack.data() + Read, 1, Ack.size() - Read, stdin);
    if (Count == 0)
      return false;
    Read += Count;
  }
  return std::memcmp(Ack.data(), MachineEffectAckDomain,
                     sizeof(MachineEffectAckDomain)) == 0 &&
         std::memcmp(Ack.data() + sizeof(MachineEffectAckDomain),
                     Challenge.data(), Challenge.size()) == 0 &&
         std::fgetc(stdin) == EOF && std::feof(stdin) != 0 &&
         std::ferror(stdin) == 0;
}

int writeResponse(Response ResponseValue) {
  auto Encoded = encodeResponse(std::move(ResponseValue));
  if (!Encoded) {
    llvm::consumeError(Encoded.takeError());
    return 70;
  }
  if (std::fwrite(Encoded->data(), 1, Encoded->size(), stdout) !=
          Encoded->size() ||
      std::fflush(stdout) != 0)
    return 74;
  return 0;
}

int runPhysicalMachineEffect(llvm::ArrayRef<uint8_t> Bytes,
                             const std::array<uint8_t, 32> &Challenge) {
  auto RequestValue = decodePhysicalMachineEffectRequest(Bytes);
  if (!RequestValue) {
    std::string Diagnostic = llvm::toString(RequestValue.takeError());
    std::fprintf(stderr, "%s\n", Diagnostic.c_str());
    return 65;
  }
  if (RequestValue->ExecutionChallenge != Challenge) {
    std::fprintf(stderr, "control and request challenges disagree\n");
    return 65;
  }
  auto Evidence = analyzeGfx942PhysicalMachineEffects(*RequestValue);
  if (!Evidence) {
    std::string Diagnostic = llvm::toString(Evidence.takeError());
    std::fprintf(stderr, "%s\n", Diagnostic.c_str());
    return 65;
  }
  auto Encoded = encodePhysicalMachineEffectEvidence(*Evidence);
  if (!Encoded) {
    std::string Diagnostic = llvm::toString(Encoded.takeError());
    std::fprintf(stderr, "%s\n", Diagnostic.c_str());
    return 70;
  }
  if (std::fwrite(Encoded->data(), 1, Encoded->size(), stdout) !=
          Encoded->size() ||
      std::fflush(stdout) != 0)
    return 74;
  return 0;
}

int runPhysicalMachineEffectIdentity(llvm::ArrayRef<uint8_t> Bytes,
                                     const std::array<uint8_t, 32> &Challenge) {
  if (Bytes.size() < Challenge.size() ||
      !std::equal(Challenge.begin(), Challenge.end(),
                  Bytes.end() - Challenge.size())) {
    std::fprintf(stderr, "control and identity challenges disagree\n");
    return 65;
  }
  auto Encoded = encodePhysicalMachineEffectIdentityResponse(Bytes);
  if (!Encoded) {
    std::string Diagnostic = llvm::toString(Encoded.takeError());
    std::fprintf(stderr, "%s\n", Diagnostic.c_str());
    return 65;
  }
  if (std::fwrite(Encoded->data(), 1, Encoded->size(), stdout) !=
          Encoded->size() ||
      std::fflush(stdout) != 0)
    return 74;
  return 0;
}

int v1DecodeFailure(const char *Diagnostic) {
  return writeResponse({{},
                        {},
                        FE2O3_WORKER_BUILD_ID,
                        Stage::Decode,
                        {Diagnostic},
                        std::nullopt});
}

} // namespace

int main(int ArgumentCount, char **ArgumentValues) {
  bool PhysicalMachineEffect =
      ArgumentCount == 4 &&
      std::strcmp(ArgumentValues[1], "--machine-effects-gfx942-v1") == 0;
  bool PhysicalMachineEffectIdentity =
      ArgumentCount == 4 &&
      std::strcmp(ArgumentValues[1],
                  "--machine-effects-gfx942-identities-v1") == 0;
  if (ArgumentCount != 1 && !PhysicalMachineEffect &&
      !PhysicalMachineEffectIdentity)
    return 64;
  if ((PhysicalMachineEffect || PhysicalMachineEffectIdentity) &&
      !installMachineEffectResourceLimits())
    return 70;
  std::optional<std::array<uint8_t, 32>> ControlChallenge;
  std::optional<size_t> RequestBytes;
  if (PhysicalMachineEffect || PhysicalMachineEffectIdentity) {
    ControlChallenge = parseControlChallenge(ArgumentValues[2]);
    RequestBytes = parseRequestBytes(ArgumentValues[3]);
    if (!ControlChallenge || !RequestBytes ||
        !writeControl(MachineEffectReadyDomain, *ControlChallenge))
      return 70;
  }
  std::vector<uint8_t> Bytes;
  Bytes.reserve(64 * 1024);
  bool ReadInput = RequestBytes ? readExactStdin(Bytes, *RequestBytes)
                                : readBoundedStdin(Bytes);
  if (!ReadInput) {
    auto Version = detectRequestProtocol(Bytes);
    if (!Version || *Version != ProtocolVersion::V1) {
      if (!Version)
        llvm::consumeError(Version.takeError());
      return 65;
    }
    return v1DecodeFailure("worker request exceeds byte bound");
  }
  if (PhysicalMachineEffect || PhysicalMachineEffectIdentity) {
    int Result =
        PhysicalMachineEffect
            ? runPhysicalMachineEffect(Bytes, *ControlChallenge)
            : runPhysicalMachineEffectIdentity(Bytes, *ControlChallenge);
    if (Result != 0)
      return Result;
    if (!writeControl(MachineEffectDoneDomain, *ControlChallenge) ||
        !awaitControlAck(*ControlChallenge))
      return 70;
    return 0;
  }
  auto Version = detectRequestProtocol(Bytes);
  if (!Version) {
    llvm::consumeError(Version.takeError());
    return 65;
  }
  auto RequestValue = decodeAnyRequest(Bytes);
  if (!RequestValue) {
    if (*Version == ProtocolVersion::V2) {
      llvm::consumeError(RequestValue.takeError());
      return 65;
    }
    std::string Diagnostic = errorToDiagnostic(RequestValue.takeError());
    return v1DecodeFailure(Diagnostic.c_str());
  }
  return writeResponse(execute(*RequestValue));
}
