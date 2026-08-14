#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/raw_ostream.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

namespace {

int fail(const char *Message) {
  llvm::errs() << "codec test failure: " << Message << '\n';
  return 1;
}

uint8_t hexDigit(char Value) {
  if (Value >= '0' && Value <= '9')
    return static_cast<uint8_t>(Value - '0');
  if (Value >= 'a' && Value <= 'f')
    return static_cast<uint8_t>(Value - 'a' + 10);
  return 0xff;
}

std::vector<uint8_t> fromHex(llvm::StringRef Hex) {
  if ((Hex.size() % 2) != 0)
    return {};
  std::vector<uint8_t> Result;
  Result.reserve(Hex.size() / 2);
  for (size_t I = 0; I < Hex.size(); I += 2) {
    uint8_t High = hexDigit(Hex[I]);
    uint8_t Low = hexDigit(Hex[I + 1]);
    if (High == 0xff || Low == 0xff)
      return {};
    Result.push_back(static_cast<uint8_t>((High << 4) | Low));
  }
  return Result;
}

} // namespace

int main() {
  std::vector<uint8_t> Empty;
  auto EmptyResult = decodeRequest(Empty);
  if (EmptyResult)
    return fail("empty request was accepted");
  llvm::consumeError(EmptyResult.takeError());

  std::vector<uint8_t> BadMagic{'N', 'O', 'T', 'F', 'E', '2', 'O', '3'};
  auto BadMagicResult = decodeRequest(BadMagic);
  if (BadMagicResult)
    return fail("request with bad magic was accepted");
  llvm::consumeError(BadMagicResult.takeError());

  constexpr llvm::StringLiteral V2GoldenHex =
      "46334c52455130320100200000001111111111111111111111111111111111111111"
      "1111111111111111111111110200070000006c6c766d2d7632030009000000776f"
      "726b65722d7632040028000000222222222222222222222222222222222222222222"
      "2222222222222222222222d20400000000000005000d0000006766783934323a786e61"
      "636b2d06000100000005070003000000020101080020000000333333333333333333"
      "3333333333333333333333333333333333333333333333090038000000012395170d"
      "a16ee85f3dd5dc51e591b796926a189d5f9da6bd8c3b9ef2658b71630f0000000000"
      "0000636f6d70696c65722d6d6f64756c650a003c0000000100000002eefaf5106d05"
      "5bc7739d91015ae3c8468e852ca1cf7455e178a79f70649cbfb90f00000000000000"
      "70726f76696465722d6f626a6563740b0017000000010000000f0000006578746572"
      "6e616c5f68656c7065720c0014000000010000000c0000006b65726e656c5f656e74"
      "72790d0027000000020000000f00000065787465726e616c5f68656c7065720c0000"
      "006b65726e656c5f656e7472790e000800000000100000000000000f0020000000f0"
      "06c4b5601d316904877004ab1b886057bc92ed7a0e3cf8c2b40866bcae5da4";
  std::vector<uint8_t> V2Golden = fromHex(V2GoldenHex);
  if (V2Golden.empty())
    return fail("Rust V2 golden could not be decoded from hex");
  auto V2Request = decodeRequestV2(V2Golden);
  if (!V2Request) {
    llvm::logAllUnhandledErrors(V2Request.takeError(), llvm::errs(),
                                "codec test failure: ");
    return 1;
  }
  if (V2Request->Protocol != ProtocolVersion::V2 ||
      V2Request->LlvmBuildIdentity != "llvm-v2" ||
      V2Request->WorkerBuildIdentity != "worker-v2" ||
      V2Request->WorkerExecutableBytes != 1234 ||
      V2Request->Target != "gfx942:xnack-" ||
      V2Request->CompilerModule.Bytes !=
          std::vector<uint8_t>({'c', 'o', 'm', 'p', 'i', 'l', 'e', 'r', '-',
                                'm', 'o', 'd', 'u', 'l', 'e'}) ||
      V2Request->ExternalProviders.size() != 1 ||
      V2Request->ImportSymbols !=
          std::vector<std::string>({"external_helper"}) ||
      V2Request->ExportSymbols != std::vector<std::string>({"kernel_entry"}) ||
      V2Request->FinalSymbols !=
          std::vector<std::string>({"external_helper", "kernel_entry"}))
    return fail("Rust V2 golden decoded with different semantics");

  auto V1Downgrade = decodeRequest(V2Golden);
  if (V1Downgrade)
    return fail("V2 request was accepted by the V1 decoder");
  llvm::consumeError(V1Downgrade.takeError());
  for (size_t I = 0; I < V2Golden.size(); ++I) {
    std::vector<uint8_t> Mutated = V2Golden;
    Mutated[I] ^= 0x80;
    auto Result = decodeRequestV2(Mutated);
    if (Result)
      return fail("mutated V2 request was accepted");
    llvm::consumeError(Result.takeError());
  }

  Response V2Failure{V2Request->RequestId,  V2Request->Identity,
                     FE2O3_WORKER_BUILD_ID, Stage::NativeLink,
                     {"V2 failure"},        std::nullopt};
  V2Failure.Protocol = ProtocolVersion::V2;
  V2Failure.CompilerEnvelopeIdentity = V2Request->CompilerEnvelopeIdentity;
  auto EncodedV2Response = encodeResponse(std::move(V2Failure));
  if (!EncodedV2Response) {
    llvm::logAllUnhandledErrors(EncodedV2Response.takeError(), llvm::errs(),
                                "codec test failure: ");
    return 1;
  }
  const std::array<uint8_t, 8> ResponseMagicV2 = {'F', '3', 'L', 'R',
                                                  'S', 'P', '0', '2'};
  if (!std::equal(ResponseMagicV2.begin(), ResponseMagicV2.end(),
                  EncodedV2Response->begin()))
    return fail("V2 response used the wrong wire version");

  DeviceLibraryProviderEvidence Provider;
  Provider.ProviderIdentity = "gfx942-ocml-v1";
  Provider.Target = "gfx942:xnack-";
  Provider.CodeObjectVersion = 6;
  Provider.ImportSymbols = {"__ocml_exp_f32"};
  Provider.Files = {{"ocml.bc", {}}};
  auto ManifestIdentity = calculateProviderManifestIdentity(Provider);
  if (!ManifestIdentity)
    return fail("provider manifest identity could not be calculated");
  Provider.ManifestIdentity = *ManifestIdentity;
  Response ProviderSuccess{V2Request->RequestId,  V2Request->Identity,
                           FE2O3_WORKER_BUILD_ID, Stage::Complete,
                           {"provider success"},  Output{{}, {'o', 'k'}}};
  ProviderSuccess.LinkedOutput->Digest =
      llvm::SHA256::hash(ProviderSuccess.LinkedOutput->Bytes);
  ProviderSuccess.Protocol = ProtocolVersion::V2;
  ProviderSuccess.CompilerEnvelopeIdentity =
      V2Request->CompilerEnvelopeIdentity;
  ProviderSuccess.DeviceLibraryProvider = Provider;
  auto EncodedProviderResponse = encodeResponse(ProviderSuccess);
  if (!EncodedProviderResponse)
    return fail("provider response could not be encoded");
  const std::array<uint8_t, 8> ResponseMagicV3 = {'F', '3', 'L', 'R',
                                                  'S', 'P', '0', '3'};
  if (!std::equal(ResponseMagicV3.begin(), ResponseMagicV3.end(),
                  EncodedProviderResponse->begin()))
    return fail("provider response omitted the authenticated extension");
  ProviderSuccess.DeviceLibraryProvider->ManifestIdentity[0] ^= 0xff;
  auto WrongProviderIdentity = encodeResponse(std::move(ProviderSuccess));
  if (WrongProviderIdentity)
    return fail("provider response accepted a false manifest identity");
  llvm::consumeError(WrongProviderIdentity.takeError());

  std::vector<uint8_t> UnknownTag{'F', '3',  'L',  'R', 'E', 'Q', '0',
                                  '1', 0xff, 0xff, 0,   0,   0,   0};
  auto UnknownTagResult = decodeRequest(UnknownTag);
  if (UnknownTagResult)
    return fail("request with unknown tag was accepted");
  llvm::consumeError(UnknownTagResult.takeError());

  Response Failure{{},
                   {},
                   FE2O3_WORKER_BUILD_ID,
                   Stage::Decode,
                   {"z diagnostic", "a diagnostic", "a diagnostic"},
                   std::nullopt};
  auto Encoded = encodeResponse(std::move(Failure));
  if (!Encoded) {
    llvm::logAllUnhandledErrors(Encoded.takeError(), llvm::errs(),
                                "codec test failure: ");
    return 1;
  }
  const std::array<uint8_t, 8> ResponseMagic = {'F', '3', 'L', 'R',
                                                'S', 'P', '0', '1'};
  if (Encoded->size() <= ResponseMagic.size())
    return fail("encoded response is too short");
  if (!std::equal(ResponseMagic.begin(), ResponseMagic.end(), Encoded->begin()))
    return fail("encoded response has bad magic");

  std::vector<std::string> Sanitized = canonicalDiagnostics(
      {"/tmp/private-123/input: failure", "a", "a"}, "/tmp/private-123");
  if (Sanitized.size() != 2)
    return fail("canonical diagnostics have the wrong size");
  if (Sanitized[0] != "<internal>/input: failure")
    return fail("canonical diagnostics retained an internal path");
  if (Sanitized[1] != "a")
    return fail("canonical diagnostics were not sorted and deduplicated");
  return 0;
}
