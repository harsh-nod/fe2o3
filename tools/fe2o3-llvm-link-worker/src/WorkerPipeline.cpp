#include "WorkerPipeline.h"
#include "WorkerLldPolicy.h"

#include "lld/Common/Driver.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/Magic.h"
#include "llvm/Bitcode/BitcodeReader.h"
#include "llvm/IR/AutoUpgrade.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DebugInfo.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Linker/Linker.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Passes/OptimizationLevel.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/Path.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include "llvm/Transforms/Utils/ModuleUtils.h"

#include <algorithm>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <system_error>
#include <tuple>

using namespace llvm;
using namespace llvm::object;

LLD_HAS_DRIVER(elf)

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

namespace fe2o3::worker {
namespace {

constexpr StringLiteral LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
constexpr StringLiteral WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
constexpr StringLiteral AmdGpuTriple = "amdgcn-amd-amdhsa";

Response failure(const Request &RequestValue, Stage FailureStage,
                 std::vector<std::string> Diagnostics) {
  Response Result{RequestValue.RequestId,
                  RequestValue.Identity,
                  WorkerBuildIdentity.str(),
                  FailureStage,
                  canonicalDiagnostics(Diagnostics),
                  std::nullopt};
  Result.Protocol = RequestValue.Protocol;
  Result.CompilerEnvelopeIdentity = RequestValue.CompilerEnvelopeIdentity;
  return Result;
}

Response failure(const Request &RequestValue, Stage FailureStage,
                 Error ErrorValue) {
  return failure(RequestValue, FailureStage,
                 {errorToDiagnostic(std::move(ErrorValue))});
}

Error pipelineError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(), Message);
}

class BoundedRawStream final : public raw_ostream {
public:
  explicit BoundedRawStream(size_t Limit) : Limit(Limit) { SetUnbuffered(); }

  StringRef str() const { return Buffer; }
  bool truncated() const { return Truncated; }

private:
  void write_impl(const char *Pointer, size_t Size) override {
    Position = Size > std::numeric_limits<uint64_t>::max() - Position
                   ? std::numeric_limits<uint64_t>::max()
                   : Position + Size;
    size_t Remaining = Limit - Buffer.size();
    size_t Accepted = std::min(Remaining, Size);
    Buffer.append(Pointer, Accepted);
    Truncated |= Accepted != Size;
  }

  uint64_t current_pos() const override { return Position; }

  std::string Buffer;
  size_t Limit;
  uint64_t Position = 0;
  bool Truncated = false;
};

struct TargetParts {
  std::string Cpu;
  std::optional<bool> SramEcc;
  std::optional<bool> Xnack;
};

TargetParts parseTarget(StringRef Target) {
  SmallVector<StringRef, 3> Components;
  Target.split(Components, ':', -1, false);
  TargetParts Result;
  Result.Cpu = Components.front().str();
  for (StringRef Component : drop_begin(Components)) {
    bool Enabled = Component.back() == '+';
    if (Component.drop_back() == "sramecc")
      Result.SramEcc = Enabled;
    else if (Component.drop_back() == "xnack")
      Result.Xnack = Enabled;
  }
  return Result;
}

Error validateRequest(const Request &RequestValue) {
  switch (RequestValue.LinkOptions.Optimization) {
  case OptimizationLevel::O0:
  case OptimizationLevel::O1:
  case OptimizationLevel::O2:
  case OptimizationLevel::O3:
    break;
  default:
    return pipelineError("unsupported optimization level");
  }
  if (RequestValue.CodeObjectVersion != 4 &&
      RequestValue.CodeObjectVersion != 5 &&
      RequestValue.CodeObjectVersion != 6)
    return pipelineError("unsupported code-object version");
  if (RequestValue.MaxOutputBytes == 0 ||
      RequestValue.MaxOutputBytes > MaxOutputBytes)
    return pipelineError("invalid output byte bound");
  if (RequestValue.Inputs.empty() || RequestValue.Inputs.size() > MaxInputs)
    return pipelineError("invalid worker input count");

  size_t Total = 0;
  for (size_t I = 0; I < RequestValue.Inputs.size(); ++I) {
    const Input &InputValue = RequestValue.Inputs[I];
    if (InputValue.Bytes.empty() ||
        InputValue.Bytes.size() > MaxTotalInputBytes ||
        Total > MaxTotalInputBytes - InputValue.Bytes.size())
      return pipelineError(Twine("input ") + Twine(I) +
                           " violates the byte bound");
    Total += InputValue.Bytes.size();
    if (SHA256::hash(InputValue.Bytes) != InputValue.Digest)
      return pipelineError(Twine("input ") + Twine(I) +
                           " digest does not match its bytes");
    StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                    InputValue.Bytes.size());
    file_magic Expected;
    switch (InputValue.Kind) {
    case InputKind::LlvmBitcode:
      Expected = file_magic::bitcode;
      break;
    case InputKind::AmdGpuRelocatable:
      Expected = file_magic::elf_relocatable;
      break;
    default:
      return pipelineError(Twine("input ") + Twine(I) +
                           " has an unsupported input kind");
    }
    if (identify_magic(Bytes) != Expected)
      return pipelineError(Twine("input ") + Twine(I) +
                           " has a file kind different from its declaration");
  }
  for (size_t I = 1; I < RequestValue.Inputs.size(); ++I) {
    const Input &Before = RequestValue.Inputs[I - 1];
    const Input &After = RequestValue.Inputs[I];
    auto BeforeKey =
        std::tuple(Before.Digest, Before.Bytes.size(), Before.Kind);
    auto AfterKey = std::tuple(After.Digest, After.Bytes.size(), After.Kind);
    if (BeforeKey >= AfterKey)
      return pipelineError("worker inputs are duplicate or noncanonical");
  }
  return Error::success();
}

uint8_t expectedAbiVersion(uint8_t CodeObjectVersion) {
  switch (CodeObjectVersion) {
  case 4:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V4;
  case 5:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V5;
  case 6:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V6;
  }
  llvm_unreachable("validated code-object version");
}

CodeGenOptLevel codegenLevel(OptimizationLevel Level) {
  switch (Level) {
  case OptimizationLevel::O0:
    return CodeGenOptLevel::None;
  case OptimizationLevel::O1:
    return CodeGenOptLevel::Less;
  case OptimizationLevel::O2:
    return CodeGenOptLevel::Default;
  case OptimizationLevel::O3:
    return CodeGenOptLevel::Aggressive;
  }
  llvm_unreachable("validated optimization level");
}

llvm::OptimizationLevel irLevel(OptimizationLevel Level) {
  switch (Level) {
  case OptimizationLevel::O0:
    return llvm::OptimizationLevel::O0;
  case OptimizationLevel::O1:
    return llvm::OptimizationLevel::O1;
  case OptimizationLevel::O2:
    return llvm::OptimizationLevel::O2;
  case OptimizationLevel::O3:
    return llvm::OptimizationLevel::O3;
  }
  llvm_unreachable("validated optimization level");
}

Expected<std::unique_ptr<TargetMachine>>
createMachine(const Request &RequestValue) {
  static bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUAsmPrinter();
    LLVMInitializeAMDGPUAsmParser();
    return true;
  }();
  (void)Initialized;

  Triple TripleValue(AmdGpuTriple);
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  if (!TargetValue)
    return pipelineError(Twine("AMDGPU target lookup failed: ") + LookupError);
  TargetParts Parts = parseTarget(RequestValue.Target);
  TargetOptions OptionsValue;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, Parts.Cpu, "", OptionsValue, Reloc::PIC_, CodeModel::Small,
      codegenLevel(RequestValue.LinkOptions.Optimization)));
  if (!Machine)
    return pipelineError("AMDGPU target-machine creation failed");
  return Machine;
}

Error setAndCheckModuleContract(Module &ModuleValue,
                                const Request &RequestValue,
                                const TargetMachine &Machine) {
  TargetParts Parts = parseTarget(RequestValue.Target);
  const Triple &ExistingTriple = ModuleValue.getTargetTriple();
  if (!ExistingTriple.getTriple().empty() &&
      Triple::normalize(ExistingTriple.getTriple()) != AmdGpuTriple)
    return pipelineError("bitcode target triple does not match AMDHSA");
  if (!ModuleValue.getDataLayoutStr().empty() &&
      ModuleValue.getDataLayout() != Machine.createDataLayout())
    return pipelineError(
        Twine("bitcode data layout does not match target machine: '") +
        ModuleValue.getDataLayoutStr() + "' != '" +
        Machine.createDataLayout().getStringRepresentation() + "'");
  Metadata *ExistingCodeObject =
      ModuleValue.getModuleFlag("amdhsa_code_object_version");
  if (ExistingCodeObject) {
    auto *Constant = mdconst::dyn_extract<ConstantInt>(ExistingCodeObject);
    if (!Constant ||
        Constant->getZExtValue() !=
            static_cast<uint64_t>(RequestValue.CodeObjectVersion) * 100)
      return pipelineError(
          "bitcode code-object version does not match request");
  }
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine.createDataLayout());
  if (!ExistingCodeObject)
    ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                              RequestValue.CodeObjectVersion * 100);

  for (StringRef FlagName : {StringRef("sramecc"), StringRef("xnack")}) {
    std::optional<uint32_t> Requested =
        FlagName == "sramecc" ? std::optional<uint32_t>(Parts.SramEcc)
                              : std::optional<uint32_t>(Parts.Xnack);
    std::string ModuleFlag = (Twine("amdgpu.") + FlagName).str();
    Metadata *Existing = ModuleValue.getModuleFlag(ModuleFlag);
    if (!Requested) {
      if (Existing)
        return pipelineError(Twine("bitcode ") + ModuleFlag +
                             " constraint is absent from request target");
      continue;
    }
    if (Existing) {
      auto *Constant = mdconst::dyn_extract<ConstantInt>(Existing);
      if (!Constant || Constant->getZExtValue() != *Requested)
        return pipelineError(Twine("bitcode ") + ModuleFlag +
                             " does not match request target");
    } else {
      ModuleValue.addModuleFlag(Module::Error, ModuleFlag, *Requested);
    }
  }

  for (const Function &FunctionValue : ModuleValue) {
    Attribute Cpu = FunctionValue.getFnAttribute("target-cpu");
    if (Cpu.isStringAttribute() && Cpu.getValueAsString() != Parts.Cpu)
      return pipelineError(Twine("bitcode function '") +
                           FunctionValue.getName() +
                           "' target CPU does not match request");
    Attribute Features = FunctionValue.getFnAttribute("target-features");
    if (!Features.isStringAttribute())
      continue;
    SmallVector<StringRef, 32> FeatureParts;
    Features.getValueAsString().split(FeatureParts, ',', -1, true);
    SmallString<256> CheckedFeatures;
    for (StringRef Feature : FeatureParts) {
      if (Feature.size() < 2 ||
          (Feature.front() != '+' && Feature.front() != '-'))
        return pipelineError(Twine("bitcode function '") +
                             FunctionValue.getName() +
                             "' has a malformed target feature");
      StringRef Name = Feature.drop_front();
      std::optional<bool> Requested;
      if (Name == "sramecc")
        Requested = Parts.SramEcc;
      else if (Name == "xnack")
        Requested = Parts.Xnack;
      if (Requested || Name == "sramecc" || Name == "xnack") {
        if (!Requested || *Requested != (Feature.front() == '+'))
          return pipelineError(Twine("bitcode function '") +
                               FunctionValue.getName() + "' " + Name +
                               " feature does not match request");
        continue;
      }
      ArrayRef<SubtargetFeatureKV> KnownFeatures =
          Machine.getMCSubtargetInfo()->getAllProcessorFeatures();
      auto Known = llvm::lower_bound(KnownFeatures, Name);
      if (Known == KnownFeatures.end() || StringRef(Known->Key) != Name)
        return pipelineError(Twine("bitcode function '") +
                             FunctionValue.getName() +
                             "' names an unknown target feature");
      if (!CheckedFeatures.empty())
        CheckedFeatures.push_back(',');
      CheckedFeatures.append(Feature);
    }
    if (!CheckedFeatures.empty() &&
        !Machine.getMCSubtargetInfo()->checkFeatures(CheckedFeatures))
      return pipelineError(Twine("bitcode function '") +
                           FunctionValue.getName() +
                           "' target features are incompatible with target");
  }
  return Error::success();
}

Expected<std::unique_ptr<Module>> linkBitcode(const Request &RequestValue,
                                              LLVMContext &Context,
                                              const TargetMachine &Machine) {
  std::unique_ptr<Module> Linked;
  for (size_t I = 0; I < RequestValue.Inputs.size(); ++I) {
    const Input &InputValue = RequestValue.Inputs[I];
    if (InputValue.Kind != InputKind::LlvmBitcode)
      continue;
    StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                    InputValue.Bytes.size());
    std::string InputName = (Twine("<bitcode-input-") + Twine(I) + ">").str();
    DataLayout ExpectedLayout = Machine.createDataLayout();
    bool AcceptedLayout = false;
    ParserCallbacks Callbacks([&](StringRef TripleValue, StringRef Layout) {
      std::string InferredAbsent = UpgradeDataLayoutString("", TripleValue);
      if (Layout == InferredAbsent) {
        AcceptedLayout = true;
      } else {
        auto ParsedLayout = DataLayout::parse(Layout);
        if (ParsedLayout)
          AcceptedLayout = *ParsedLayout == ExpectedLayout;
        else
          consumeError(ParsedLayout.takeError());
      }
      return std::optional<std::string>(
          ExpectedLayout.getStringRepresentation());
    });
    auto Parsed = parseBitcodeFile(MemoryBufferRef(Bytes, InputName), Context,
                                   std::move(Callbacks));
    if (!Parsed)
      return pipelineError(Twine("bitcode input ") + Twine(I) + ": " +
                           errorToDiagnostic(Parsed.takeError()));
    if (!AcceptedLayout)
      return pipelineError("bitcode data layout does not match target machine");
    if (Error E = setAndCheckModuleContract(**Parsed, RequestValue, Machine))
      return E;
    if (RequestValue.LinkOptions.VerifyEach) {
      BoundedRawStream Stream(MaxDiagnosticBytes);
      if (verifyModule(**Parsed, &Stream)) {
        Stream.flush();
        return pipelineError(Twine("input bitcode verification failed: ") +
                             Stream.str());
      }
    }
    if (!Linked) {
      Linked = std::move(*Parsed);
    } else if (Linker::linkModules(*Linked, std::move(*Parsed))) {
      return pipelineError("LLVM bitcode linking failed");
    }
  }
  if (Linked) {
    BoundedRawStream Stream(MaxDiagnosticBytes);
    if (verifyModule(*Linked, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("linked bitcode verification failed: ") +
                           Stream.str());
    }
  }
  return Linked;
}

Error optimizeModule(Module &ModuleValue, const Request &RequestValue,
                     TargetMachine &Machine) {
  if (RequestValue.LinkOptions.StripDebug)
    StripDebugInfo(ModuleValue);
  SmallVector<GlobalValue *, 16> PreservedExports;
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    if (GlobalValue *Value = ModuleValue.getNamedValue(Name);
        Value && !Value->isDeclaration()) {
      if (Value->hasLocalLinkage() ||
          Value->getVisibility() != GlobalValue::DefaultVisibility)
        return pipelineError(Twine("requested bitcode export '") + Name +
                             "' is not externally visible");
      PreservedExports.push_back(Value);
    }
  if (!PreservedExports.empty())
    appendToUsed(ModuleValue, PreservedExports);
  PassBuilder Builder(&Machine);
  LoopAnalysisManager Loops;
  FunctionAnalysisManager Functions;
  CGSCCAnalysisManager Cgscc;
  ModuleAnalysisManager Modules;
  Builder.registerModuleAnalyses(Modules);
  Builder.registerCGSCCAnalyses(Cgscc);
  Builder.registerFunctionAnalyses(Functions);
  Builder.registerLoopAnalyses(Loops);
  Builder.crossRegisterProxies(Loops, Functions, Cgscc, Modules);
  ModulePassManager Pipeline = Builder.buildPerModuleDefaultPipeline(
      irLevel(RequestValue.LinkOptions.Optimization));
  Pipeline.run(ModuleValue, Modules);
  BoundedRawStream Stream(MaxDiagnosticBytes);
  if (verifyModule(ModuleValue, &Stream)) {
    Stream.flush();
    return pipelineError(Twine("optimized bitcode verification failed: ") +
                         Stream.str());
  }
  return Error::success();
}

Expected<std::vector<uint8_t>> emitObject(Module &ModuleValue,
                                          TargetMachine &Machine) {
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  legacy::PassManager Passes;
  if (Machine.addPassesToEmitFile(Passes, Stream, nullptr,
                                  CodeGenFileType::ObjectFile, false))
    return pipelineError("AMDGPU target does not support object emission");
  Passes.run(ModuleValue);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

struct ElfContract {
  uint32_t Flags;
  uint8_t OsAbi;
  uint8_t AbiVersion;
  uint8_t AddressBytes;
  bool LittleEndian;
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::set<std::string> RequiredImports;
};

bool matches(const ElfContract &Left, const ElfContract &Right) {
  return Left.Flags == Right.Flags && Left.OsAbi == Right.OsAbi &&
         Left.AbiVersion == Right.AbiVersion &&
         Left.AddressBytes == Right.AddressBytes &&
         Left.LittleEndian == Right.LittleEndian;
}

Expected<ElfContract> inspectRelocatable(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<object>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  if (!Elf || Elf->getEMachine() != ELF::EM_AMDGPU ||
      Elf->getEType() != ELF::ET_REL)
    return pipelineError("input is not an AMDGPU ELF relocatable");
  if (Bytes.size() < ELF::EI_NIDENT ||
      Bytes[ELF::EI_CLASS] != ELF::ELFCLASS64 ||
      Bytes[ELF::EI_DATA] != ELF::ELFDATA2LSB ||
      Bytes[ELF::EI_VERSION] != ELF::EV_CURRENT ||
      Elf->getBytesInAddress() != 8 || !Elf->isLittleEndian())
    return pipelineError("AMDGPU relocatable is not 64-bit little-endian ELF");
  uint8_t OsAbi = Bytes[ELF::EI_OSABI];
  if (OsAbi != ELF::ELFOSABI_AMDGPU_HSA)
    return pipelineError("AMDGPU relocatable does not use the AMDHSA OS ABI");

  ElfContract Result{Elf->getPlatformFlags(),
                     OsAbi,
                     Elf->getEIdentABIVersion(),
                     Elf->getBytesInAddress(),
                     Elf->isLittleEndian(),
                     {},
                     {},
                     {}};
  for (SymbolRef Symbol : Elf->symbols()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    uint32_t Flags = *FlagsOrError;
    if ((Flags & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0 ||
        (Flags & SymbolRef::SF_FormatSpecific) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (NameOrError->empty())
      continue;
    std::string Name = NameOrError->str();
    if ((Flags & SymbolRef::SF_Undefined) != 0) {
      Result.RequiredImports.insert(std::move(Name));
    } else {
      Result.Definitions.insert(Name);
      if ((Flags & SymbolRef::SF_Hidden) == 0)
        Result.PublicDefinitions.insert(std::move(Name));
    }
  }
  return Result;
}

Expected<std::set<std::string>> inspectOutput(ArrayRef<uint8_t> Bytes,
                                              const ElfContract &Expected) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  if (!Elf || Elf->getEMachine() != ELF::EM_AMDGPU ||
      Elf->getEType() != ELF::ET_DYN)
    return pipelineError("linked output is not an AMDGPU shared ELF");
  if (Bytes.size() < ELF::EI_NIDENT ||
      Bytes[ELF::EI_CLASS] != ELF::ELFCLASS64 ||
      Bytes[ELF::EI_DATA] != ELF::ELFDATA2LSB ||
      Bytes[ELF::EI_VERSION] != ELF::EV_CURRENT ||
      Elf->getBytesInAddress() != 8 || !Elf->isLittleEndian())
    return pipelineError("linked output has an invalid AMDGPU ELF envelope");
  if (Elf->getPlatformFlags() != Expected.Flags ||
      Bytes[ELF::EI_OSABI] != Expected.OsAbi ||
      Elf->getEIdentABIVersion() != Expected.AbiVersion ||
      Elf->getBytesInAddress() != Expected.AddressBytes ||
      Elf->isLittleEndian() != Expected.LittleEndian)
    return pipelineError(
        "linked output target or code-object version mismatch");

  std::set<std::string> Defined;
  for (SymbolRef Symbol : Elf->getDynamicSymbolIterators()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    if ((*FlagsOrError & SymbolRef::SF_Undefined) != 0)
      return pipelineError("linked output retains an unresolved import");
    if ((*FlagsOrError & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0 ||
        (*FlagsOrError &
         (SymbolRef::SF_FormatSpecific | SymbolRef::SF_Hidden)) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (!NameOrError->empty())
      Defined.insert(NameOrError->str());
  }
  return Defined;
}

Error validateSymbolClosure(ArrayRef<ElfContract> Contracts,
                            const Request &RequestValue) {
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::map<std::string, size_t> DefinitionCounts;
  for (const ElfContract &Contract : Contracts) {
    Definitions.insert(Contract.Definitions.begin(),
                       Contract.Definitions.end());
    PublicDefinitions.insert(Contract.PublicDefinitions.begin(),
                             Contract.PublicDefinitions.end());
    for (const std::string &Name : Contract.Definitions)
      ++DefinitionCounts[Name];
  }
  for (const auto &[Name, Count] : DefinitionCounts)
    if (Count > 1)
      return pipelineError(Twine("duplicate definition: ") + Name);
  for (const ElfContract &Contract : Contracts)
    for (const std::string &Name : Contract.RequiredImports)
      if (!Definitions.contains(Name))
        return pipelineError(Twine("unresolved required import: ") + Name);
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    if (!PublicDefinitions.contains(Name))
      return pipelineError(Twine("requested output has no public provider: ") +
                           Name);
  return Error::success();
}

struct TemporaryDirectory {
  SmallString<128> Path;
  ~TemporaryDirectory() {
    if (!Path.empty()) {
      std::error_code ErrorCode = sys::fs::remove_directories(Path);
      if (ErrorCode)
        consumeError(errorCodeToError(ErrorCode));
    }
  }
};

Error writeBytes(StringRef Path, ArrayRef<uint8_t> Bytes) {
  std::error_code ErrorCode;
  raw_fd_ostream Stream(Path, ErrorCode, sys::fs::OF_None);
  if (ErrorCode)
    return errorCodeToError(ErrorCode);
  Stream.write(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  Stream.close();
  if (Stream.has_error())
    return pipelineError("failed to write private linker input");
  return Error::success();
}

Expected<std::vector<uint8_t>> readBytes(StringRef Path, uint64_t MaxBytes) {
  sys::fs::file_status Status;
  if (std::error_code ErrorCode = sys::fs::status(Path, Status))
    return errorCodeToError(ErrorCode);
  if (!sys::fs::is_regular_file(Status) || Status.getSize() == 0 ||
      Status.getSize() > MaxBytes)
    return pipelineError("linked output violates byte bound");
  auto BufferOrError = MemoryBuffer::getFile(Path, false, false);
  if (!BufferOrError)
    return errorCodeToError(BufferOrError.getError());
  StringRef Bytes = (*BufferOrError)->getBuffer();
  if (Bytes.size() != Status.getSize())
    return pipelineError("linked output changed while it was inspected");
  return std::vector<uint8_t>(Bytes.bytes_begin(), Bytes.bytes_end());
}

Expected<std::vector<uint8_t>>
nativeLink(ArrayRef<std::vector<uint8_t>> Objects, const Request &RequestValue,
           StringRef Directory, std::vector<std::string> &Diagnostics) {
  std::vector<std::string> Paths;
  Paths.reserve(Objects.size());
  for (size_t I = 0; I < Objects.size(); ++I) {
    SmallString<160> Path(Directory);
    sys::path::append(Path, (Twine("input-") + Twine(I) + ".o").str());
    if (Error E = writeBytes(Path, Objects[I]))
      return E;
    Paths.push_back(Path.str().str());
  }
  SmallString<160> OutputPath(Directory);
  sys::path::append(OutputPath, "linked.hsaco");

  std::vector<std::string> OwnedArguments = {"ld.lld",
                                             "--shared",
                                             "--no-undefined",
                                             "--export-dynamic",
                                             "--build-id=none",
                                             "--nostdlib",
                                             "--no-dependent-libraries",
                                             "--fatal-warnings",
                                             "--threads=1"};
  if (RequestValue.LinkOptions.StripDebug)
    OwnedArguments.push_back("--strip-debug");
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    OwnedArguments.push_back((Twine("--undefined=") + Name).str());
  OwnedArguments.insert(OwnedArguments.end(), Paths.begin(), Paths.end());
  OwnedArguments.push_back("-o");
  OwnedArguments.push_back(OutputPath.str().str());
  std::vector<const char *> Arguments;
  Arguments.reserve(OwnedArguments.size());
  for (const std::string &Argument : OwnedArguments)
    Arguments.push_back(Argument.c_str());

  BoundedRawStream OutputStream(MaxTotalDiagnosticBytes);
  BoundedRawStream ErrorStream(MaxTotalDiagnosticBytes);
  lld::Result LinkResult = lld::lldMain(Arguments, OutputStream, ErrorStream,
                                        {{lld::Gnu, &lld::elf::link}});
  detail::enforceReusableLldResult(LinkResult);
  OutputStream.flush();
  ErrorStream.flush();
  if (!OutputStream.str().empty())
    Diagnostics.push_back(OutputStream.str().str());
  if (OutputStream.truncated())
    Diagnostics.push_back("LLD stdout exceeded diagnostic byte bound");
  if (!ErrorStream.str().empty())
    Diagnostics.push_back(ErrorStream.str().str());
  if (ErrorStream.truncated())
    Diagnostics.push_back("LLD stderr exceeded diagnostic byte bound");
  if (LinkResult.retCode != 0)
    return pipelineError("LLD ELF link failed");
  return readBytes(OutputPath, RequestValue.MaxOutputBytes);
}

} // namespace

Response execute(const Request &RequestValue) {
  if (RequestValue.LlvmBuildIdentity != LlvmBuildIdentity)
    return failure(RequestValue, Stage::Toolchain,
                   {"request LLVM identity does not match worker measurement"});
  if (RequestValue.Protocol == ProtocolVersion::V2 &&
      RequestValue.WorkerBuildIdentity != WorkerBuildIdentity)
    return failure(
        RequestValue, Stage::Toolchain,
        {"request worker identity does not match worker measurement"});
  if (Error E = validateRequest(RequestValue))
    return failure(RequestValue, Stage::InputValidation, std::move(E));

  auto MachineOrError = createMachine(RequestValue);
  if (!MachineOrError)
    return failure(RequestValue, Stage::Toolchain, MachineOrError.takeError());
  std::unique_ptr<TargetMachine> Machine = std::move(*MachineOrError);

  LLVMContext Context;
  auto LinkedModule = linkBitcode(RequestValue, Context, *Machine);
  if (!LinkedModule)
    return failure(RequestValue, Stage::BitcodeLink, LinkedModule.takeError());

  Module Reference("fe2o3-target-reference", Context);
  if (Error E = setAndCheckModuleContract(Reference, RequestValue, *Machine))
    return failure(RequestValue, Stage::Toolchain, std::move(E));
  auto ReferenceObject = emitObject(Reference, *Machine);
  if (!ReferenceObject)
    return failure(RequestValue, Stage::Codegen, ReferenceObject.takeError());
  auto ExpectedElf = inspectRelocatable(*ReferenceObject);
  if (!ExpectedElf)
    return failure(RequestValue, Stage::Codegen, ExpectedElf.takeError());

  if (ExpectedElf->AbiVersion !=
      expectedAbiVersion(RequestValue.CodeObjectVersion))
    return failure(RequestValue, Stage::Codegen,
                   {"target machine emitted the wrong code-object version"});

  std::vector<std::vector<uint8_t>> Objects;
  std::vector<ElfContract> ObjectContracts;
  for (const Input &InputValue : RequestValue.Inputs) {
    if (InputValue.Kind != InputKind::AmdGpuRelocatable)
      continue;
    auto Contract = inspectRelocatable(InputValue.Bytes);
    if (!Contract)
      return failure(RequestValue, Stage::InputValidation,
                     Contract.takeError());
    if (!matches(*Contract, *ExpectedElf))
      return failure(RequestValue, Stage::InputValidation,
                     {"native input target or code-object version mismatch"});
    Objects.push_back(InputValue.Bytes);
    ObjectContracts.push_back(std::move(*Contract));
  }

  if (*LinkedModule) {
    if (Error E = optimizeModule(**LinkedModule, RequestValue, *Machine))
      return failure(RequestValue, Stage::Optimization, std::move(E));
    auto GeneratedObject = emitObject(**LinkedModule, *Machine);
    if (!GeneratedObject)
      return failure(RequestValue, Stage::Codegen, GeneratedObject.takeError());
    auto Contract = inspectRelocatable(*GeneratedObject);
    if (!Contract)
      return failure(RequestValue, Stage::Codegen, Contract.takeError());
    if (!matches(*Contract, *ExpectedElf))
      return failure(RequestValue, Stage::Codegen,
                     {"generated object target contract mismatch"});
    Objects.push_back(std::move(*GeneratedObject));
    ObjectContracts.push_back(std::move(*Contract));
  }
  if (Objects.empty())
    return failure(RequestValue, Stage::InputValidation,
                   {"request produced no native link inputs"});
  if (Error E = validateSymbolClosure(ObjectContracts, RequestValue))
    return failure(RequestValue, Stage::InputValidation, std::move(E));

  TemporaryDirectory Temporary;
  if (std::error_code ErrorCode =
          sys::fs::createUniqueDirectory("fe2o3-llvm-link", Temporary.Path))
    return failure(RequestValue, Stage::NativeLink,
                   errorCodeToError(ErrorCode));
  std::vector<std::string> LinkDiagnostics;
  auto LinkedBytes =
      nativeLink(Objects, RequestValue, Temporary.Path, LinkDiagnostics);
  if (!LinkedBytes) {
    LinkDiagnostics.push_back(errorToDiagnostic(LinkedBytes.takeError()));
    return failure(RequestValue, Stage::NativeLink,
                   canonicalDiagnostics(LinkDiagnostics, Temporary.Path));
  }

  auto DefinedSymbols = inspectOutput(*LinkedBytes, *ExpectedElf);
  if (!DefinedSymbols)
    return failure(RequestValue, Stage::OutputInspection,
                   DefinedSymbols.takeError());
  std::set<std::string> ExpectedSymbols(
      RequestValue.ExpectedDefinedSymbols.begin(),
      RequestValue.ExpectedDefinedSymbols.end());
  if (*DefinedSymbols != ExpectedSymbols)
    return failure(RequestValue, Stage::OutputInspection,
                   {"linked output defined-symbol set mismatch"});
  for (const std::string &Required : RequestValue.RequiredSymbols)
    if (!DefinedSymbols->contains(Required))
      return failure(RequestValue, Stage::OutputInspection,
                     {"linked output is missing a required symbol"});

  Output ResultOutput{SHA256::hash(*LinkedBytes), std::move(*LinkedBytes)};
  Response Result{RequestValue.RequestId,
                  RequestValue.Identity,
                  WorkerBuildIdentity.str(),
                  Stage::Complete,
                  canonicalDiagnostics(LinkDiagnostics, Temporary.Path),
                  std::move(ResultOutput)};
  Result.Protocol = RequestValue.Protocol;
  Result.CompilerEnvelopeIdentity = RequestValue.CompilerEnvelopeIdentity;
  return Result;
}

} // namespace fe2o3::worker
