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
#include "llvm/Support/Endian.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <cstddef>
#include <cstdlib>
#include <cstring>
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

std::vector<uint8_t> makeTextIr(StringRef ModuleName, StringRef Definition,
                                std::optional<StringRef> Callee,
                                const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  std::string Buffer;
  raw_string_ostream Stream(Buffer);
  ModuleValue->print(Stream, nullptr);
  Stream.flush();
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t>
makeKernelBitcode(StringRef Name,
                  std::optional<std::array<uint32_t, 3>> RequiredWorkgroup =
                      std::array<uint32_t, 3>{256, 1, 1},
                  uint32_t MaxWorkgroup = 256) {
  LLVMContext Context;
  auto ModuleValue = std::make_unique<Module>("publication-kernel", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue->setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue->setDataLayout(Machine->createDataLayout());
  ModuleValue->addModuleFlag(Module::Error, "amdhsa_code_object_version", 500);

  FunctionType *Signature = FunctionType::get(Type::getVoidTy(Context), false);
  Function *Kernel = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                      Name, *ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr("target-cpu", "gfx942");
  Kernel->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
  std::string FlatWorkgroup =
      (Twine(MaxWorkgroup) + "," + Twine(MaxWorkgroup)).str();
  Kernel->addFnAttr("amdgpu-flat-work-group-size", FlatWorkgroup);
  if (RequiredWorkgroup) {
    Metadata *Workgroup[] = {
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[0])),
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[1])),
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[2]))};
    Kernel->setMetadata("reqd_work_group_size",
                        MDNode::get(Context, Workgroup));
  }
  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
  IRBuilder<>(Entry).CreateRetVoid();

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

Request makeV2Request(Input CompilerModule,
                      std::vector<Input> ExternalProviders,
                      std::vector<std::string> Imports,
                      std::vector<std::string> Exports,
                      std::vector<std::string> FinalSymbols) {
  std::vector<Input> Inputs = ExternalProviders;
  Inputs.push_back(CompilerModule);
  Request Result = makeRequest(std::move(Inputs), FinalSymbols);
  Result.Protocol = ProtocolVersion::V2;
  Result.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
  Result.WorkerExecutableDigest.fill(0x51);
  Result.WorkerExecutableBytes = 4096;
  Result.CompilerEnvelopeIdentity.fill(0x62);
  Result.CompilerModule = std::move(CompilerModule);
  Result.ExternalProviders = std::move(ExternalProviders);
  llvm::sort(Imports);
  llvm::sort(Exports);
  llvm::sort(FinalSymbols);
  Result.ImportSymbols = std::move(Imports);
  Result.ExportSymbols = std::move(Exports);
  Result.FinalSymbols = std::move(FinalSymbols);
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

Response requireFailure(const Request &RequestValue, Stage ExpectedStage) {
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
  return Result;
}

void requireDiagnostic(const Response &ResponseValue, StringRef Text) {
  require(llvm::any_of(ResponseValue.Diagnostics,
                       [Text](const std::string &Diagnostic) {
                         return StringRef(Diagnostic).contains(Text);
                       }),
          Text);
}

void requireInspectionFailure(ArrayRef<uint8_t> Bytes,
                              const Request &RequestValue,
                              StringRef ExpectedDiagnostic) {
  auto Inspection = inspectLinkedOutputForPublication(Bytes, RequestValue);
  require(!Inspection, "adversarial output passed publication inspection");
  std::string Diagnostic = toString(Inspection.takeError());
  if (!StringRef(Diagnostic).contains(ExpectedDiagnostic)) {
    errs() << "unexpected publication diagnostic: " << Diagnostic << '\n';
    fail("publication inspection failed for the wrong reason");
  }
}

uint64_t read64(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 8,
          "fixture ELF read is out of bounds");
  return support::endian::read64le(Bytes.data() + Offset);
}

uint32_t read32(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 4,
          "fixture ELF read is out of bounds");
  return support::endian::read32le(Bytes.data() + Offset);
}

uint16_t read16(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 2,
          "fixture ELF read is out of bounds");
  return support::endian::read16le(Bytes.data() + Offset);
}

void write32(MutableArrayRef<uint8_t> Bytes, size_t Offset, uint32_t Value) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 4,
          "fixture ELF write is out of bounds");
  support::endian::write32le(Bytes.data() + Offset, Value);
}

void write16(MutableArrayRef<uint8_t> Bytes, size_t Offset, uint16_t Value) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 2,
          "fixture ELF write is out of bounds");
  support::endian::write16le(Bytes.data() + Offset, Value);
}

void makeDynamicSymbolUndefined(
    std::vector<uint8_t> &Bytes, StringRef SymbolName,
    std::optional<StringRef> Replacement = std::nullopt) {
  constexpr size_t Elf64SectionTypeOffset = 4;
  constexpr size_t Elf64SectionOffsetOffset = 24;
  constexpr size_t Elf64SectionSizeOffset = 32;
  constexpr size_t Elf64SectionLinkOffset = 40;
  constexpr size_t Elf64SectionEntrySizeOffset = 56;
  constexpr size_t Elf64SymbolNameOffset = 0;
  constexpr size_t Elf64SymbolSectionIndexOffset = 6;
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  uint16_t SectionCount = read16(Bytes, 60);
  require(SectionEntrySize >= 64, "fixture has a short section header");

  for (uint16_t I = 0; I < SectionCount; ++I) {
    size_t Section = SectionTable + static_cast<uint64_t>(I) * SectionEntrySize;
    if (read32(Bytes, Section + Elf64SectionTypeOffset) != ELF::SHT_DYNSYM)
      continue;
    uint64_t Symbols = read64(Bytes, Section + Elf64SectionOffsetOffset);
    uint64_t SymbolBytes = read64(Bytes, Section + Elf64SectionSizeOffset);
    uint64_t SymbolSize = read64(Bytes, Section + Elf64SectionEntrySizeOffset);
    uint32_t StringsIndex = read32(Bytes, Section + Elf64SectionLinkOffset);
    require(StringsIndex < SectionCount && SymbolSize >= 24,
            "fixture has an invalid dynamic symbol section");
    size_t StringsSection =
        SectionTable + static_cast<uint64_t>(StringsIndex) * SectionEntrySize;
    uint64_t Strings = read64(Bytes, StringsSection + Elf64SectionOffsetOffset);
    uint64_t StringBytes =
        read64(Bytes, StringsSection + Elf64SectionSizeOffset);
    for (uint64_t Offset = 0; Offset < SymbolBytes; Offset += SymbolSize) {
      uint32_t NameOffset =
          read32(Bytes, Symbols + Offset + Elf64SymbolNameOffset);
      require(NameOffset < StringBytes,
              "fixture dynamic symbol has an invalid name");
      const char *Name =
          reinterpret_cast<const char *>(Bytes.data() + Strings + NameOffset);
      size_t Remaining = StringBytes - NameOffset;
      size_t Length = strnlen(Name, Remaining);
      require(Length < Remaining, "fixture dynamic symbol is unterminated");
      if (StringRef(Name, Length) != SymbolName)
        continue;
      if (Replacement) {
        require(Replacement->size() <= Length,
                "replacement dynamic symbol is too long");
        std::fill(Bytes.begin() + Strings + NameOffset,
                  Bytes.begin() + Strings + NameOffset + Length, 0);
        llvm::copy(*Replacement, Bytes.begin() + Strings + NameOffset);
      }
      write16(Bytes, Symbols + Offset + Elf64SymbolSectionIndexOffset,
              ELF::SHN_UNDEF);
      return;
    }
  }
  fail("fixture did not contain the requested dynamic symbol");
}

void corruptMetadataKey(std::vector<uint8_t> &Bytes, StringRef Key) {
  auto Position = std::search(Bytes.begin(), Bytes.end(), Key.bytes_begin(),
                              Key.bytes_end());
  require(Position != Bytes.end(), "fixture has no requested metadata key");
  *Position ^= 0x20;
}

void replaceMetadataByte(std::vector<uint8_t> &Bytes, StringRef Key,
                         uint8_t Expected, uint8_t Replacement) {
  auto Position = std::search(Bytes.begin(), Bytes.end(), Key.bytes_begin(),
                              Key.bytes_end());
  require(Position != Bytes.end(), "fixture has no requested metadata key");
  auto Value = Position + Key.size();
  auto End = std::min(Bytes.end(), Value + 8);
  Value = std::find(Value, End, Expected);
  require(Value != End, "fixture metadata value has an unexpected encoding");
  *Value = Replacement;
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
  require(ArgumentCount == 1 || ArgumentCount == 2 || ArgumentCount == 4,
          "usage: fe2o3-worker-pipeline-tests "
          "[OUTPUT.hsaco [INPUT.bc INPUT.o]]");

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

  Request TextPair = makeRequest(
      {makeInput(InputKind::LlvmTextIr,
                 makeTextIr("text-entry", "text_entry", "text_helper")),
       makeInput(InputKind::LlvmBitcode,
                 makeBitcode("text-helper", "text_helper", std::nullopt))},
      {"text_entry", "text_helper"});
  runSuccess(TextPair, {"text_entry", "text_helper"});

  Request InvalidText = makeRequest(
      {makeInput(InputKind::LlvmTextIr,
                 std::vector<uint8_t>{'n', 'o', 't', ' ', 'i', 'r'})},
      {});
  requireFailure(InvalidText, Stage::BitcodeLink);

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

  Request MixedV2 = makeV2Request(
      makeInput(InputKind::LlvmBitcode, MixedBitcode),
      {makeInput(InputKind::AmdGpuRelocatable, MixedObject)}, {"object_helper"},
      {"mixed_entry"}, {"mixed_entry", "object_helper"});
  Response MixedV2Response =
      runSuccess(MixedV2, {"mixed_entry", "object_helper"});
  require(MixedV2Response.Protocol == ProtocolVersion::V2,
          "V2 pipeline response lost its protocol version");
  require(MixedV2Response.CompilerEnvelopeIdentity ==
              MixedV2.CompilerEnvelopeIdentity,
          "V2 pipeline response lost its compiler envelope identity");

  Request TextV2 = makeV2Request(
      makeInput(InputKind::LlvmTextIr,
                makeTextIr("text-v2", "text_v2_entry", std::nullopt)),
      {}, {}, {"text_v2_entry"}, {"text_v2_entry"});
  Response TextV2Response = runSuccess(TextV2, {"text_v2_entry"});
  requireDiagnostic(TextV2Response,
                    "post_link.check=metadata status=ok kernels=0");

  Request PublicationKernel =
      makeV2Request(makeInput(InputKind::LlvmBitcode,
                              makeKernelBitcode("publication_kernel")),
                    {}, {}, {"publication_kernel"},
                    {"publication_kernel", "publication_kernel.kd"});
  Response PublicationResponse = runSuccess(
      PublicationKernel, {"publication_kernel", "publication_kernel.kd"});
  requireDiagnostic(PublicationResponse,
                    "post_link.check=target status=ok arch=gfx942");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=exports status=ok "
                    "symbols=[publication_kernel,publication_kernel.kd]");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=unresolved status=ok symbols=[]");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=metadata status=ok kernels=1");
  requireDiagnostic(PublicationResponse,
                    "post_link.kernel name=publication_kernel "
                    "symbol=publication_kernel.kd");
  requireDiagnostic(PublicationResponse, "wavefront_size=64");
  requireDiagnostic(PublicationResponse, "max_workgroup_size=256");
  requireDiagnostic(PublicationResponse, "reqd_workgroup_size=[256,1,1]");

  auto PublicationInspection = inspectLinkedOutputForPublication(
      PublicationResponse.LinkedOutput->Bytes, PublicationKernel);
  if (!PublicationInspection)
    fail(toString(PublicationInspection.takeError()));

  Request DescriptorOmitted = PublicationKernel;
  DescriptorOmitted.ExpectedDefinedSymbols = {"publication_kernel"};
  requireInspectionFailure(PublicationResponse.LinkedOutput->Bytes,
                           DescriptorOmitted,
                           "post_link.check=exports status=failed");

  std::vector<uint8_t> WrongOutputTarget =
      PublicationResponse.LinkedOutput->Bytes;
  constexpr size_t Elf64FlagsOffset = 48;
  uint32_t Flags = read32(WrongOutputTarget, Elf64FlagsOffset);
  write32(WrongOutputTarget, Elf64FlagsOffset, Flags & ~ELF::EF_AMDGPU_MACH);
  requireInspectionFailure(WrongOutputTarget, PublicationKernel,
                           "post_link.check=target status=failed "
                           "expected=gfx942");

  std::vector<uint8_t> UndefinedOutput =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(UndefinedOutput, "publication_kernel");
  requireInspectionFailure(UndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[publication_kernel]");

  std::vector<uint8_t> RuntimeUndefinedOutput =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(RuntimeUndefinedOutput, "publication_kernel",
                             "__ockl_bad");
  requireInspectionFailure(RuntimeUndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[__ockl_bad]");

  RuntimeUndefinedOutput = PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(RuntimeUndefinedOutput, "publication_kernel",
                             "__ocml_bad");
  requireInspectionFailure(RuntimeUndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[__ocml_bad]");

  std::vector<uint8_t> InvalidMetadata =
      PublicationResponse.LinkedOutput->Bytes;
  corruptMetadataKey(InvalidMetadata, ".wavefront_size");
  requireInspectionFailure(InvalidMetadata, PublicationKernel,
                           "post_link.check=metadata status=failed "
                           "reason=linked%20output%20has%20invalid%20AMDGPU%20"
                           "metadata%20schema");

  Request WrongRequiredWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("wrong_required_workgroup",
                                  std::array<uint32_t, 3>{128, 1, 1})),
      {}, {}, {"wrong_required_workgroup"},
      {"wrong_required_workgroup", "wrong_required_workgroup.kd"});
  Response WrongRequiredFailure =
      requireFailure(WrongRequiredWorkgroup, Stage::OutputInspection);
  requireDiagnostic(WrongRequiredFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=wrong_required_workgroup "
                    "field=reqd_workgroup_size expected=[256,1,1]");

  Request MissingRequiredWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("missing_required_workgroup", std::nullopt)),
      {}, {}, {"missing_required_workgroup"},
      {"missing_required_workgroup", "missing_required_workgroup.kd"});
  Response MissingRequiredFailure =
      requireFailure(MissingRequiredWorkgroup, Stage::OutputInspection);
  requireDiagnostic(MissingRequiredFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=missing_required_workgroup "
                    "field=reqd_workgroup_size expected=[256,1,1]");

  Request WrongMaxWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("wrong_max_workgroup",
                                  std::array<uint32_t, 3>{256, 1, 1}, 512)),
      {}, {}, {"wrong_max_workgroup"},
      {"wrong_max_workgroup", "wrong_max_workgroup.kd"});
  Response WrongMaxFailure =
      requireFailure(WrongMaxWorkgroup, Stage::OutputInspection);
  requireDiagnostic(WrongMaxFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=wrong_max_workgroup "
                    "field=max_flat_workgroup_size expected=256 actual=512");

  std::vector<uint8_t> WrongOutputWavefront =
      PublicationResponse.LinkedOutput->Bytes;
  replaceMetadataByte(WrongOutputWavefront, ".wavefront_size", 64, 32);
  requireInspectionFailure(WrongOutputWavefront, PublicationKernel,
                           "post_link.check=g1_profile status=failed "
                           "kernel=publication_kernel field=wavefront_size "
                           "expected=64 actual=32");

  Request WrongV2Worker = MixedV2;
  WrongV2Worker.WorkerBuildIdentity = "wrong-worker";
  requireFailure(WrongV2Worker, Stage::Toolchain);

  Request SameCardinalitySubstitution = MixedV2;
  SameCardinalitySubstitution.ImportSymbols = {"substituted_helper"};
  requireFailure(SameCardinalitySubstitution, Stage::InputValidation);

  Request SwappedRoles = MixedV2;
  std::swap(SwappedRoles.CompilerModule,
            SwappedRoles.ExternalProviders.front());
  requireFailure(SwappedRoles, Stage::InputValidation);

  Request ImportedSymbolDefinedByModule = MixedV2;
  ImportedSymbolDefinedByModule.ImportSymbols = {"mixed_entry"};
  ImportedSymbolDefinedByModule.ExportSymbols.clear();
  requireFailure(ImportedSymbolDefinedByModule, Stage::InputValidation);

  Request ExportDefinedOnlyByProvider = MixedV2;
  ExportDefinedOnlyByProvider.ImportSymbols.clear();
  ExportDefinedOnlyByProvider.ExportSymbols = {"object_helper"};
  requireFailure(ExportDefinedOnlyByProvider, Stage::InputValidation);

  Request V2Duplicate = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeBitcode("v2-duplicate-module", "v2_duplicate", std::nullopt,
                            withAddend(1))),
      {makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("v2-duplicate-provider", "v2_duplicate",
                            std::nullopt, withAddend(2)))},
      {}, {"v2_duplicate"}, {"v2_duplicate"});
  requireFailure(V2Duplicate, Stage::InputValidation);
  if (ArgumentCount >= 2)
    writeOutput(Arguments[1], MixedFirst.LinkedOutput->Bytes);
  if (ArgumentCount == 4) {
    writeOutput(Arguments[2], MixedBitcode);
    writeOutput(Arguments[3], MixedObject);
  }

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
