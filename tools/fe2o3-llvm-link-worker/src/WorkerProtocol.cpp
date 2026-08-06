#include "WorkerProtocol.h"

#include "llvm/ADT/STLExtras.h"
#include "llvm/ADT/StringRef.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/SHA256.h"

#include <algorithm>
#include <cstring>
#include <limits>
#include <set>
#include <tuple>

using namespace llvm;

namespace fe2o3::worker {
namespace {

constexpr uint8_t RequestMagicV1[] = {'F', '3', 'L', 'R', 'E', 'Q', '0', '1'};
constexpr uint8_t ResponseMagicV1[] = {'F', '3', 'L', 'R', 'S', 'P', '0', '1'};
constexpr uint8_t RequestMagicV2[] = {'F', '3', 'L', 'R', 'E', 'Q', '0', '2'};
constexpr uint8_t ResponseMagicV2[] = {'F', '3', 'L', 'R', 'S', 'P', '0', '2'};
constexpr char RequestDomainV1[] = "FE2O3/DIRECT-LLVM-WORKER-REQUEST/V1\0";
constexpr char RequestDomainV2[] = "FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0";
constexpr size_t MaxBuildIdentityBytes = 160;
constexpr size_t MaxTargetBytes = 128;

Error protocolError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(), Message);
}

class Cursor {
public:
  explicit Cursor(ArrayRef<uint8_t> Bytes) : Bytes(Bytes) {}

  Expected<ArrayRef<uint8_t>> take(size_t Count) {
    if (Count > Bytes.size() - Position)
      return protocolError("truncated worker message");
    ArrayRef<uint8_t> Result = Bytes.slice(Position, Count);
    Position += Count;
    return Result;
  }

  Expected<uint8_t> byte() {
    auto BytesOrError = take(1);
    if (!BytesOrError)
      return BytesOrError.takeError();
    return (*BytesOrError)[0];
  }

  Expected<uint16_t> u16() {
    auto BytesOrError = take(2);
    if (!BytesOrError)
      return BytesOrError.takeError();
    return support::endian::read16le(BytesOrError->data());
  }

  Expected<uint32_t> u32() {
    auto BytesOrError = take(4);
    if (!BytesOrError)
      return BytesOrError.takeError();
    return support::endian::read32le(BytesOrError->data());
  }

  Expected<uint64_t> u64() {
    auto BytesOrError = take(8);
    if (!BytesOrError)
      return BytesOrError.takeError();
    return support::endian::read64le(BytesOrError->data());
  }

  Error finish() const {
    if (Position != Bytes.size())
      return protocolError("worker message has trailing bytes");
    return Error::success();
  }

  size_t position() const { return Position; }

private:
  ArrayRef<uint8_t> Bytes;
  size_t Position = 0;
};

class MessageDecoder {
public:
  MessageDecoder(ArrayRef<uint8_t> Bytes, ArrayRef<uint8_t> Magic,
                 uint16_t MaxTag)
      : Bytes(Bytes), Input(Bytes), MaxTag(MaxTag) {
    auto Actual = Input.take(Magic.size());
    if (!Actual || *Actual != Magic) {
      if (!Actual)
        consumeError(Actual.takeError());
      Valid = false;
    }
  }

  Expected<ArrayRef<uint8_t>> field(uint16_t ExpectedTag, size_t MaxBytes) {
    if (!Valid)
      return protocolError("invalid worker protocol magic/version");
    auto TagOrError = Input.u16();
    if (!TagOrError)
      return TagOrError.takeError();
    uint16_t Tag = *TagOrError;
    if (Tag > MaxTag)
      return protocolError(Twine("unknown worker field tag ") + Twine(Tag));
    if (Tag == LastTag)
      return protocolError(Twine("duplicate worker field tag ") + Twine(Tag));
    if (Tag != ExpectedTag)
      return protocolError(Twine("noncanonical worker field tag ") +
                           Twine(Tag));
    LastTag = Tag;
    auto LengthOrError = Input.u32();
    if (!LengthOrError)
      return LengthOrError.takeError();
    if (*LengthOrError > MaxBytes)
      return protocolError(Twine("worker field exceeds bound: ") + Twine(Tag));
    return Input.take(*LengthOrError);
  }

  Error finish(uint16_t FinalTag) {
    if (LastTag != FinalTag)
      return protocolError("worker message is missing required fields");
    return Input.finish();
  }

  size_t position() const { return Input.position(); }

private:
  ArrayRef<uint8_t> Bytes;
  Cursor Input;
  uint16_t LastTag = 0;
  uint16_t MaxTag;
  bool Valid = true;
};

template <size_t N>
Expected<std::array<uint8_t, N>> fixed(ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() != N)
    return protocolError("fixed-width worker field has the wrong length");
  std::array<uint8_t, N> Result{};
  llvm::copy(Bytes, Result.begin());
  return Result;
}

Expected<std::string> text(ArrayRef<uint8_t> Bytes, size_t MaxBytes,
                           StringRef Field) {
  if (Bytes.empty() || Bytes.size() > MaxBytes)
    return protocolError(Twine("invalid ") + Field);
  for (uint8_t Byte : Bytes)
    if (Byte > 0x7f || Byte < 0x20 || Byte == 0x7f)
      return protocolError(Twine("noncanonical ") + Field);
  return std::string(reinterpret_cast<const char *>(Bytes.data()),
                     Bytes.size());
}

bool knownProcessor(StringRef Processor) {
  static constexpr StringLiteral Known[] = {
      "gfx600",  "gfx601",  "gfx602",  "gfx700",  "gfx701",  "gfx702",
      "gfx703",  "gfx704",  "gfx705",  "gfx801",  "gfx802",  "gfx803",
      "gfx805",  "gfx810",  "gfx900",  "gfx902",  "gfx904",  "gfx906",
      "gfx908",  "gfx909",  "gfx90a",  "gfx90c",  "gfx942",  "gfx950",
      "gfx1010", "gfx1011", "gfx1012", "gfx1013", "gfx1030", "gfx1031",
      "gfx1032", "gfx1033", "gfx1034", "gfx1035", "gfx1036", "gfx1100",
      "gfx1101", "gfx1102", "gfx1103", "gfx1150", "gfx1151", "gfx1152",
      "gfx1153", "gfx1154", "gfx1170", "gfx1171", "gfx1172", "gfx1200",
      "gfx1201", "gfx1250", "gfx1251", "gfx1310"};
  return llvm::is_contained(Known, Processor);
}

bool supportsFeature(StringRef Processor, StringRef Feature) {
  if (Feature == "sramecc")
    return Processor == "gfx906" || Processor == "gfx908" ||
           Processor == "gfx90a" || Processor == "gfx942" ||
           Processor == "gfx950" || Processor == "gfx1250" ||
           Processor == "gfx1251";
  if (Feature != "xnack")
    return false;
  static constexpr StringLiteral Supported[] = {
      "gfx801",  "gfx810",  "gfx900",  "gfx902", "gfx904", "gfx906",
      "gfx908",  "gfx909",  "gfx90a",  "gfx90c", "gfx942", "gfx950",
      "gfx1010", "gfx1011", "gfx1012", "gfx1013"};
  return llvm::is_contained(Supported, Processor);
}

Error validateTarget(StringRef Target) {
  SmallVector<StringRef, 3> Components;
  Target.split(Components, ':', -1, true);
  if (Components.empty() || !knownProcessor(Components.front()))
    return protocolError("unknown or noncanonical AMDGPU processor");
  StringRef Previous;
  for (StringRef Component : drop_begin(Components)) {
    if (Component.size() < 2 ||
        (Component.back() != '+' && Component.back() != '-'))
      return protocolError("invalid AMDGPU target feature");
    StringRef Feature = Component.drop_back();
    if ((Feature != "sramecc" && Feature != "xnack") ||
        !supportsFeature(Components.front(), Feature))
      return protocolError("unsupported AMDGPU target feature");
    if (!Previous.empty() && Previous >= Feature)
      return protocolError("noncanonical AMDGPU target feature order");
    Previous = Feature;
  }
  return Error::success();
}

Expected<std::vector<std::string>> decodeStrings(ArrayRef<uint8_t> Bytes,
                                                 size_t MaxCount,
                                                 size_t MaxEach,
                                                 size_t MaxTotal) {
  Cursor Input(Bytes);
  auto CountOrError = Input.u32();
  if (!CountOrError)
    return CountOrError.takeError();
  if (*CountOrError > MaxCount)
    return protocolError("worker string count exceeds bound");
  std::vector<std::string> Result;
  Result.reserve(*CountOrError);
  size_t Total = 0;
  for (uint32_t I = 0; I < *CountOrError; ++I) {
    auto LengthOrError = Input.u32();
    if (!LengthOrError)
      return LengthOrError.takeError();
    if (*LengthOrError > MaxEach || Total > MaxTotal - *LengthOrError)
      return protocolError("worker string bytes exceed bound");
    Total += *LengthOrError;
    auto ValueOrError = Input.take(*LengthOrError);
    if (!ValueOrError)
      return ValueOrError.takeError();
    Result.emplace_back(reinterpret_cast<const char *>(ValueOrError->data()),
                        ValueOrError->size());
  }
  if (Error E = Input.finish())
    return E;
  return Result;
}

Error validateSymbols(ArrayRef<std::string> Symbols) {
  StringRef Previous;
  for (const std::string &Symbol : Symbols) {
    StringRef Value(Symbol);
    if (Value.empty() || Value.size() > MaxSymbolBytes)
      return protocolError("invalid symbol length");
    for (unsigned char Byte : Value)
      if (Byte > 0x7f || Byte <= 0x20 || Byte == 0x7f || Byte == '/' ||
          Byte == '\\' || Byte == '\'' || Byte == '"')
        return protocolError("invalid symbol text");
    if (!Previous.empty() && Previous >= Value)
      return protocolError("symbols are duplicate or noncanonical");
    Previous = Value;
  }
  return Error::success();
}

Expected<Input> decodeInput(Cursor &InputBytes, size_t &Total,
                            size_t MaxTotal) {
  auto KindOrError = InputBytes.byte();
  if (!KindOrError)
    return KindOrError.takeError();
  InputKind Kind;
  if (*KindOrError == static_cast<uint8_t>(InputKind::LlvmBitcode))
    Kind = InputKind::LlvmBitcode;
  else if (*KindOrError == static_cast<uint8_t>(InputKind::AmdGpuRelocatable))
    Kind = InputKind::AmdGpuRelocatable;
  else
    return protocolError("unknown worker input kind");
  auto DigestBytes = InputBytes.take(32);
  if (!DigestBytes)
    return DigestBytes.takeError();
  auto Digest = fixed<32>(*DigestBytes);
  if (!Digest)
    return Digest.takeError();
  auto LengthOrError = InputBytes.u64();
  if (!LengthOrError)
    return LengthOrError.takeError();
  if (*LengthOrError == 0 || *LengthOrError > MaxTotal ||
      Total > MaxTotal - *LengthOrError)
    return protocolError("worker input bytes exceed bound");
  Total += static_cast<size_t>(*LengthOrError);
  auto PayloadOrError = InputBytes.take(static_cast<size_t>(*LengthOrError));
  if (!PayloadOrError)
    return PayloadOrError.takeError();
  if (SHA256::hash(*PayloadOrError) != *Digest)
    return protocolError("worker input digest mismatch");
  return Input{
      Kind, *Digest,
      std::vector<uint8_t>(PayloadOrError->begin(), PayloadOrError->end())};
}

Error validateInputOrder(ArrayRef<Input> Inputs) {
  for (size_t I = 1; I < Inputs.size(); ++I) {
    const Input &A = Inputs[I - 1];
    const Input &B = Inputs[I];
    auto AKey = std::tuple(A.Digest, A.Bytes.size(), A.Kind);
    auto BKey = std::tuple(B.Digest, B.Bytes.size(), B.Kind);
    if (AKey >= BKey)
      return protocolError("worker inputs are duplicate or noncanonical");
  }
  return Error::success();
}

Expected<Input> decodeSingleInput(ArrayRef<uint8_t> Bytes) {
  Cursor InputBytes(Bytes);
  size_t Total = 0;
  auto Result = decodeInput(InputBytes, Total, MaxTotalInputBytes);
  if (!Result)
    return Result.takeError();
  if (Error E = InputBytes.finish())
    return E;
  return Result;
}

Expected<std::vector<Input>>
decodeInputs(ArrayRef<uint8_t> Bytes, bool AllowEmpty = false,
             size_t MaxTotal = MaxTotalInputBytes) {
  Cursor InputBytes(Bytes);
  auto CountOrError = InputBytes.u32();
  if (!CountOrError)
    return CountOrError.takeError();
  if ((!AllowEmpty && *CountOrError == 0) || *CountOrError > MaxInputs)
    return protocolError("invalid worker input count");
  std::vector<Input> Result;
  Result.reserve(*CountOrError);
  size_t Total = 0;
  for (uint32_t I = 0; I < *CountOrError; ++I) {
    auto Value = decodeInput(InputBytes, Total, MaxTotal);
    if (!Value)
      return Value.takeError();
    Result.push_back(std::move(*Value));
  }
  if (Error E = InputBytes.finish())
    return E;
  if (Error E = validateInputOrder(Result))
    return E;
  return Result;
}

void appendU16(std::vector<uint8_t> &Out, uint16_t Value) {
  uint8_t Bytes[2];
  support::endian::write16le(Bytes, Value);
  Out.insert(Out.end(), Bytes, Bytes + 2);
}

void appendU32(std::vector<uint8_t> &Out, uint32_t Value) {
  uint8_t Bytes[4];
  support::endian::write32le(Bytes, Value);
  Out.insert(Out.end(), Bytes, Bytes + 4);
}

void appendU64(std::vector<uint8_t> &Out, uint64_t Value) {
  uint8_t Bytes[8];
  support::endian::write64le(Bytes, Value);
  Out.insert(Out.end(), Bytes, Bytes + 8);
}

Error appendField(std::vector<uint8_t> &Out, uint16_t Tag,
                  ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() > std::numeric_limits<uint32_t>::max())
    return protocolError("worker response field length overflow");
  appendU16(Out, Tag);
  appendU32(Out, static_cast<uint32_t>(Bytes.size()));
  Out.insert(Out.end(), Bytes.begin(), Bytes.end());
  return Error::success();
}

std::vector<uint8_t> encodeStrings(ArrayRef<std::string> Values) {
  std::vector<uint8_t> Result;
  appendU32(Result, static_cast<uint32_t>(Values.size()));
  for (const std::string &Value : Values) {
    appendU32(Result, static_cast<uint32_t>(Value.size()));
    Result.insert(Result.end(), Value.begin(), Value.end());
  }
  return Result;
}

} // namespace

Expected<Request> decodeRequest(ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() > MaxRequestBytes)
    return protocolError("worker request exceeds bound");
  MessageDecoder Decoder(Bytes, RequestMagicV1, 10);
  Request Result;
  auto RequestId = Decoder.field(1, 32);
  if (!RequestId)
    return RequestId.takeError();
  auto ParsedRequestId = fixed<32>(*RequestId);
  if (!ParsedRequestId)
    return ParsedRequestId.takeError();
  Result.RequestId = *ParsedRequestId;
  if (llvm::all_of(Result.RequestId, [](uint8_t Byte) { return Byte == 0; }))
    return protocolError("request ID is the reserved zero value");

  auto BuildIdentity = Decoder.field(2, MaxBuildIdentityBytes);
  if (!BuildIdentity)
    return BuildIdentity.takeError();
  auto ParsedBuildIdentity =
      text(*BuildIdentity, MaxBuildIdentityBytes, "LLVM build identity");
  if (!ParsedBuildIdentity)
    return ParsedBuildIdentity.takeError();
  Result.LlvmBuildIdentity = std::move(*ParsedBuildIdentity);

  auto Target = Decoder.field(3, MaxTargetBytes);
  if (!Target)
    return Target.takeError();
  auto ParsedTarget = text(*Target, MaxTargetBytes, "AMDGPU target");
  if (!ParsedTarget)
    return ParsedTarget.takeError();
  if (Error E = validateTarget(*ParsedTarget))
    return E;
  Result.Target = std::move(*ParsedTarget);

  auto CodeObject = Decoder.field(4, 1);
  if (!CodeObject)
    return CodeObject.takeError();
  if (CodeObject->size() != 1 ||
      ((*CodeObject)[0] != 4 && (*CodeObject)[0] != 5 && (*CodeObject)[0] != 6))
    return protocolError("unsupported code-object version");
  Result.CodeObjectVersion = (*CodeObject)[0];

  auto OptionsBytes = Decoder.field(5, 3);
  if (!OptionsBytes)
    return OptionsBytes.takeError();
  if (OptionsBytes->size() != 3 || (*OptionsBytes)[0] > 3 ||
      (*OptionsBytes)[1] > 1 || (*OptionsBytes)[2] > 1)
    return protocolError("unsupported structured worker option");
  Result.LinkOptions.Optimization =
      static_cast<OptimizationLevel>((*OptionsBytes)[0]);
  Result.LinkOptions.StripDebug = (*OptionsBytes)[1] != 0;
  Result.LinkOptions.VerifyEach = (*OptionsBytes)[2] != 0;

  auto InputsBytes = Decoder.field(6, MaxTotalInputBytes + 8192);
  if (!InputsBytes)
    return InputsBytes.takeError();
  auto Inputs = decodeInputs(*InputsBytes);
  if (!Inputs)
    return Inputs.takeError();
  Result.Inputs = std::move(*Inputs);

  constexpr size_t MaxSymbolField = MaxSymbols * (MaxSymbolBytes + 4) + 4;
  auto RequiredBytes = Decoder.field(7, MaxSymbolField);
  if (!RequiredBytes)
    return RequiredBytes.takeError();
  auto Required = decodeStrings(*RequiredBytes, MaxSymbols, MaxSymbolBytes,
                                MaxSymbols * MaxSymbolBytes);
  if (!Required)
    return Required.takeError();
  if (Error E = validateSymbols(*Required))
    return E;
  Result.RequiredSymbols = std::move(*Required);

  auto DefinedBytes = Decoder.field(8, MaxSymbolField);
  if (!DefinedBytes)
    return DefinedBytes.takeError();
  auto Defined = decodeStrings(*DefinedBytes, MaxSymbols, MaxSymbolBytes,
                               MaxSymbols * MaxSymbolBytes);
  if (!Defined)
    return Defined.takeError();
  if (Error E = validateSymbols(*Defined))
    return E;
  Result.ExpectedDefinedSymbols = std::move(*Defined);
  for (const std::string &RequiredSymbol : Result.RequiredSymbols)
    if (!std::binary_search(Result.ExpectedDefinedSymbols.begin(),
                            Result.ExpectedDefinedSymbols.end(),
                            RequiredSymbol))
      return protocolError("required symbol is absent from exact symbol set");

  auto OutputBoundBytes = Decoder.field(9, 8);
  if (!OutputBoundBytes)
    return OutputBoundBytes.takeError();
  if (OutputBoundBytes->size() != 8)
    return protocolError("invalid output bound field");
  Result.MaxOutputBytes = support::endian::read64le(OutputBoundBytes->data());
  if (Result.MaxOutputBytes == 0 || Result.MaxOutputBytes > MaxOutputBytes)
    return protocolError("invalid output byte bound");

  size_t IdentityFieldOffset = Decoder.position();
  auto IdentityBytes = Decoder.field(10, 32);
  if (!IdentityBytes)
    return IdentityBytes.takeError();
  auto Identity = fixed<32>(*IdentityBytes);
  if (!Identity)
    return Identity.takeError();
  Result.Identity = *Identity;
  if (Error E = Decoder.finish(10))
    return E;

  SHA256 Hasher;
  Hasher.update(StringRef(RequestDomainV1, sizeof(RequestDomainV1) - 1));
  uint8_t LengthBytes[8];
  support::endian::write64le(LengthBytes, IdentityFieldOffset);
  Hasher.update(ArrayRef<uint8_t>(LengthBytes));
  Hasher.update(Bytes.take_front(IdentityFieldOffset));
  if (Hasher.final() != Result.Identity)
    return protocolError("worker request identity mismatch");
  return Result;
}

Expected<ProtocolVersion> detectRequestProtocol(ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() < sizeof(RequestMagicV1))
    return protocolError("truncated worker request magic");
  ArrayRef<uint8_t> Magic = Bytes.take_front(sizeof(RequestMagicV1));
  if (Magic == ArrayRef<uint8_t>(RequestMagicV1))
    return ProtocolVersion::V1;
  if (Magic == ArrayRef<uint8_t>(RequestMagicV2))
    return ProtocolVersion::V2;
  return protocolError("invalid worker protocol magic/version");
}

Expected<Request> decodeAnyRequest(ArrayRef<uint8_t> Bytes) {
  auto Version = detectRequestProtocol(Bytes);
  if (!Version)
    return Version.takeError();
  if (*Version == ProtocolVersion::V1)
    return decodeRequest(Bytes);
  return decodeRequestV2(Bytes);
}

Expected<Request> decodeRequestV2(ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() > MaxRequestBytes)
    return protocolError("worker V2 request exceeds bound");
  MessageDecoder Decoder(Bytes, RequestMagicV2, 15);
  Request Result;
  Result.Protocol = ProtocolVersion::V2;

  auto RequestId = Decoder.field(1, 32);
  if (!RequestId)
    return RequestId.takeError();
  auto ParsedRequestId = fixed<32>(*RequestId);
  if (!ParsedRequestId)
    return ParsedRequestId.takeError();
  Result.RequestId = *ParsedRequestId;
  if (llvm::all_of(Result.RequestId, [](uint8_t Byte) { return Byte == 0; }))
    return protocolError("request ID is the reserved zero value");

  auto LlvmIdentity = Decoder.field(2, MaxBuildIdentityBytes);
  if (!LlvmIdentity)
    return LlvmIdentity.takeError();
  auto ParsedLlvmIdentity =
      text(*LlvmIdentity, MaxBuildIdentityBytes, "LLVM build identity");
  if (!ParsedLlvmIdentity)
    return ParsedLlvmIdentity.takeError();
  Result.LlvmBuildIdentity = std::move(*ParsedLlvmIdentity);

  auto WorkerIdentity = Decoder.field(3, MaxBuildIdentityBytes);
  if (!WorkerIdentity)
    return WorkerIdentity.takeError();
  auto ParsedWorkerIdentity =
      text(*WorkerIdentity, MaxBuildIdentityBytes, "worker build identity");
  if (!ParsedWorkerIdentity)
    return ParsedWorkerIdentity.takeError();
  Result.WorkerBuildIdentity = std::move(*ParsedWorkerIdentity);

  auto ExecutableIdentity = Decoder.field(4, 40);
  if (!ExecutableIdentity)
    return ExecutableIdentity.takeError();
  if (ExecutableIdentity->size() != 40)
    return protocolError("invalid worker executable identity");
  auto ExecutableDigest = fixed<32>(ExecutableIdentity->take_front(32));
  if (!ExecutableDigest)
    return ExecutableDigest.takeError();
  Result.WorkerExecutableDigest = *ExecutableDigest;
  Result.WorkerExecutableBytes =
      support::endian::read64le(ExecutableIdentity->data() + 32);
  if (Result.WorkerExecutableBytes == 0 ||
      Result.WorkerExecutableBytes > MaxWorkerExecutableBytes)
    return protocolError("invalid worker executable byte length");

  auto Target = Decoder.field(5, MaxTargetBytes);
  if (!Target)
    return Target.takeError();
  auto ParsedTarget = text(*Target, MaxTargetBytes, "AMDGPU target");
  if (!ParsedTarget)
    return ParsedTarget.takeError();
  if (Error E = validateTarget(*ParsedTarget))
    return E;
  Result.Target = std::move(*ParsedTarget);

  auto CodeObject = Decoder.field(6, 1);
  if (!CodeObject)
    return CodeObject.takeError();
  if (CodeObject->size() != 1 ||
      ((*CodeObject)[0] != 4 && (*CodeObject)[0] != 5 && (*CodeObject)[0] != 6))
    return protocolError("unsupported code-object version");
  Result.CodeObjectVersion = (*CodeObject)[0];

  auto OptionsBytes = Decoder.field(7, 3);
  if (!OptionsBytes)
    return OptionsBytes.takeError();
  if (OptionsBytes->size() != 3 || (*OptionsBytes)[0] > 3 ||
      (*OptionsBytes)[1] > 1 || (*OptionsBytes)[2] > 1)
    return protocolError("unsupported structured worker option");
  Result.LinkOptions.Optimization =
      static_cast<OptimizationLevel>((*OptionsBytes)[0]);
  Result.LinkOptions.StripDebug = (*OptionsBytes)[1] != 0;
  Result.LinkOptions.VerifyEach = (*OptionsBytes)[2] != 0;

  auto EnvelopeIdentity = Decoder.field(8, 32);
  if (!EnvelopeIdentity)
    return EnvelopeIdentity.takeError();
  auto ParsedEnvelopeIdentity = fixed<32>(*EnvelopeIdentity);
  if (!ParsedEnvelopeIdentity)
    return ParsedEnvelopeIdentity.takeError();
  Result.CompilerEnvelopeIdentity = *ParsedEnvelopeIdentity;
  if (llvm::all_of(Result.CompilerEnvelopeIdentity,
                   [](uint8_t Byte) { return Byte == 0; }))
    return protocolError(
        "compiler envelope identity is the reserved zero value");

  auto ModuleBytes = Decoder.field(9, MaxTotalInputBytes + 41);
  if (!ModuleBytes)
    return ModuleBytes.takeError();
  auto Module = decodeSingleInput(*ModuleBytes);
  if (!Module)
    return Module.takeError();
  Result.CompilerModule = std::move(*Module);
  size_t RemainingInputBytes =
      MaxTotalInputBytes - Result.CompilerModule.Bytes.size();

  constexpr size_t MaxProviderOverhead = 4 + MaxInputs * 41;
  auto ProviderBytes =
      Decoder.field(10, RemainingInputBytes + MaxProviderOverhead);
  if (!ProviderBytes)
    return ProviderBytes.takeError();
  auto Providers = decodeInputs(*ProviderBytes, true, RemainingInputBytes);
  if (!Providers)
    return Providers.takeError();
  if (Providers->size() + 1 > MaxInputs)
    return protocolError("invalid worker input count");
  for (const Input &Provider : *Providers)
    if (Provider.Digest == Result.CompilerModule.Digest &&
        Provider.Bytes.size() == Result.CompilerModule.Bytes.size())
      return protocolError("compiler module is duplicated as a provider");
  Result.ExternalProviders = std::move(*Providers);

  constexpr size_t MaxSymbolField = MaxSymbols * (MaxSymbolBytes + 4) + 4;
  auto ImportBytes = Decoder.field(11, MaxSymbolField);
  if (!ImportBytes)
    return ImportBytes.takeError();
  auto Imports = decodeStrings(*ImportBytes, MaxSymbols, MaxSymbolBytes,
                               MaxSymbols * MaxSymbolBytes);
  if (!Imports)
    return Imports.takeError();
  if (Error E = validateSymbols(*Imports))
    return E;
  Result.ImportSymbols = std::move(*Imports);

  auto ExportBytes = Decoder.field(12, MaxSymbolField);
  if (!ExportBytes)
    return ExportBytes.takeError();
  auto Exports = decodeStrings(*ExportBytes, MaxSymbols, MaxSymbolBytes,
                               MaxSymbols * MaxSymbolBytes);
  if (!Exports)
    return Exports.takeError();
  if (Error E = validateSymbols(*Exports))
    return E;
  Result.ExportSymbols = std::move(*Exports);

  auto FinalBytes = Decoder.field(13, MaxSymbolField);
  if (!FinalBytes)
    return FinalBytes.takeError();
  auto FinalSymbols = decodeStrings(*FinalBytes, MaxSymbols, MaxSymbolBytes,
                                    MaxSymbols * MaxSymbolBytes);
  if (!FinalSymbols)
    return FinalSymbols.takeError();
  if (Error E = validateSymbols(*FinalSymbols))
    return E;
  if (FinalSymbols->empty())
    return protocolError("V2 final symbol closure is empty");
  Result.FinalSymbols = std::move(*FinalSymbols);

  for (const std::string &Import : Result.ImportSymbols) {
    if (!std::binary_search(Result.FinalSymbols.begin(),
                            Result.FinalSymbols.end(), Import))
      return protocolError("V2 import is absent from final symbol closure");
    if (std::binary_search(Result.ExportSymbols.begin(),
                           Result.ExportSymbols.end(), Import))
      return protocolError("V2 symbol is both imported and exported");
  }
  for (const std::string &Export : Result.ExportSymbols)
    if (!std::binary_search(Result.FinalSymbols.begin(),
                            Result.FinalSymbols.end(), Export))
      return protocolError("V2 export is absent from final symbol closure");

  auto OutputBoundBytes = Decoder.field(14, 8);
  if (!OutputBoundBytes)
    return OutputBoundBytes.takeError();
  if (OutputBoundBytes->size() != 8)
    return protocolError("invalid output bound field");
  Result.MaxOutputBytes = support::endian::read64le(OutputBoundBytes->data());
  if (Result.MaxOutputBytes == 0 || Result.MaxOutputBytes > MaxOutputBytes)
    return protocolError("invalid output byte bound");

  size_t IdentityFieldOffset = Decoder.position();
  auto IdentityBytes = Decoder.field(15, 32);
  if (!IdentityBytes)
    return IdentityBytes.takeError();
  auto Identity = fixed<32>(*IdentityBytes);
  if (!Identity)
    return Identity.takeError();
  Result.Identity = *Identity;
  if (Error E = Decoder.finish(15))
    return E;

  SHA256 Hasher;
  Hasher.update(StringRef(RequestDomainV2, sizeof(RequestDomainV2) - 1));
  uint8_t LengthBytes[8];
  support::endian::write64le(LengthBytes, IdentityFieldOffset);
  Hasher.update(ArrayRef<uint8_t>(LengthBytes));
  Hasher.update(Bytes.take_front(IdentityFieldOffset));
  if (Hasher.final() != Result.Identity)
    return protocolError("worker V2 request identity mismatch");

  Result.Inputs = Result.ExternalProviders;
  Result.Inputs.push_back(Result.CompilerModule);
  llvm::sort(Result.Inputs, [](const Input &Left, const Input &Right) {
    return std::tuple(Left.Digest, Left.Bytes.size(), Left.Kind) <
           std::tuple(Right.Digest, Right.Bytes.size(), Right.Kind);
  });
  Result.RequiredSymbols = Result.FinalSymbols;
  Result.ExpectedDefinedSymbols = Result.FinalSymbols;
  return Result;
}

Expected<std::vector<uint8_t>> encodeResponse(Response Value) {
  if (Value.WorkerBuildIdentity.empty() ||
      Value.WorkerBuildIdentity.size() > MaxBuildIdentityBytes)
    return protocolError("invalid worker build identity");
  for (unsigned char Byte : Value.WorkerBuildIdentity)
    if (Byte > 0x7f || Byte < 0x20 || Byte == 0x7f)
      return protocolError("noncanonical worker build identity");
  Value.Diagnostics = canonicalDiagnostics(Value.Diagnostics);
  bool Success = Value.LinkedOutput.has_value();
  if (Success != (Value.FailureStage == Stage::Complete))
    return protocolError("invalid worker response state");
  if (Success) {
    if (Value.LinkedOutput->Bytes.empty() ||
        Value.LinkedOutput->Bytes.size() > MaxOutputBytes)
      return protocolError("invalid worker output size");
    if (SHA256::hash(Value.LinkedOutput->Bytes) != Value.LinkedOutput->Digest)
      return protocolError("worker output digest mismatch");
  }

  if (Value.Protocol == ProtocolVersion::V2 &&
      llvm::all_of(Value.CompilerEnvelopeIdentity,
                   [](uint8_t Byte) { return Byte == 0; }))
    return protocolError("V2 response has no compiler envelope identity");

  std::vector<uint8_t> Encoded;
  if (Value.Protocol == ProtocolVersion::V1)
    Encoded.assign(std::begin(ResponseMagicV1), std::end(ResponseMagicV1));
  else if (Value.Protocol == ProtocolVersion::V2)
    Encoded.assign(std::begin(ResponseMagicV2), std::end(ResponseMagicV2));
  else
    return protocolError("unsupported response protocol version");
  if (Error E = appendField(Encoded, 1, Value.RequestId))
    return E;
  if (Error E = appendField(Encoded, 2, Value.RequestIdentity))
    return E;
  uint16_t Offset = 0;
  if (Value.Protocol == ProtocolVersion::V2) {
    if (Error E = appendField(Encoded, 3, Value.CompilerEnvelopeIdentity))
      return E;
    Offset = 1;
  }
  if (Error E =
          appendField(Encoded, 3 + Offset,
                      ArrayRef<uint8_t>(reinterpret_cast<const uint8_t *>(
                                            Value.WorkerBuildIdentity.data()),
                                        Value.WorkerBuildIdentity.size())))
    return E;
  uint8_t StageByte = static_cast<uint8_t>(Value.FailureStage);
  if (Error E =
          appendField(Encoded, 4 + Offset, ArrayRef<uint8_t>(&StageByte, 1)))
    return E;
  std::vector<uint8_t> DiagnosticBytes = encodeStrings(Value.Diagnostics);
  if (Error E = appendField(Encoded, 5 + Offset, DiagnosticBytes))
    return E;

  std::vector<uint8_t> OutputBytes;
  if (!Value.LinkedOutput) {
    OutputBytes.push_back(0);
  } else {
    OutputBytes.push_back(1);
    OutputBytes.insert(OutputBytes.end(), Value.LinkedOutput->Digest.begin(),
                       Value.LinkedOutput->Digest.end());
    appendU64(OutputBytes, Value.LinkedOutput->Bytes.size());
    OutputBytes.insert(OutputBytes.end(), Value.LinkedOutput->Bytes.begin(),
                       Value.LinkedOutput->Bytes.end());
  }
  if (Error E = appendField(Encoded, 6 + Offset, OutputBytes))
    return E;
  return Encoded;
}

std::vector<std::string> canonicalDiagnostics(ArrayRef<std::string> Diagnostics,
                                              StringRef InternalPath) {
  std::vector<std::string> Result;
  size_t Total = 0;
  for (StringRef Diagnostic : Diagnostics) {
    std::string Value = Diagnostic.str();
    if (!InternalPath.empty()) {
      size_t Position = 0;
      while ((Position = Value.find(InternalPath.str(), Position)) !=
             std::string::npos) {
        Value.replace(Position, InternalPath.size(), "<internal>");
        Position += 10;
      }
    }
    std::string Sanitized;
    Sanitized.reserve(std::min(Value.size(), MaxDiagnosticBytes));
    for (unsigned char Byte : Value) {
      char OutputByte = (Byte >= 0x20 && Byte <= 0x7e) ? Byte : ' ';
      if (Sanitized.empty() || OutputByte != ' ' || Sanitized.back() != ' ')
        Sanitized.push_back(OutputByte);
      if (Sanitized.size() == MaxDiagnosticBytes)
        break;
    }
    while (!Sanitized.empty() && Sanitized.back() == ' ')
      Sanitized.pop_back();
    if (Sanitized.empty())
      continue;
    if (Total + Sanitized.size() > MaxTotalDiagnosticBytes)
      continue;
    Total += Sanitized.size();
    Result.push_back(std::move(Sanitized));
  }
  llvm::sort(Result);
  Result.erase(std::unique(Result.begin(), Result.end()), Result.end());
  if (Result.size() > MaxDiagnostics)
    Result.resize(MaxDiagnostics);
  return Result;
}

std::string errorToDiagnostic(Error ErrorValue) {
  return toString(std::move(ErrorValue));
}

} // namespace fe2o3::worker
