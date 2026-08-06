#include "WorkerPipeline.h"

#include "lld/Common/Driver.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/Bitcode/BitcodeReader.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DebugInfo.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Linker/Linker.h"
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

#include <algorithm>
#include <memory>
#include <optional>
#include <set>
#include <system_error>

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
  return {RequestValue.RequestId,
          RequestValue.Identity,
          WorkerBuildIdentity.str(),
          FailureStage,
          canonicalDiagnostics(Diagnostics),
          std::nullopt};
}

Response failure(const Request &RequestValue, Stage FailureStage,
                 Error ErrorValue) {
  return failure(RequestValue, FailureStage,
                 {errorToDiagnostic(std::move(ErrorValue))});
}

Error pipelineError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(), Message);
}

struct TargetParts {
  std::string Cpu;
};

TargetParts parseTarget(StringRef Target) {
  SmallVector<StringRef, 3> Components;
  Target.split(Components, ':', -1, false);
  TargetParts Result;
  Result.Cpu = Components.front().str();
  return Result;
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
  const Triple &ExistingTriple = ModuleValue.getTargetTriple();
  if (!ExistingTriple.getTriple().empty() &&
      Triple::normalize(ExistingTriple.getTriple()) != AmdGpuTriple)
    return pipelineError("bitcode target triple does not match AMDHSA");
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

  SmallVector<StringRef, 3> Components;
  StringRef(RequestValue.Target).split(Components, ':', -1, false);
  for (StringRef FlagName : {StringRef("sramecc"), StringRef("xnack")}) {
    std::optional<uint32_t> Requested;
    for (StringRef Component : drop_begin(Components))
      if (Component.drop_back() == FlagName)
        Requested = Component.back() == '+' ? 1 : 0;
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
  return Error::success();
}

Expected<std::unique_ptr<Module>> linkBitcode(const Request &RequestValue,
                                              LLVMContext &Context,
                                              const TargetMachine &Machine) {
  std::unique_ptr<Module> Linked;
  for (const Input &InputValue : RequestValue.Inputs) {
    if (InputValue.Kind != InputKind::LlvmBitcode)
      continue;
    StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                    InputValue.Bytes.size());
    auto Parsed = parseBitcodeFile(MemoryBufferRef(Bytes, "<input>"), Context);
    if (!Parsed)
      return Parsed.takeError();
    if (Error E = setAndCheckModuleContract(**Parsed, RequestValue, Machine))
      return E;
    if (RequestValue.LinkOptions.VerifyEach) {
      std::string Diagnostic;
      raw_string_ostream Stream(Diagnostic);
      if (verifyModule(**Parsed, &Stream)) {
        Stream.flush();
        return pipelineError(Twine("input bitcode verification failed: ") +
                             Diagnostic);
      }
    }
    if (!Linked) {
      Linked = std::move(*Parsed);
    } else if (Linker::linkModules(*Linked, std::move(*Parsed))) {
      return pipelineError("LLVM bitcode linking failed");
    }
  }
  if (Linked) {
    std::string Diagnostic;
    raw_string_ostream Stream(Diagnostic);
    if (verifyModule(*Linked, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("linked bitcode verification failed: ") +
                           Diagnostic);
    }
  }
  return Linked;
}

Error optimizeModule(Module &ModuleValue, const Request &RequestValue,
                     TargetMachine &Machine) {
  if (RequestValue.LinkOptions.StripDebug)
    StripDebugInfo(ModuleValue);
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
  std::string Diagnostic;
  raw_string_ostream Stream(Diagnostic);
  if (verifyModule(ModuleValue, &Stream)) {
    Stream.flush();
    return pipelineError(Twine("optimized bitcode verification failed: ") +
                         Diagnostic);
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
  uint8_t AbiVersion;
};

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
  return ElfContract{Elf->getPlatformFlags(), Elf->getEIdentABIVersion()};
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
  if (Elf->getPlatformFlags() != Expected.Flags ||
      Elf->getEIdentABIVersion() != Expected.AbiVersion)
    return pipelineError(
        "linked output target or code-object version mismatch");

  std::set<std::string> Defined;
  for (SymbolRef Symbol : Elf->symbols()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    if ((*FlagsOrError & SymbolRef::SF_Global) == 0 ||
        (*FlagsOrError & SymbolRef::SF_Undefined) != 0 ||
        (*FlagsOrError & SymbolRef::SF_FormatSpecific) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (!NameOrError->empty())
      Defined.insert(NameOrError->str());
  }
  return Defined;
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
  auto BufferOrError = MemoryBuffer::getFile(Path, false, false);
  if (!BufferOrError)
    return errorCodeToError(BufferOrError.getError());
  StringRef Bytes = (*BufferOrError)->getBuffer();
  if (Bytes.empty() || Bytes.size() > MaxBytes)
    return pipelineError("linked output violates byte bound");
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

  std::vector<std::string> OwnedArguments = {
      "ld.lld",           "--shared",   "--no-undefined",
      "--build-id=none",  "--nostdlib", "--no-dependent-libraries",
      "--fatal-warnings", "--threads=1"};
  if (RequestValue.LinkOptions.StripDebug)
    OwnedArguments.push_back("--strip-debug");
  OwnedArguments.insert(OwnedArguments.end(), Paths.begin(), Paths.end());
  OwnedArguments.push_back("-o");
  OwnedArguments.push_back(OutputPath.str().str());
  std::vector<const char *> Arguments;
  Arguments.reserve(OwnedArguments.size());
  for (const std::string &Argument : OwnedArguments)
    Arguments.push_back(Argument.c_str());

  std::string StandardOutput;
  std::string StandardError;
  raw_string_ostream OutputStream(StandardOutput);
  raw_string_ostream ErrorStream(StandardError);
  lld::Result LinkResult = lld::lldMain(Arguments, OutputStream, ErrorStream,
                                        {{lld::Gnu, &lld::elf::link}});
  OutputStream.flush();
  ErrorStream.flush();
  if (!StandardOutput.empty())
    Diagnostics.push_back(StandardOutput);
  if (!StandardError.empty())
    Diagnostics.push_back(StandardError);
  if (LinkResult.retCode != 0)
    return pipelineError("LLD ELF link failed");
  if (!LinkResult.canRunAgain)
    return pipelineError("LLD reported a non-reusable corrupted state");
  return readBytes(OutputPath, RequestValue.MaxOutputBytes);
}

} // namespace

Response execute(const Request &RequestValue) {
  if (RequestValue.LlvmBuildIdentity != LlvmBuildIdentity)
    return failure(RequestValue, Stage::Toolchain,
                   {"request LLVM identity does not match worker measurement"});

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

  std::vector<std::vector<uint8_t>> Objects;
  for (const Input &InputValue : RequestValue.Inputs) {
    if (InputValue.Kind != InputKind::AmdGpuRelocatable)
      continue;
    auto Contract = inspectRelocatable(InputValue.Bytes);
    if (!Contract)
      return failure(RequestValue, Stage::InputValidation,
                     Contract.takeError());
    if (Contract->Flags != ExpectedElf->Flags ||
        Contract->AbiVersion != ExpectedElf->AbiVersion)
      return failure(RequestValue, Stage::InputValidation,
                     {"native input target or code-object version mismatch"});
    Objects.push_back(InputValue.Bytes);
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
    if (Contract->Flags != ExpectedElf->Flags ||
        Contract->AbiVersion != ExpectedElf->AbiVersion)
      return failure(RequestValue, Stage::Codegen,
                     {"generated object target contract mismatch"});
    Objects.push_back(std::move(*GeneratedObject));
  }
  if (Objects.empty())
    return failure(RequestValue, Stage::InputValidation,
                   {"request produced no native link inputs"});

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
  return {RequestValue.RequestId,
          RequestValue.Identity,
          WorkerBuildIdentity.str(),
          Stage::Complete,
          canonicalDiagnostics(LinkDiagnostics, Temporary.Path),
          std::move(ResultOutput)};
}

} // namespace fe2o3::worker
