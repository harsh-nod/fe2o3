#include "WorkerLldPolicy.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/Bitcode/BitcodeWriter.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <cstdlib>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

#include <sys/wait.h>
#include <unistd.h>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;
using namespace llvm;
using namespace llvm::object;

namespace {

constexpr StringLiteral AmdGpuTriple = "amdgcn-amd-amdhsa";

enum class LayoutMode { Exact, Absent, Incompatible };

struct FixtureOptions {
  StringRef Cpu = "gfx942";
  uint8_t CodeObjectVersion = 5;
  LayoutMode Layout = LayoutMode::Exact;
  StringRef FunctionCpu;
  StringRef FunctionFeatures;
  bool WeakDefinition = false;
  bool WeakImport = false;
  uint32_t Addend = 1;
};

FixtureOptions withLayout(LayoutMode Layout) {
  FixtureOptions Result;
  Result.Layout = Layout;
  return Result;
}

FixtureOptions withCpu(StringRef Cpu) {
  FixtureOptions Result;
  Result.Cpu = Cpu;
  return Result;
}

FixtureOptions withCodeObjectVersion(uint8_t Version) {
  FixtureOptions Result;
  Result.CodeObjectVersion = Version;
  return Result;
}

FixtureOptions withFunctionContract(StringRef Cpu, StringRef Features) {
  FixtureOptions Result;
  Result.FunctionCpu = Cpu;
  Result.FunctionFeatures = Features;
  return Result;
}

FixtureOptions withFunctionFeatures(StringRef Features) {
  return withFunctionContract({}, Features);
}

FixtureOptions withFunctionCpu(StringRef Cpu) {
  return withFunctionContract(Cpu, {});
}

FixtureOptions withWeakImport() {
  FixtureOptions Result;
  Result.WeakImport = true;
  return Result;
}

FixtureOptions withAddend(uint32_t Addend) {
  FixtureOptions Result;
  Result.Addend = Addend;
  return Result;
}

FixtureOptions withWeakDefinition(uint32_t Addend) {
  FixtureOptions Result = withAddend(Addend);
  Result.WeakDefinition = true;
  return Result;
}

[[noreturn]] void fail(StringRef Message) {
  errs() << "pipeline test failed: " << Message << '\n';
  std::abort();
}

void require(bool Condition, StringRef Message) {
  if (!Condition)
    fail(Message);
}

std::unique_ptr<TargetMachine> createMachine(StringRef Cpu) {
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
  require(TargetValue != nullptr, LookupError);
  TargetOptions OptionsValue;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, Cpu, "", OptionsValue, Reloc::PIC_, CodeModel::Small,
      CodeGenOptLevel::None));
  require(Machine != nullptr, "could not create fixture target machine");
  return Machine;
}

std::unique_ptr<Module> makeModule(LLVMContext &Context, StringRef ModuleName,
                                   StringRef Definition,
                                   std::optional<StringRef> Callee,
                                   const FixtureOptions &Options) {
  auto Result = std::make_unique<Module>(ModuleName, Context);
  std::unique_ptr<TargetMachine> Machine = createMachine(Options.Cpu);
  Result->setTargetTriple(Triple(AmdGpuTriple));
  if (Options.Layout == LayoutMode::Exact)
    Result->setDataLayout(Machine->createDataLayout());
  else if (Options.Layout == LayoutMode::Incompatible)
    Result->setDataLayout("e-p:32:32");
  Result->addModuleFlag(Module::Error, "amdhsa_code_object_version",
                        Options.CodeObjectVersion * 100);

  Type *I32 = Type::getInt32Ty(Context);
  FunctionType *Signature = FunctionType::get(I32, {I32}, false);
  Function *Defined = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                       Definition, *Result);
  if (Options.WeakDefinition)
    Defined->setLinkage(GlobalValue::WeakAnyLinkage);
  if (!Options.FunctionCpu.empty())
    Defined->addFnAttr("target-cpu", Options.FunctionCpu);
  if (!Options.FunctionFeatures.empty())
    Defined->addFnAttr("target-features", Options.FunctionFeatures);
  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Defined);
  IRBuilder<> Builder(Entry);
  Value *Argument = Defined->getArg(0);
  Value *ReturnValue = nullptr;
  if (Callee) {
    FunctionCallee Imported = Result->getOrInsertFunction(*Callee, Signature);
    if (Options.WeakImport)
      cast<Function>(Imported.getCallee())
          ->setLinkage(GlobalValue::ExternalWeakLinkage);
    ReturnValue = Builder.CreateCall(Imported, {Argument});
  } else {
    ReturnValue =
        Builder.CreateAdd(Argument, ConstantInt::get(I32, Options.Addend));
  }
  Builder.CreateRet(ReturnValue);
  return Result;
}

std::vector<uint8_t> makeBitcode(StringRef ModuleName, StringRef Definition,
                                 std::optional<StringRef> Callee,
                                 const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(*ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeObject(StringRef ModuleName, StringRef Definition,
                                std::optional<StringRef> Callee,
                                const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  std::unique_ptr<TargetMachine> Machine = createMachine(Options.Cpu);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  legacy::PassManager Passes;
  require(!Machine->addPassesToEmitFile(Passes, Stream, nullptr,
                                        CodeGenFileType::ObjectFile, false),
          "fixture target machine cannot emit objects");
  Passes.run(*ModuleValue);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

Input makeInput(InputKind Kind, std::vector<uint8_t> Bytes) {
  std::array<uint8_t, 32> Digest = SHA256::hash(Bytes);
  return {Kind, Digest, std::move(Bytes)};
}

Request makeRequest(std::vector<Input> Inputs,
                    std::vector<std::string> ExpectedSymbols,
                    StringRef Target = "gfx942",
                    uint8_t CodeObjectVersion = 5) {
  llvm::sort(Inputs, [](const Input &Left, const Input &Right) {
    return std::tuple(Left.Digest, Left.Bytes.size(), Left.Kind) <
           std::tuple(Right.Digest, Right.Bytes.size(), Right.Kind);
  });
  llvm::sort(ExpectedSymbols);
  Request Result;
  Result.RequestId.fill(0x31);
  Result.Identity.fill(0x72);
  Result.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  Result.Target = Target.str();
  Result.CodeObjectVersion = CodeObjectVersion;
  Result.LinkOptions = {OptimizationLevel::O3, true, true};
  Result.Inputs = std::move(Inputs);
  Result.RequiredSymbols = ExpectedSymbols;
  Result.ExpectedDefinedSymbols = std::move(ExpectedSymbols);
  Result.MaxOutputBytes = 4 * 1024 * 1024;
  return Result;
}

std::set<std::string> inspectHsaco(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "fixture.hsaco"));
  if (!ObjectOrError)
    fail(toString(ObjectOrError.takeError()));
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  require(Elf != nullptr, "output is not ELF");
  require(Elf->getEMachine() == ELF::EM_AMDGPU, "output is not AMDGPU ELF");
  require(Elf->getEType() == ELF::ET_DYN, "output is not a shared ELF");
  require(Bytes.size() >= ELF::EI_NIDENT, "output has a truncated ELF header");
  require(Bytes[ELF::EI_CLASS] == ELF::ELFCLASS64 &&
              Bytes[ELF::EI_DATA] == ELF::ELFDATA2LSB &&
              Bytes[ELF::EI_VERSION] == ELF::EV_CURRENT,
          "output has the wrong ELF envelope");
  require(Bytes[ELF::EI_OSABI] == ELF::ELFOSABI_AMDGPU_HSA,
          "output does not use the AMDHSA OS ABI");
  require(Elf->getEIdentABIVersion() == ELF::ELFABIVERSION_AMDGPU_HSA_V5,
          "output does not use code-object V5");

  std::set<std::string> Symbols;
  for (SymbolRef Symbol : Elf->getDynamicSymbolIterators()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      fail(toString(FlagsOrError.takeError()));
    require((*FlagsOrError & SymbolRef::SF_Undefined) == 0,
            "output has an unresolved dynamic symbol");
    if ((*FlagsOrError & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0 ||
        (*FlagsOrError &
         (SymbolRef::SF_FormatSpecific | SymbolRef::SF_Hidden)) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      fail(toString(NameOrError.takeError()));
    if (!NameOrError->empty())
      Symbols.insert(NameOrError->str());
  }
  return Symbols;
}

Response runSuccess(const Request &RequestValue,
                    const std::set<std::string> &ExpectedSymbols) {
  Response Result = execute(RequestValue);
  if (!Result.LinkedOutput) {
    errs() << "failed request exports:";
    for (const std::string &Symbol : ExpectedSymbols)
      errs() << ' ' << Symbol;
    errs() << '\n';
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("expected worker success");
  }
  require(Result.FailureStage == Stage::Complete,
          "success reported the wrong stage");
  require(Result.LinkedOutput->Digest ==
              SHA256::hash(Result.LinkedOutput->Bytes),
          "success output digest is incorrect");
  require(inspectHsaco(Result.LinkedOutput->Bytes) == ExpectedSymbols,
          "HSACO exports do not match the request");
  return Result;
}

void requireFailure(const Request &RequestValue, Stage ExpectedStage) {
  Response Result = execute(RequestValue);
  require(!Result.LinkedOutput, "rejected request returned output bytes");
  if (Result.FailureStage != ExpectedStage) {
    errs() << "unexpected failure stage: expected "
           << static_cast<unsigned>(ExpectedStage) << ", got "
           << static_cast<unsigned>(Result.FailureStage) << '\n';
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("request failed at an unexpected stage");
  }
  require(!Result.Diagnostics.empty(), "failure omitted diagnostics");
  size_t Total = 0;
  for (const std::string &Diagnostic : Result.Diagnostics) {
    require(Diagnostic.size() <= MaxDiagnosticBytes,
            "diagnostic exceeded its byte bound");
    Total += Diagnostic.size();
  }
  require(Total <= MaxTotalDiagnosticBytes,
          "diagnostics exceeded their total byte bound");
}

void writeOutput(StringRef Path, ArrayRef<uint8_t> Bytes) {
  std::error_code ErrorCode;
  raw_fd_ostream Stream(Path, ErrorCode, sys::fs::OF_None);
  require(!ErrorCode, "could not open requested HSACO fixture output");
  Stream.write(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  Stream.close();
  require(!Stream.has_error(), "could not write requested HSACO fixture");
}

void testLldExitPolicy(int ExitCode) {
  pid_t Child = fork();
  require(Child >= 0, "could not fork LLD contract test");
  if (Child == 0) {
    fe2o3::worker::detail::enforceReusableLldResult({ExitCode, false});
    _exit(99);
  }
  int Status = 0;
  pid_t WaitResult;
  do {
    WaitResult = waitpid(Child, &Status, 0);
  } while (WaitResult < 0 && errno == EINTR);
  require(WaitResult == Child, "could not wait for LLD contract child");
  require(WIFEXITED(Status) && WEXITSTATUS(Status) == ExitCode,
          "non-reusable LLD result did not preserve its exit code");
}

} // namespace

int main(int ArgumentCount, char **Arguments) {
  require(ArgumentCount == 1 || ArgumentCount == 2,
          "usage: fe2o3-worker-pipeline-tests [OUTPUT.hsaco]");

  fe2o3::worker::detail::enforceReusableLldResult({0, true});
  fe2o3::worker::detail::enforceReusableLldResult({1, true});
  testLldExitPolicy(0);
  testLldExitPolicy(1);

  Request BitcodePair = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bc-entry", "bc_entry", "bc_helper")),
       makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bc-helper", "bc_helper", std::nullopt))},
      {"bc_entry", "bc_helper"});
  runSuccess(BitcodePair, {"bc_entry", "bc_helper"});

  Request AbsentLayout = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("absent-layout", "absent_layout", std::nullopt,
                             withLayout(LayoutMode::Absent)))},
      {"absent_layout"});
  runSuccess(AbsentLayout, {"absent_layout"});

  Request IncompatibleLayout = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bad-layout", "bad_layout", std::nullopt,
                             withLayout(LayoutMode::Incompatible)))},
      {"bad_layout"});
  requireFailure(IncompatibleLayout, Stage::BitcodeLink);

  Request CompatibleFeatures = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode(
                     "compatible-features", "compatible_features", std::nullopt,
                     withFunctionContract(
                         "gfx942", "-wavefrontsize32,+wavefrontsize64")))},
      {"compatible_features"});
  runSuccess(CompatibleFeatures, {"compatible_features"});

  Request WrongWavefront = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-wave", "wrong_wave", std::nullopt,
                             withFunctionFeatures(
                                 "+wavefrontsize32,-wavefrontsize64")))},
      {"wrong_wave"});
  requireFailure(WrongWavefront, Stage::BitcodeLink);

  Request WrongInstructionSet = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-isa", "wrong_isa", std::nullopt,
                             withFunctionFeatures("+gfx950-insts")))},
      {"wrong_isa"});
  requireFailure(WrongInstructionSet, Stage::BitcodeLink);

  Request WrongFunctionCpu =
      makeRequest({makeInput(InputKind::LlvmBitcode,
                             makeBitcode("wrong-cpu", "wrong_cpu", std::nullopt,
                                         withFunctionCpu("gfx950")))},
                  {"wrong_cpu"});
  requireFailure(WrongFunctionCpu, Stage::BitcodeLink);

  std::vector<uint8_t> MixedBitcode =
      makeBitcode("mixed-entry", "mixed_entry", "object_helper");
  std::vector<uint8_t> MixedObject =
      makeObject("mixed-helper", "object_helper", std::nullopt);
  Request Mixed =
      makeRequest({makeInput(InputKind::LlvmBitcode, MixedBitcode),
                   makeInput(InputKind::AmdGpuRelocatable, MixedObject)},
                  {"mixed_entry", "object_helper"});
  Response MixedFirst = runSuccess(Mixed, {"mixed_entry", "object_helper"});
  Response MixedSecond = runSuccess(Mixed, {"mixed_entry", "object_helper"});
  require(MixedFirst.LinkedOutput->Bytes == MixedSecond.LinkedOutput->Bytes,
          "identical requests produced different HSACO bytes");
  if (ArgumentCount == 2)
    writeOutput(Arguments[1], MixedFirst.LinkedOutput->Bytes);

  Request ObjectPair = makeRequest(
      {makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("object-entry", "object_entry", "object_leaf")),
       makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("object-leaf", "object_leaf", std::nullopt))},
      {"object_entry", "object_leaf"});
  runSuccess(ObjectPair, {"object_entry", "object_leaf"});

  Request ObjectAsBitcode = makeRequest(
      {makeInput(InputKind::LlvmBitcode, MixedObject)}, {"object_helper"});
  requireFailure(ObjectAsBitcode, Stage::InputValidation);

  Request BitcodeAsObject = makeRequest(
      {makeInput(InputKind::AmdGpuRelocatable, MixedBitcode)}, {"mixed_entry"});
  requireFailure(BitcodeAsObject, Stage::InputValidation);

  Request WrongTarget =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("wrong-target", "wrong_target",
                                        std::nullopt, withCpu("gfx1151")))},
                  {"wrong_target"});
  requireFailure(WrongTarget, Stage::InputValidation);

  Request WrongCodeObject =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("wrong-cov", "wrong_cov", std::nullopt,
                                        withCodeObjectVersion(4)))},
                  {"wrong_cov"});
  requireFailure(WrongCodeObject, Stage::InputValidation);

  Request WrongBitcodeCodeObject = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-bitcode-cov", "wrong_bitcode_cov",
                             std::nullopt, withCodeObjectVersion(4)))},
      {"wrong_bitcode_cov"});
  requireFailure(WrongBitcodeCodeObject, Stage::BitcodeLink);

  Request Unresolved = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("unresolved", "unresolved_entry", "missing"))},
      {"unresolved_entry"});
  requireFailure(Unresolved, Stage::InputValidation);

  Request UnresolvedWeak = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("unresolved-weak", "unresolved_weak_entry",
                             "missing_weak", withWeakImport()))},
      {"unresolved_weak_entry"});
  requireFailure(UnresolvedWeak, Stage::InputValidation);

  Request Duplicate =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-a", "duplicate",
                                        std::nullopt, withAddend(1))),
                   makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-b", "duplicate",
                                        std::nullopt, withAddend(2)))},
                  {"duplicate"});
  requireFailure(Duplicate, Stage::InputValidation);

  Request DuplicateWeak =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-weak-a", "duplicate_weak",
                                        std::nullopt, withWeakDefinition(1))),
                   makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-weak-b", "duplicate_weak",
                                        std::nullopt, withWeakDefinition(2)))},
                  {"duplicate_weak"});
  requireFailure(DuplicateWeak, Stage::InputValidation);

  Request OutputTooSmall = Mixed;
  OutputTooSmall.MaxOutputBytes = 1;
  requireFailure(OutputTooSmall, Stage::NativeLink);

  Request MissingExport = Mixed;
  MissingExport.ExpectedDefinedSymbols.push_back("phantom_export");
  llvm::sort(MissingExport.ExpectedDefinedSymbols);
  requireFailure(MissingExport, Stage::InputValidation);
  return 0;
}
