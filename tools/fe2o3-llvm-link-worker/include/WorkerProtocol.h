#ifndef FE2O3_LLVM_LINK_WORKER_PROTOCOL_H
#define FE2O3_LLVM_LINK_WORKER_PROTOCOL_H

#include "llvm/ADT/ArrayRef.h"
#include "llvm/Support/Error.h"

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace fe2o3::worker {

inline constexpr size_t MaxRequestBytes = 64 * 1024 * 1024 + 256 * 1024;
inline constexpr size_t MaxTotalInputBytes = 64 * 1024 * 1024;
inline constexpr size_t MaxOutputBytes = 64 * 1024 * 1024;
inline constexpr size_t MaxInputs = 128;
inline constexpr size_t MaxSymbols = 4096;
inline constexpr size_t MaxSymbolBytes = 256;
inline constexpr size_t MaxDiagnostics = 64;
inline constexpr size_t MaxDiagnosticBytes = 1024;
inline constexpr size_t MaxTotalDiagnosticBytes = 16 * 1024;
inline constexpr size_t MaxWorkerExecutableBytes = 512 * 1024 * 1024;

enum class ProtocolVersion : uint8_t { V1 = 1, V2 = 2 };
enum class InputKind : uint8_t {
  LlvmBitcode = 1,
  AmdGpuRelocatable = 2,
  LlvmTextIr = 3,
};
enum class OptimizationLevel : uint8_t { O0 = 0, O1 = 1, O2 = 2, O3 = 3 };
enum class Stage : uint8_t {
  Decode = 1,
  Toolchain = 2,
  InputValidation = 3,
  BitcodeLink = 4,
  Optimization = 5,
  Codegen = 6,
  NativeLink = 7,
  OutputInspection = 8,
  Complete = 9,
};

struct Options {
  OptimizationLevel Optimization = OptimizationLevel::O0;
  bool StripDebug = false;
  bool VerifyEach = false;
};

struct Input {
  InputKind Kind = InputKind::LlvmBitcode;
  std::array<uint8_t, 32> Digest{};
  std::vector<uint8_t> Bytes;
};

struct Request {
  std::array<uint8_t, 32> RequestId{};
  std::array<uint8_t, 32> Identity{};
  std::string LlvmBuildIdentity;
  std::string Target;
  uint8_t CodeObjectVersion = 0;
  Options LinkOptions;
  std::vector<Input> Inputs;
  std::vector<std::string> RequiredSymbols;
  std::vector<std::string> ExpectedDefinedSymbols;
  uint64_t MaxOutputBytes = 0;
  ProtocolVersion Protocol = ProtocolVersion::V1;
  std::string WorkerBuildIdentity;
  std::array<uint8_t, 32> WorkerExecutableDigest{};
  uint64_t WorkerExecutableBytes = 0;
  std::array<uint8_t, 32> CompilerEnvelopeIdentity{};
  Input CompilerModule;
  std::vector<Input> ExternalProviders;
  std::vector<std::string> ImportSymbols;
  std::vector<std::string> ExportSymbols;
  std::vector<std::string> FinalSymbols;
};

struct Output {
  std::array<uint8_t, 32> Digest{};
  std::vector<uint8_t> Bytes;
};

struct Response {
  std::array<uint8_t, 32> RequestId{};
  std::array<uint8_t, 32> RequestIdentity{};
  std::string WorkerBuildIdentity;
  Stage FailureStage = Stage::Decode;
  std::vector<std::string> Diagnostics;
  std::optional<Output> LinkedOutput;
  ProtocolVersion Protocol = ProtocolVersion::V1;
  std::array<uint8_t, 32> CompilerEnvelopeIdentity{};
};

llvm::Expected<Request> decodeRequest(llvm::ArrayRef<uint8_t> Bytes);
llvm::Expected<Request> decodeRequestV2(llvm::ArrayRef<uint8_t> Bytes);
llvm::Expected<Request> decodeAnyRequest(llvm::ArrayRef<uint8_t> Bytes);
llvm::Expected<ProtocolVersion>
detectRequestProtocol(llvm::ArrayRef<uint8_t> Bytes);
llvm::Expected<std::vector<uint8_t>> encodeResponse(Response ResponseValue);

std::vector<std::string>
canonicalDiagnostics(llvm::ArrayRef<std::string> Diagnostics,
                     llvm::StringRef InternalPath = {});
std::string errorToDiagnostic(llvm::Error ErrorValue);

} // namespace fe2o3::worker

#endif
