#include "WorkerPipeline.h"
#include "WorkerDeviceLibraryPolicy.h"
#include "WorkerLldPolicy.h"

#include "lld/Common/Driver.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/Magic.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
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
#include "llvm/Transforms/IPO/GlobalDCE.h"
#include "llvm/Transforms/IPO/Internalize.h"
#include "llvm/Transforms/Utils/ModuleUtils.h"

#include <algorithm>
#include <cstdint>
#include <iomanip>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <sstream>
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
constexpr StringLiteral UnauthenticatedTestWorkerIdentity =
    "fe2o3-unauthenticated-test-device-library-policy";
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

std::string targetMachineFeatures(const TargetParts &Parts) {
  SmallString<32> Features;
  auto Append = [&](StringRef Name, std::optional<bool> Enabled) {
    if (!Enabled)
      return;
    if (!Features.empty())
      Features.push_back(',');
    Features.push_back(*Enabled ? '+' : '-');
    Features.append(Name);
  };
  Append("sramecc", Parts.SramEcc);
  Append("xnack", Parts.Xnack);
  return Features.str().str();
}

uint32_t expectedGfx942Flags(const TargetParts &Parts) {
  uint32_t Flags = ELF::EF_AMDGPU_MACH_AMDGCN_GFX942;
  Flags |= Parts.Xnack ? (*Parts.Xnack ? ELF::EF_AMDGPU_FEATURE_XNACK_ON_V4
                                       : ELF::EF_AMDGPU_FEATURE_XNACK_OFF_V4)
                       : ELF::EF_AMDGPU_FEATURE_XNACK_ANY_V4;
  Flags |= Parts.SramEcc
               ? (*Parts.SramEcc ? ELF::EF_AMDGPU_FEATURE_SRAMECC_ON_V4
                                 : ELF::EF_AMDGPU_FEATURE_SRAMECC_OFF_V4)
               : ELF::EF_AMDGPU_FEATURE_SRAMECC_ANY_V4;
  return Flags;
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
    case InputKind::LlvmTextIr:
      if (llvm::any_of(InputValue.Bytes,
                       [](uint8_t Byte) { return Byte == 0 || Byte > 0x7f; }))
        return pipelineError(Twine("input ") + Twine(I) +
                             " has noncanonical textual LLVM IR bytes");
      continue;
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
  std::string Features = targetMachineFeatures(Parts);
  TargetOptions OptionsValue;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, Parts.Cpu, Features, OptionsValue, Reloc::PIC_,
      CodeModel::Small, codegenLevel(RequestValue.LinkOptions.Optimization)));
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

Expected<std::unique_ptr<Module>>
parseModuleInput(const Input &InputValue, StringRef InputName,
                 const Request &RequestValue, LLVMContext &Context,
                 const TargetMachine &Machine) {
  if (InputValue.Kind != InputKind::LlvmBitcode &&
      InputValue.Kind != InputKind::LlvmTextIr)
    return pipelineError(Twine(InputName) + " is not an LLVM module");
  StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                  InputValue.Bytes.size());
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
    return std::optional<std::string>(ExpectedLayout.getStringRepresentation());
  });
  Expected<std::unique_ptr<Module>> Parsed =
      [&]() -> Expected<std::unique_ptr<Module>> {
    if (InputValue.Kind == InputKind::LlvmBitcode)
      return parseBitcodeFile(MemoryBufferRef(Bytes, InputName), Context,
                              std::move(Callbacks));
    SMDiagnostic Diagnostic;
    auto TextBuffer = MemoryBuffer::getMemBufferCopy(Bytes, InputName);
    auto TextModule =
        parseAssembly(TextBuffer->getMemBufferRef(), Diagnostic, Context);
    if (!TextModule) {
      std::string Message;
      raw_string_ostream Stream(Message);
      Diagnostic.print("fe2o3-llvm-link-worker", Stream, false, false);
      Stream.flush();
      return pipelineError(Twine("textual LLVM IR input ") + InputName + ": " +
                           Message);
    }
    AcceptedLayout = TextModule->getDataLayoutStr().empty() ||
                     TextModule->getDataLayout() == ExpectedLayout;
    return TextModule;
  }();
  if (!Parsed)
    return pipelineError(Twine(InputName) + ": " +
                         errorToDiagnostic(Parsed.takeError()));
  if (!AcceptedLayout)
    return pipelineError(
        "LLVM module data layout does not match target machine");
  if (Error E = setAndCheckModuleContract(**Parsed, RequestValue, Machine))
    return E;
  if (RequestValue.LinkOptions.VerifyEach) {
    BoundedRawStream Stream(MaxDiagnosticBytes);
    if (verifyModule(**Parsed, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("input LLVM module verification failed: ") +
                           Stream.str());
    }
  }
  return std::move(*Parsed);
}

bool hasOcmlF32Abi(const Function &FunctionValue) {
  FunctionType *Type = FunctionValue.getFunctionType();
  return Type->getReturnType()->isFloatTy() && !Type->isVarArg() &&
         Type->getNumParams() == 1 && Type->getParamType(0)->isFloatTy() &&
         FunctionValue.getCallingConv() == CallingConv::C;
}

Error validateOcmlImportAbi(const Module &Compiler, StringRef Name) {
  const auto *FunctionValue =
      dyn_cast_or_null<Function>(Compiler.getNamedValue(Name));
  if (!FunctionValue || !FunctionValue->isDeclaration() ||
      !hasOcmlF32Abi(*FunctionValue))
    return pipelineError(Twine("compiler module has the wrong OCML ABI for ") +
                         Name);
  return Error::success();
}

Error validateOcmlRootAbi(const Module &Provider, StringRef Name) {
  const auto *FunctionValue =
      dyn_cast_or_null<Function>(Provider.getNamedValue(Name));
  if (!FunctionValue || FunctionValue->isDeclaration())
    return pipelineError(Twine("gfx942 OCML provider does not define ") + Name);
  if (!hasOcmlF32Abi(*FunctionValue))
    return pipelineError(Twine("gfx942 OCML provider has the wrong ABI for ") +
                         Name);
  return Error::success();
}

Error reduceBuiltinProviderClosure(Module &Linked,
                                   const Request &RequestValue) {
  std::set<std::string> Preserved(RequestValue.ExpectedDefinedSymbols.begin(),
                                  RequestValue.ExpectedDefinedSymbols.end());
  for (const std::string &Import : RequestValue.ImportSymbols) {
    if (!isSupportedGfx942OcmlImport(Import))
      continue;
    auto *FunctionValue =
        dyn_cast_or_null<Function>(Linked.getNamedValue(Import));
    if (!FunctionValue || FunctionValue->isDeclaration())
      return pipelineError(Twine("gfx942 OCML import remained unresolved: ") +
                           Import);
    FunctionValue->setVisibility(GlobalValue::DefaultVisibility);
  }

  internalizeModule(Linked, [&Preserved](const GlobalValue &Value) {
    return Value.hasName() && Preserved.contains(Value.getName().str());
  });
  ModuleAnalysisManager Analyses;
  GlobalDCEPass().run(Linked, Analyses);
  return Error::success();
}

Expected<std::unique_ptr<Module>> linkBitcode(const Request &RequestValue,
                                              ArrayRef<Input> BuiltinProviders,
                                              LLVMContext &Context,
                                              const TargetMachine &Machine) {
  std::unique_ptr<Module> Linked;
  for (size_t I = 0; I < RequestValue.Inputs.size(); ++I) {
    const Input &InputValue = RequestValue.Inputs[I];
    if (InputValue.Kind != InputKind::LlvmBitcode &&
        InputValue.Kind != InputKind::LlvmTextIr)
      continue;
    std::string InputName = (Twine("<bitcode-input-") + Twine(I) + ">").str();
    auto Parsed =
        parseModuleInput(InputValue, InputName, RequestValue, Context, Machine);
    if (!Parsed)
      return Parsed.takeError();
    if (!Linked)
      Linked = std::move(*Parsed);
    else if (Linker::linkModules(*Linked, std::move(*Parsed)))
      return pipelineError("LLVM module linking failed");
  }

  if (!BuiltinProviders.empty() && !Linked)
    return pipelineError(
        "measured device library has no compiler LLVM module to resolve");
  if (!BuiltinProviders.empty())
    for (const std::string &Import : RequestValue.ImportSymbols)
      if (isSupportedGfx942OcmlImport(Import))
        if (Error E = validateOcmlImportAbi(*Linked, Import))
          return E;

  for (size_t I = 0; I < BuiltinProviders.size(); ++I) {
    const Input &InputValue = BuiltinProviders[I];
    std::string InputName =
        (Twine("<measured-gfx942-device-library-") + Twine(I) + ">").str();
    auto Parsed =
        parseModuleInput(InputValue, InputName, RequestValue, Context, Machine);
    if (!Parsed)
      return Parsed.takeError();
    for (const std::string &Import : RequestValue.ImportSymbols)
      if (isSupportedGfx942OcmlImport(Import))
        if (const GlobalValue *Value = (*Parsed)->getNamedValue(Import);
            Value && !Value->isDeclaration())
          if (Error E = validateOcmlRootAbi(**Parsed, Import))
            return E;
    if (!Linked)
      return pipelineError(
          "measured device library has no compiler module to resolve");
    if (Linker::linkModules(*Linked, std::move(*Parsed),
                            Linker::Flags::LinkOnlyNeeded))
      return pipelineError("gfx942 device-library linking failed");
  }
  if (!BuiltinProviders.empty())
    if (Error E = reduceBuiltinProviderClosure(*Linked, RequestValue))
      return E;
  if (Linked) {
    BoundedRawStream Stream(MaxDiagnosticBytes);
    if (verifyModule(*Linked, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("linked LLVM module verification failed: ") +
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

  // The compiler descriptor commits COV5/6 kernels to the complete 256-byte
  // implicit block. Optimization may infer amdgpu-no-implicitarg-ptr when a
  // kernel does not read that block; canonicalize the physical ABI after the
  // inference passes so metadata, native descriptors, and host admission stay
  // on the same contract.
  if (RequestValue.CodeObjectVersion >= 5) {
    for (Function &FunctionValue : ModuleValue) {
      if (FunctionValue.getCallingConv() != CallingConv::AMDGPU_KERNEL)
        continue;
      FunctionValue.removeFnAttr("amdgpu-no-implicitarg-ptr");
      FunctionValue.addFnAttr("amdgpu-implicitarg-num-bytes", "256");
    }
  }

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

struct SymbolContract {
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::set<std::string> RequiredImports;
};

std::string diagnosticAtom(StringRef Value) {
  static constexpr char Hex[] = "0123456789ABCDEF";
  std::string Result;
  for (unsigned char Byte : Value) {
    if (llvm::isAlnum(Byte) || Byte == '_' || Byte == '-' || Byte == '.' ||
        Byte == '$') {
      Result.push_back(static_cast<char>(Byte));
    } else {
      Result.push_back('%');
      Result.push_back(Hex[Byte >> 4]);
      Result.push_back(Hex[Byte & 0xf]);
    }
  }
  return Result;
}

std::string diagnosticList(const std::set<std::string> &Values) {
  std::string Result = "[";
  for (const std::string &Value : Values) {
    if (Result.size() != 1)
      Result.push_back(',');
    Result.append(diagnosticAtom(Value));
  }
  Result.push_back(']');
  return Result;
}

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
    if ((Flags & SymbolRef::SF_FormatSpecific) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (NameOrError->empty())
      continue;
    std::string Name = NameOrError->str();
    if ((Flags & SymbolRef::SF_Undefined) != 0) {
      Result.RequiredImports.insert(std::move(Name));
    } else if ((Flags & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) != 0) {
      Result.Definitions.insert(Name);
      if ((Flags & SymbolRef::SF_Hidden) == 0)
        Result.PublicDefinitions.insert(std::move(Name));
    }
  }
  return Result;
}

SymbolContract inspectModuleSymbols(const Module &ModuleValue) {
  SymbolContract Result;
  for (const GlobalValue &Value : ModuleValue.global_values()) {
    if (!Value.hasName() || Value.hasLocalLinkage() ||
        Value.hasAppendingLinkage())
      continue;

    if (Value.isDeclarationForLinker()) {
      const auto *FunctionValue = dyn_cast<Function>(&Value);
      if (!Value.use_empty() &&
          (!FunctionValue || !FunctionValue->isIntrinsic()))
        Result.RequiredImports.insert(Value.getName().str());
      continue;
    }

    std::string Name = Value.getName().str();
    Result.Definitions.insert(Name);
    if (Value.getVisibility() == GlobalValue::DefaultVisibility)
      Result.PublicDefinitions.insert(std::move(Name));
  }
  return Result;
}

Expected<SymbolContract> inspectModuleSymbols(const Input &InputValue,
                                              StringRef InputName) {
  StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                  InputValue.Bytes.size());
  LLVMContext Context;
  Expected<std::unique_ptr<Module>> Parsed =
      [&]() -> Expected<std::unique_ptr<Module>> {
    if (InputValue.Kind == InputKind::LlvmBitcode)
      return parseBitcodeFile(MemoryBufferRef(Bytes, InputName), Context);
    SMDiagnostic Diagnostic;
    auto TextBuffer = MemoryBuffer::getMemBufferCopy(Bytes, InputName);
    auto TextModule =
        parseAssembly(TextBuffer->getMemBufferRef(), Diagnostic, Context);
    if (!TextModule) {
      std::string Message;
      raw_string_ostream Stream(Message);
      Diagnostic.print("fe2o3-llvm-link-worker", Stream, false, false);
      Stream.flush();
      return pipelineError(Message);
    }
    return TextModule;
  }();
  if (!Parsed)
    return pipelineError(Twine(InputName) + ": " +
                         errorToDiagnostic(Parsed.takeError()));

  return inspectModuleSymbols(**Parsed);
}

Error requireExactCompilerImports(const Request &RequestValue,
                                  const SymbolContract &CompilerModule,
                                  StringRef Source) {
  std::set<std::string> Declared(RequestValue.ImportSymbols.begin(),
                                 RequestValue.ImportSymbols.end());
  std::set<std::string> Omitted;
  std::set<std::string> Extra;
  for (const std::string &Name : CompilerModule.RequiredImports)
    if (!Declared.contains(Name))
      Omitted.insert(Name);
  for (const std::string &Name : Declared)
    if (!CompilerModule.RequiredImports.contains(Name))
      Extra.insert(Name);
  if (!Omitted.empty() || !Extra.empty())
    return pipelineError(
        Twine("compiler-module") +
        (Source.empty() ? "" : (Twine(" ") + Source).str()) +
        " import manifest mismatch: omitted=" + diagnosticList(Omitted) +
        " extra=" + diagnosticList(Extra));
  return Error::success();
}

Expected<SymbolContract>
inspectNormalizedCompilerModule(const Request &RequestValue,
                                TargetMachine &Machine) {
  LLVMContext Context;
  auto Parsed =
      parseModuleInput(RequestValue.CompilerModule, "<compiler-module>",
                       RequestValue, Context, Machine);
  if (!Parsed)
    return Parsed.takeError();

  BoundedRawStream Verification(MaxDiagnosticBytes);
  if (verifyModule(**Parsed, &Verification)) {
    Verification.flush();
    return pipelineError(Twine("compiler-module verification failed: ") +
                         Verification.str());
  }

  SymbolContract IrSymbols = inspectModuleSymbols(**Parsed);
  auto ObjectBytes = emitObject(**Parsed, Machine);
  if (!ObjectBytes)
    return ObjectBytes.takeError();
  auto ObjectSymbols = inspectRelocatable(*ObjectBytes);
  if (!ObjectSymbols)
    return ObjectSymbols.takeError();
  SymbolContract EmittedSymbols{std::move(ObjectSymbols->Definitions),
                                std::move(ObjectSymbols->PublicDefinitions),
                                std::move(ObjectSymbols->RequiredImports)};

  if (Error E = requireExactCompilerImports(RequestValue, IrSymbols, ""))
    return E;
  if (Error E =
          requireExactCompilerImports(RequestValue, EmittedSymbols, "object"))
    return E;
  if (IrSymbols.RequiredImports != EmittedSymbols.RequiredImports)
    return pipelineError(
        "compiler-module IR and object import sets do not match");
  return IrSymbols;
}

Expected<SymbolContract> inspectInputSymbols(const Input &InputValue,
                                             StringRef InputName) {
  if (InputValue.Kind == InputKind::LlvmBitcode ||
      InputValue.Kind == InputKind::LlvmTextIr)
    return inspectModuleSymbols(InputValue, InputName);
  auto Elf = inspectRelocatable(InputValue.Bytes);
  if (!Elf)
    return Elf.takeError();
  return SymbolContract{std::move(Elf->Definitions),
                        std::move(Elf->PublicDefinitions),
                        std::move(Elf->RequiredImports)};
}

Error validateV2SymbolRoles(const Request &RequestValue,
                            const SymbolContract &Module,
                            ArrayRef<Input> BuiltinProviders) {
  std::vector<SymbolContract> Providers;
  Providers.reserve(RequestValue.ExternalProviders.size() +
                    BuiltinProviders.size());
  for (size_t I = 0; I < RequestValue.ExternalProviders.size(); ++I) {
    std::string Name = (Twine("<external-provider-") + Twine(I) + ">").str();
    auto Provider =
        inspectInputSymbols(RequestValue.ExternalProviders[I], Name);
    if (!Provider)
      return Provider.takeError();
    Providers.push_back(std::move(*Provider));
  }
  for (size_t I = 0; I < BuiltinProviders.size(); ++I) {
    std::string Name =
        (Twine("<measured-gfx942-device-library-") + Twine(I) + ">").str();
    auto Provider = inspectInputSymbols(BuiltinProviders[I], Name);
    if (!Provider)
      return Provider.takeError();
    Providers.push_back(std::move(*Provider));
  }

  std::map<std::string, size_t> DefinitionCounts;
  for (const std::string &Name : Module.Definitions)
    ++DefinitionCounts[Name];
  for (const SymbolContract &Provider : Providers)
    for (const std::string &Name : Provider.Definitions)
      ++DefinitionCounts[Name];
  for (const auto &[Name, Count] : DefinitionCounts)
    if (Count > 1)
      return pipelineError(Twine("duplicate definition: ") + Name);

  for (const std::string &Name : RequestValue.ExportSymbols)
    if (!Module.PublicDefinitions.contains(Name))
      return pipelineError(Twine("compiler-module export is not defined by "
                                 "the compiler module: ") +
                           Name);

  for (const std::string &Name : RequestValue.ImportSymbols) {
    if (Module.Definitions.contains(Name))
      return pipelineError(Twine("compiler-module import is defined by the "
                                 "compiler module: ") +
                           Name);
    if (!Module.RequiredImports.contains(Name))
      return pipelineError(Twine("compiler-module import is not unresolved "
                                 "by the compiler module: ") +
                           Name);
    bool ResolvedByProvider =
        llvm::any_of(Providers, [&Name](const SymbolContract &Provider) {
          return Provider.Definitions.contains(Name);
        });
    if (!ResolvedByProvider)
      return pipelineError(Twine("compiler-module import has no external "
                                 "provider: ") +
                           Name);
  }
  return Error::success();
}

struct KernelLaunchContract {
  std::string Name;
  std::string Symbol;
  uint64_t KernargSegmentSize;
  uint64_t GroupSegmentFixedSize;
  uint64_t PrivateSegmentFixedSize;
  uint64_t KernargSegmentAlign;
  uint64_t WavefrontSize;
  uint64_t MaxFlatWorkgroupSize;
  std::optional<std::array<uint64_t, 3>> RequiredWorkgroupSize;
};

struct MetadataContract {
  bool Present = false;
  std::optional<std::string> Target;
  std::vector<KernelLaunchContract> Kernels;
};

Error postLinkError(StringRef Check, StringRef Reason) {
  return pipelineError(Twine("post_link.check=") + Check +
                       " status=failed reason=" + diagnosticAtom(Reason));
}

std::string hexadecimal(uint32_t Value) {
  std::ostringstream Stream;
  Stream << "0x" << std::hex << std::nouppercase << Value;
  return Stream.str();
}

Expected<msgpack::DocNode *> requiredMetadataField(msgpack::MapDocNode &Map,
                                                   StringRef Name) {
  auto Field = Map.find(Name);
  if (Field == Map.end())
    return pipelineError(Twine("AMDGPU metadata is missing ") + Name);
  return &Field->second;
}

Expected<StringRef> metadataString(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredMetadataField(Map, Name);
  if (!Field)
    return Field.takeError();
  if (!(**Field).isString())
    return pipelineError(Twine("AMDGPU metadata field ") + Name +
                         " is not a string");
  return (**Field).getString();
}

Expected<uint64_t> metadataUnsigned(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredMetadataField(Map, Name);
  if (!Field)
    return Field.takeError();
  if ((**Field).getKind() == msgpack::Type::UInt)
    return (**Field).getUInt();
  if ((**Field).getKind() == msgpack::Type::Int && (**Field).getInt() >= 0)
    return static_cast<uint64_t>((**Field).getInt());
  return pipelineError(Twine("AMDGPU metadata field ") + Name +
                       " is not a nonnegative integer");
}

Expected<std::optional<std::array<uint64_t, 3>>>
metadataWorkgroupSize(msgpack::MapDocNode &Map) {
  auto Field = Map.find(".reqd_workgroup_size");
  if (Field == Map.end())
    return std::optional<std::array<uint64_t, 3>>{};
  if (!Field->second.isArray() || Field->second.getArray().size() != 3)
    return pipelineError(
        "AMDGPU metadata .reqd_workgroup_size is not a three-element array");
  std::array<uint64_t, 3> Result{};
  size_t I = 0;
  for (msgpack::DocNode &Node : Field->second.getArray()) {
    if (Node.getKind() == msgpack::Type::UInt)
      Result[I] = Node.getUInt();
    else if (Node.getKind() == msgpack::Type::Int && Node.getInt() >= 0)
      Result[I] = static_cast<uint64_t>(Node.getInt());
    else
      return pipelineError("AMDGPU metadata workgroup dimension is not a "
                           "nonnegative integer");
    if (Result[I] == 0)
      return pipelineError("AMDGPU metadata workgroup dimension is zero");
    ++I;
  }
  return std::optional<std::array<uint64_t, 3>>(Result);
}

Expected<MetadataContract>
inspectMetadata(const ELFObjectFile<ELF64LE> &ObjectValue) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();

  std::vector<StringRef> MetadataBlobs;
  for (const ELF64LE::Shdr &Section : *Sections) {
    if (Section.sh_type != ELF::SHT_NOTE)
      continue;
    Error NoteError = Error::success();
    for (const ELF64LE::Note Note : File.notes(Section, NoteError)) {
      if (Note.getName() != "AMDGPU" ||
          Note.getType() != ELF::NT_AMDGPU_METADATA)
        continue;
      if (Section.sh_addralign != 4)
        return pipelineError(
            "AMDGPU metadata note has a noncanonical alignment");
      MetadataBlobs.push_back(Note.getDescAsStringRef(Section.sh_addralign));
    }
    if (NoteError)
      return NoteError;
  }

  MetadataContract Result;
  if (MetadataBlobs.empty())
    return Result;
  Result.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  for (StringRef MetadataBlob : MetadataBlobs) {
    if (MetadataBlob.empty())
      return pipelineError("linked output has an empty AMDGPU metadata note");
    msgpack::Document Document;
    if (!Document.readFromBlob(MetadataBlob, false))
      return pipelineError("linked output has malformed AMDGPU metadata");
    AMDGPU::HSAMD::V3::MetadataVerifier Verifier(true);
    if (!Verifier.verify(Document.getRoot()))
      return pipelineError("linked output has invalid AMDGPU metadata schema");

    auto &Root = Document.getRoot().getMap();
    auto Target = metadataString(Root, "amdhsa.target");
    if (!Target)
      return Target.takeError();
    if (Result.Target && *Result.Target != *Target)
      return pipelineError(
          "linked output has conflicting AMDGPU metadata targets");
    Result.Target = Target->str();
    auto KernelsField = requiredMetadataField(Root, "amdhsa.kernels");
    if (!KernelsField)
      return KernelsField.takeError();
    if (!(**KernelsField).isArray())
      return pipelineError("AMDGPU metadata kernels field is not an array");

    for (msgpack::DocNode &KernelNode : (**KernelsField).getArray()) {
      if (!KernelNode.isMap())
        return pipelineError("AMDGPU metadata kernel is not a map");
      auto &Kernel = KernelNode.getMap();
      auto Name = metadataString(Kernel, ".name");
      if (!Name)
        return Name.takeError();
      auto Symbol = metadataString(Kernel, ".symbol");
      if (!Symbol)
        return Symbol.takeError();
      auto KernargSize = metadataUnsigned(Kernel, ".kernarg_segment_size");
      if (!KernargSize)
        return KernargSize.takeError();
      auto GroupSize = metadataUnsigned(Kernel, ".group_segment_fixed_size");
      if (!GroupSize)
        return GroupSize.takeError();
      auto PrivateSize =
          metadataUnsigned(Kernel, ".private_segment_fixed_size");
      if (!PrivateSize)
        return PrivateSize.takeError();
      auto KernargAlign = metadataUnsigned(Kernel, ".kernarg_segment_align");
      if (!KernargAlign)
        return KernargAlign.takeError();
      auto Wavefront = metadataUnsigned(Kernel, ".wavefront_size");
      if (!Wavefront)
        return Wavefront.takeError();
      auto MaxWorkgroup = metadataUnsigned(Kernel, ".max_flat_workgroup_size");
      if (!MaxWorkgroup)
        return MaxWorkgroup.takeError();
      auto RequiredWorkgroup = metadataWorkgroupSize(Kernel);
      if (!RequiredWorkgroup)
        return RequiredWorkgroup.takeError();

      if (!Names.insert(Name->str()).second)
        return pipelineError("AMDGPU metadata repeats a kernel name");
      if (!Symbols.insert(Symbol->str()).second)
        return pipelineError("AMDGPU metadata repeats a kernel symbol");
      if (*Symbol != (Twine(*Name) + ".kd").str())
        return pipelineError(
            "AMDGPU metadata kernel descriptor does not match its entry name");
      if (*KernargAlign == 0 || !isPowerOf2_64(*KernargAlign))
        return pipelineError("AMDGPU metadata kernarg alignment is invalid");
      if (*Wavefront != 32 && *Wavefront != 64)
        return pipelineError("AMDGPU metadata wavefront size is invalid");
      if (*MaxWorkgroup == 0)
        return pipelineError("AMDGPU metadata maximum workgroup size is zero");
      if (*RequiredWorkgroup) {
        uint64_t Product = 1;
        for (uint64_t Dimension : **RequiredWorkgroup) {
          if (Product > std::numeric_limits<uint64_t>::max() / Dimension)
            return pipelineError("AMDGPU metadata workgroup size overflows");
          Product *= Dimension;
        }
        if (Product > *MaxWorkgroup)
          return pipelineError(
              "AMDGPU metadata required workgroup size exceeds its maximum");
      }
      Result.Kernels.push_back({Name->str(), Symbol->str(), *KernargSize,
                                *GroupSize, *PrivateSize, *KernargAlign,
                                *Wavefront, *MaxWorkgroup, *RequiredWorkgroup});
    }
  }
  llvm::sort(Result.Kernels, [](const KernelLaunchContract &Left,
                                const KernelLaunchContract &Right) {
    return std::tie(Left.Name, Left.Symbol) <
           std::tie(Right.Name, Right.Symbol);
  });
  return Result;
}

Expected<std::vector<std::string>> inspectOutput(ArrayRef<uint8_t> Bytes,
                                                 const ElfContract &Expected,
                                                 const Request &RequestValue) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<output>"));
  if (!ObjectOrError)
    return postLinkError("elf", errorToDiagnostic(ObjectOrError.takeError()));
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  if (!Elf || Elf->getEMachine() != ELF::EM_AMDGPU ||
      Elf->getEType() != ELF::ET_DYN)
    return postLinkError("elf", "not_amdgpu_shared");
  if (Bytes.size() < ELF::EI_NIDENT ||
      Bytes[ELF::EI_CLASS] != ELF::ELFCLASS64 ||
      Bytes[ELF::EI_DATA] != ELF::ELFDATA2LSB ||
      Bytes[ELF::EI_VERSION] != ELF::EV_CURRENT ||
      Elf->getBytesInAddress() != 8 || !Elf->isLittleEndian())
    return postLinkError("elf", "invalid_envelope");

  TargetParts RequestedTarget = parseTarget(RequestValue.Target);
  uint32_t PlatformFlags = Elf->getPlatformFlags();
  if (RequestedTarget.Cpu == "gfx942") {
    uint32_t ExpectedFlags = expectedGfx942Flags(RequestedTarget);
    if (PlatformFlags != ExpectedFlags)
      return postLinkError("target", (Twine("e_flags expected=") +
                                      hexadecimal(ExpectedFlags) +
                                      " actual=" + hexadecimal(PlatformFlags))
                                         .str());
  }
  if (Elf->getPlatformFlags() != Expected.Flags ||
      Bytes[ELF::EI_OSABI] != Expected.OsAbi ||
      Elf->getEIdentABIVersion() != Expected.AbiVersion ||
      Elf->getBytesInAddress() != Expected.AddressBytes ||
      Elf->isLittleEndian() != Expected.LittleEndian)
    return postLinkError("target", "target_or_code_object_mismatch");

  std::set<std::string> Defined;
  std::set<std::string> UndefinedSymbols;
  auto InspectSymbol = [&](SymbolRef Symbol, bool Dynamic) -> Error {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    uint32_t Flags = *FlagsOrError;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (NameOrError->empty())
      return Error::success();
    std::string Name = NameOrError->str();
    if ((Flags & SymbolRef::SF_Undefined) != 0) {
      UndefinedSymbols.insert(std::move(Name));
      return Error::success();
    }
    if ((Flags & SymbolRef::SF_FormatSpecific) != 0)
      return Error::success();
    if ((Flags & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0)
      return Error::success();
    if (Dynamic && (Flags & SymbolRef::SF_Hidden) == 0)
      Defined.insert(std::move(Name));
    return Error::success();
  };
  for (SymbolRef Symbol : Elf->symbols())
    if (Error E = InspectSymbol(Symbol, false))
      return postLinkError("symbols", errorToDiagnostic(std::move(E)));
  for (SymbolRef Symbol : Elf->getDynamicSymbolIterators()) {
    if (Error E = InspectSymbol(Symbol, true))
      return postLinkError("symbols", errorToDiagnostic(std::move(E)));
  }
  if (!UndefinedSymbols.empty())
    return pipelineError(
        Twine("post_link.check=unresolved status=failed symbols=") +
        diagnosticList(UndefinedSymbols));

  std::set<std::string> ExpectedSymbols(
      RequestValue.ExpectedDefinedSymbols.begin(),
      RequestValue.ExpectedDefinedSymbols.end());
  if (Defined != ExpectedSymbols)
    return pipelineError(
        Twine("post_link.check=exports status=failed expected=") +
        diagnosticList(ExpectedSymbols) + " actual=" + diagnosticList(Defined));

  auto *ConcreteElf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!ConcreteElf)
    return postLinkError("elf", "not_elf64le");
  auto Metadata = inspectMetadata(*ConcreteElf);
  if (!Metadata)
    return postLinkError("metadata", errorToDiagnostic(Metadata.takeError()));
  if (Metadata->Present) {
    std::string ExpectedMetadataTarget =
        (Twine(AmdGpuTriple) + "--" + RequestValue.Target).str();
    if (!Metadata->Target || *Metadata->Target != ExpectedMetadataTarget)
      return postLinkError(
          "metadata_target",
          (Twine("expected=") + ExpectedMetadataTarget +
           " actual=" + (Metadata->Target ? *Metadata->Target : "absent"))
              .str());
  }

  std::set<std::string> ExpectedDescriptors;
  for (const std::string &Name : ExpectedSymbols)
    if (StringRef(Name).ends_with(".kd"))
      ExpectedDescriptors.insert(Name);
  std::set<std::string> MetadataDescriptors;
  for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
    if (!Defined.contains(Kernel.Name) || !Defined.contains(Kernel.Symbol))
      return postLinkError("metadata_bindings",
                           "kernel_or_descriptor_not_exported");
    MetadataDescriptors.insert(Kernel.Symbol);
  }
  if (!ExpectedDescriptors.empty() && !Metadata->Present)
    return postLinkError("metadata", "missing_for_descriptors");
  if (MetadataDescriptors != ExpectedDescriptors)
    return pipelineError(
        Twine("post_link.check=descriptors status=failed expected=") +
        diagnosticList(ExpectedDescriptors) +
        " actual=" + diagnosticList(MetadataDescriptors));

  if (RequestedTarget.Cpu == "gfx942" && !ExpectedDescriptors.empty()) {
    static constexpr std::array<uint64_t, 3> G1Workgroup = {256, 1, 1};
    for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
      if (!Kernel.RequiredWorkgroupSize ||
          *Kernel.RequiredWorkgroupSize != G1Workgroup)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=reqd_workgroup_size expected=[256,1,1]");
      if (Kernel.MaxFlatWorkgroupSize != 256)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=max_flat_workgroup_size expected=256 actual=" +
            Twine(Kernel.MaxFlatWorkgroupSize));
      if (Kernel.WavefrontSize != 64)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=wavefront_size expected=64 actual=" +
            Twine(Kernel.WavefrontSize));
    }
  }

  std::vector<std::string> Diagnostics;
  Diagnostics.push_back(
      (Twine("post_link.check=target status=ok arch=") +
       diagnosticAtom(RequestedTarget.Cpu) + " code_object_version=" +
       Twine(static_cast<unsigned>(RequestValue.CodeObjectVersion)) +
       " e_flags=" + hexadecimal(Elf->getPlatformFlags()))
          .str());
  Diagnostics.push_back((Twine("post_link.check=exports status=ok symbols=") +
                         diagnosticList(Defined))
                            .str());
  Diagnostics.push_back("post_link.check=unresolved status=ok symbols=[]");
  Diagnostics.push_back(
      (Twine("post_link.check=metadata status=") +
       (Metadata->Present ? "ok" : "absent") +
       " kernels=" + Twine(Metadata->Kernels.size()) + " target=" +
       diagnosticAtom(Metadata->Target ? StringRef(*Metadata->Target)
                                       : StringRef("absent")))
          .str());
  for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
    std::string Required = "absent";
    if (Kernel.RequiredWorkgroupSize)
      Required = (Twine("[") + Twine((*Kernel.RequiredWorkgroupSize)[0]) + "," +
                  Twine((*Kernel.RequiredWorkgroupSize)[1]) + "," +
                  Twine((*Kernel.RequiredWorkgroupSize)[2]) + "]")
                     .str();
    Diagnostics.push_back(
        (Twine("post_link.kernel name=") + diagnosticAtom(Kernel.Name) +
         " symbol=" + diagnosticAtom(Kernel.Symbol) +
         " kernarg_size=" + Twine(Kernel.KernargSegmentSize) +
         " group_size=" + Twine(Kernel.GroupSegmentFixedSize) +
         " private_size=" + Twine(Kernel.PrivateSegmentFixedSize) +
         " kernarg_align=" + Twine(Kernel.KernargSegmentAlign) +
         " wavefront_size=" + Twine(Kernel.WavefrontSize) +
         " max_workgroup_size=" + Twine(Kernel.MaxFlatWorkgroupSize) +
         " reqd_workgroup_size=" + Required)
            .str());
  }
  return Diagnostics;
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

Expected<std::vector<std::string>>
inspectLinkedOutputForPublication(ArrayRef<uint8_t> Bytes,
                                  const Request &RequestValue) {
  if (Error E = validateRequest(RequestValue))
    return E;
  auto MachineOrError = createMachine(RequestValue);
  if (!MachineOrError)
    return MachineOrError.takeError();
  std::unique_ptr<TargetMachine> Machine = std::move(*MachineOrError);
  LLVMContext Context;
  Module Reference("fe2o3-publication-target-reference", Context);
  if (Error E = setAndCheckModuleContract(Reference, RequestValue, *Machine))
    return E;
  auto ReferenceObject = emitObject(Reference, *Machine);
  if (!ReferenceObject)
    return ReferenceObject.takeError();
  auto ExpectedElf = inspectRelocatable(*ReferenceObject);
  if (!ExpectedElf)
    return ExpectedElf.takeError();
  if (ExpectedElf->AbiVersion !=
      expectedAbiVersion(RequestValue.CodeObjectVersion))
    return pipelineError(
        "target machine emitted the wrong code-object version");
  return inspectOutput(Bytes, *ExpectedElf, RequestValue);
}

Response executeImpl(const Request &RequestValue,
                     const Gfx942DeviceLibraryPolicy *TestPolicy) {
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

  std::optional<SymbolContract> CompilerModule;
  if (RequestValue.Protocol == ProtocolVersion::V2) {
    auto Inspected = inspectNormalizedCompilerModule(RequestValue, *Machine);
    if (!Inspected)
      return failure(RequestValue, Stage::InputValidation,
                     Inspected.takeError());
    CompilerModule = std::move(*Inspected);
  }

  std::set<std::string> BuiltinOcmlImports;
  const std::set<std::string> NoImports;
  const std::set<std::string> &MeasuredImports =
      CompilerModule ? CompilerModule->RequiredImports : NoImports;
  for (const std::string &Import : MeasuredImports) {
    if (isSupportedGfx942OcmlImport(Import)) {
      BuiltinOcmlImports.insert(Import);
      continue;
    }
    if (isOcmlImportNamespace(Import))
      return failure(RequestValue, Stage::InputValidation,
                     {"unsupported gfx942 OCML import: " + Import});
  }

  std::vector<Input> BuiltinProviders;
  std::optional<DeviceLibraryProviderEvidence> ProviderEvidence;
  if (!BuiltinOcmlImports.empty()) {
    TargetParts Parts = parseTarget(RequestValue.Target);
    if (RequestValue.Protocol != ProtocolVersion::V2 || Parts.Cpu != "gfx942" ||
        !isSupportedGfx942OcmlCodeObjectVersion(RequestValue.CodeObjectVersion))
      return failure(RequestValue, Stage::InputValidation,
                     {"measured OCML providers require Worker V2, gfx942, and "
                      "code-object V5 or V6"});
    Expected<Gfx942DeviceLibraryPolicy> Measured =
        TestPolicy ? Expected<Gfx942DeviceLibraryPolicy>(*TestPolicy)
                   : measuredGfx942DeviceLibraryPolicy();
    if (!Measured)
      return failure(RequestValue, Stage::Toolchain, Measured.takeError());
    std::vector<std::string> MeasuredImportList(MeasuredImports.begin(),
                                                MeasuredImports.end());
    auto Loaded = loadGfx942DeviceLibraries(MeasuredImportList, *Measured);
    if (!Loaded)
      return failure(RequestValue, Stage::Toolchain, Loaded.takeError());
    BuiltinProviders = std::move(*Loaded);

    DeviceLibraryProviderEvidence Evidence;
    Evidence.ProviderIdentity = "gfx942-ocml-v1";
    Evidence.Target = RequestValue.Target;
    Evidence.CodeObjectVersion = RequestValue.CodeObjectVersion;
    Evidence.ImportSymbols.assign(BuiltinOcmlImports.begin(),
                                  BuiltinOcmlImports.end());
    for (const PinnedDeviceLibraryFile &File : Measured->Files)
      Evidence.Files.push_back({File.Basename, File.Digest});
    auto ManifestIdentity = calculateProviderManifestIdentity(Evidence);
    if (!ManifestIdentity)
      return failure(RequestValue, Stage::Toolchain,
                     ManifestIdentity.takeError());
    Evidence.ManifestIdentity = *ManifestIdentity;
    ProviderEvidence = std::move(Evidence);
  }

  if (RequestValue.Protocol == ProtocolVersion::V2)
    if (Error E = validateV2SymbolRoles(RequestValue, *CompilerModule,
                                        BuiltinProviders))
      return failure(RequestValue, Stage::InputValidation, std::move(E));

  LLVMContext Context;
  auto LinkedModule =
      linkBitcode(RequestValue, BuiltinProviders, Context, *Machine);
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
  if (!BuiltinProviders.empty())
    LinkDiagnostics.push_back(
        (Twine("device_library.check=identity status=ok ") +
         "provider=gfx942-ocml-v1 roots=" + diagnosticList(BuiltinOcmlImports) +
         " files=" + Twine(BuiltinProviders.size()))
            .str());
  auto LinkedBytes =
      nativeLink(Objects, RequestValue, Temporary.Path, LinkDiagnostics);
  if (!LinkedBytes) {
    LinkDiagnostics.push_back(errorToDiagnostic(LinkedBytes.takeError()));
    return failure(RequestValue, Stage::NativeLink,
                   canonicalDiagnostics(LinkDiagnostics, Temporary.Path));
  }

  auto PublicationDiagnostics =
      inspectOutput(*LinkedBytes, *ExpectedElf, RequestValue);
  if (!PublicationDiagnostics)
    return failure(RequestValue, Stage::OutputInspection,
                   PublicationDiagnostics.takeError());
  LinkDiagnostics.insert(LinkDiagnostics.end(), PublicationDiagnostics->begin(),
                         PublicationDiagnostics->end());

  Output ResultOutput{SHA256::hash(*LinkedBytes), std::move(*LinkedBytes)};
  Response Result{
      RequestValue.RequestId,
      RequestValue.Identity,
      (TestPolicy ? UnauthenticatedTestWorkerIdentity : WorkerBuildIdentity)
          .str(),
      Stage::Complete,
      canonicalDiagnostics(LinkDiagnostics, Temporary.Path),
      std::move(ResultOutput)};
  Result.Protocol = RequestValue.Protocol;
  Result.CompilerEnvelopeIdentity = RequestValue.CompilerEnvelopeIdentity;
  Result.DeviceLibraryProvider = std::move(ProviderEvidence);
  return Result;
}

Response execute(const Request &RequestValue) {
  return executeImpl(RequestValue, nullptr);
}

Response executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
    const Request &RequestValue, const Gfx942DeviceLibraryPolicy &Policy) {
  return executeImpl(RequestValue, &Policy);
}

} // namespace fe2o3::worker
