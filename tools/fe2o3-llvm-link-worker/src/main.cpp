#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"

#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

namespace {

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

int runPhysicalMachineEffect(llvm::ArrayRef<uint8_t> Bytes) {
  auto RequestValue = decodePhysicalMachineEffectRequest(Bytes);
  if (!RequestValue) {
    std::string Diagnostic = llvm::toString(RequestValue.takeError());
    std::fprintf(stderr, "%s\n", Diagnostic.c_str());
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

int runPhysicalMachineEffectIdentity(llvm::ArrayRef<uint8_t> Bytes) {
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
      ArgumentCount == 2 &&
      std::strcmp(ArgumentValues[1], "--machine-effects-gfx942-v1") == 0;
  bool PhysicalMachineEffectIdentity =
      ArgumentCount == 2 &&
      std::strcmp(ArgumentValues[1],
                  "--machine-effects-gfx942-identities-v1") == 0;
  if (ArgumentCount != 1 && !PhysicalMachineEffect &&
      !PhysicalMachineEffectIdentity)
    return 64;
  std::vector<uint8_t> Bytes;
  Bytes.reserve(64 * 1024);
  if (!readBoundedStdin(Bytes)) {
    auto Version = detectRequestProtocol(Bytes);
    if (!Version || *Version != ProtocolVersion::V1) {
      if (!Version)
        llvm::consumeError(Version.takeError());
      return 65;
    }
    return v1DecodeFailure("worker request exceeds byte bound");
  }
  if (PhysicalMachineEffect)
    return runPhysicalMachineEffect(Bytes);
  if (PhysicalMachineEffectIdentity)
    return runPhysicalMachineEffectIdentity(Bytes);
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
