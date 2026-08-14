#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/ADT/SmallVector.h"
#include "llvm/Bitcode/BitcodeWriter.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCTargetOptions.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;
using namespace llvm;
using namespace llvm::object;

namespace {

constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral RequestDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST/V1\0";
constexpr StringLiteral RequestIdentityDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY/V1\0";
constexpr StringLiteral IdentityChallengeDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE/V1\0";

[[noreturn]] void fail(StringRef Message) {
  errs() << "machine-effect test failed: " << Message << '\n';
  std::abort();
}

void require(bool Condition, StringRef Message) {
  if (!Condition)
    fail(Message);
}

std::string takeError(Error ErrorValue) {
  return toString(std::move(ErrorValue));
}

std::unique_ptr<TargetMachine> createMachine() {
  static bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUDisassembler();
    LLVMInitializeAMDGPUAsmPrinter();
    return true;
  }();
  (void)Initialized;
  Triple TripleValue(TripleName);
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  require(TargetValue != nullptr, LookupError);
  TargetOptions Options;
  std::unique_ptr<TargetMachine> Result(TargetValue->createTargetMachine(
      TripleValue, "gfx942", "", Options, Reloc::PIC_, CodeModel::Small,
      CodeGenOptLevel::None));
  require(Result != nullptr, "cannot create gfx942 target machine");
  return Result;
}

void configureKernel(Function &Kernel, LLVMContext &Context) {
  Kernel.setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel.addFnAttr("target-cpu", "gfx942");
  Kernel.addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
  Kernel.addFnAttr("amdgpu-flat-work-group-size", "256,256");
  Metadata *Workgroup[] = {
      ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context), 256)),
      ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context), 1)),
      ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context), 1))};
  Kernel.setMetadata("reqd_work_group_size", MDNode::get(Context, Workgroup));
}

std::vector<uint8_t> makeKernelBitcode(bool WithHelper,
                                       uint32_t CodeObjectFlag = 600,
                                       bool WithNestedHelper = false) {
  LLVMContext Context;
  Module ModuleValue("physical-machine-effect-fixture", Context);
  auto Machine = createMachine();
  ModuleValue.setTargetTriple(Triple(TripleName));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                            CodeObjectFlag);

  Type *F32 = Type::getFloatTy(Context);
  PointerType *GlobalPointer = PointerType::get(Context, 1);
  FunctionType *HelperType =
      FunctionType::get(Type::getVoidTy(Context), {GlobalPointer}, false);
  Function *Helper = nullptr;
  Function *NestedHelper = nullptr;
  if (WithHelper) {
    if (WithNestedHelper) {
      NestedHelper = Function::Create(HelperType, GlobalValue::InternalLinkage,
                                      "alpha_nested_helper", ModuleValue);
      NestedHelper->addFnAttr(Attribute::NoInline);
      NestedHelper->addFnAttr("target-cpu", "gfx942");
      NestedHelper->addFnAttr("target-features",
                              "-wavefrontsize32,+wavefrontsize64");
      BasicBlock *NestedBlock =
          BasicBlock::Create(Context, "entry", NestedHelper);
      IRBuilder<> NestedBuilder(NestedBlock);
      NestedBuilder.CreateStore(ConstantFP::get(F32, 11.0),
                                NestedHelper->getArg(0));
      NestedBuilder.CreateRetVoid();
    }
    Helper = Function::Create(HelperType, GlobalValue::InternalLinkage,
                              "alpha_helper", ModuleValue);
    Helper->addFnAttr(Attribute::NoInline);
    Helper->addFnAttr("target-cpu", "gfx942");
    Helper->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
    BasicBlock *Block = BasicBlock::Create(Context, "entry", Helper);
    IRBuilder<> Builder(Block);
    if (NestedHelper)
      Builder.CreateCall(HelperType, NestedHelper, {Helper->getArg(0)});
    Builder.CreateStore(ConstantFP::get(F32, 9.0), Helper->getArg(0));
    Builder.CreateRetVoid();
  }

  auto addKernel = [&](StringRef Name, bool TwoInputs) {
    SmallVector<Type *, 3> Arguments;
    Arguments.push_back(GlobalPointer);
    if (TwoInputs)
      Arguments.push_back(GlobalPointer);
    Arguments.push_back(GlobalPointer);
    FunctionType *Type =
        FunctionType::get(Type::getVoidTy(Context), Arguments, false);
    Function *Kernel =
        Function::Create(Type, GlobalValue::ExternalLinkage, Name, ModuleValue);
    configureKernel(*Kernel, Context);
    BasicBlock *Block = BasicBlock::Create(Context, "entry", Kernel);
    IRBuilder<> Builder(Block);
    if (WithHelper && Name == "alpha") {
      Builder.CreateCall(HelperType, Helper, {Kernel->getArg(0)});
    }
    Value *Value = Builder.CreateLoad(F32, Kernel->getArg(0));
    if (TwoInputs)
      Value =
          Builder.CreateFAdd(Value, Builder.CreateLoad(F32, Kernel->getArg(1)));
    Builder.CreateStore(Value, Kernel->getArg(TwoInputs ? 2 : 1));
    Builder.CreateRetVoid();
  };
  addKernel("alpha", false);
  addKernel("zeta", true);

  SmallVector<char, 0> Bytes;
  raw_svector_ostream Stream(Bytes);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Bytes.begin(), Bytes.end());
}

Input input(std::vector<uint8_t> Bytes) {
  return {InputKind::LlvmBitcode, SHA256::hash(Bytes), std::move(Bytes)};
}

std::vector<uint8_t> finalize(std::vector<uint8_t> Bitcode,
                              StringRef Target = "gfx942:xnack-",
                              uint8_t CodeObjectVersion = 6) {
  Request RequestValue;
  RequestValue.RequestId.fill(0x41);
  RequestValue.Identity.fill(0x42);
  RequestValue.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  RequestValue.Target = Target.str();
  RequestValue.CodeObjectVersion = CodeObjectVersion;
  RequestValue.LinkOptions = {OptimizationLevel::O1, false, true};
  RequestValue.Inputs = {input(std::move(Bitcode))};
  RequestValue.RequiredSymbols = {"alpha", "zeta"};
  RequestValue.ExpectedDefinedSymbols = {"alpha", "alpha.kd", "zeta",
                                         "zeta.kd"};
  RequestValue.MaxOutputBytes = 4 * 1024 * 1024;
  Response Result = execute(RequestValue);
  if (!Result.LinkedOutput) {
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("fixture finalization failed");
  }
  return std::move(Result.LinkedOutput->Bytes);
}

PhysicalMachineEffectBudget generousBudget() { return {64, 32, 16, 16, 8}; }

PhysicalMachineEffectBudget scalarGemmBudget() { return {9, 8, 1, 1, 0}; }

PhysicalMachineEffectRequest
directRequest(std::vector<uint8_t> Payload,
              PhysicalMachineEffectBudget Budget = generousBudget()) {
  auto Identities = physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest Result;
  Result.AnalyzerIdentity = Identities.Analyzer;
  Result.ToolchainIdentity = Identities.Toolchain;
  Result.ExecutionChallenge.fill(0x50);
  Result.PayloadDigest = SHA256::hash(Payload);
  Result.PayloadBytes = Payload.size();
  Result.Entries = {{"alpha", Budget}, {"zeta", Budget}};
  Result.Payload = std::move(Payload);
  Result.RequestIdentity.fill(0x51);
  Result.RequestBytes = 4096;
  return Result;
}

PhysicalMachineEffectRequest directScalarGemmRequest(
    std::vector<uint8_t> Payload,
    PhysicalMachineEffectBudget Budget = scalarGemmBudget()) {
  auto Identities = physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest Result;
  Result.AnalyzerIdentity = Identities.Analyzer;
  Result.ToolchainIdentity = Identities.Toolchain;
  Result.ExecutionChallenge.fill(0x50);
  Result.PayloadDigest = SHA256::hash(Payload);
  Result.PayloadBytes = Payload.size();
  Result.Entries = {{"scalar_gemm_v1", Budget}};
  Result.Payload = std::move(Payload);
  Result.RequestIdentity.fill(0x51);
  Result.RequestBytes = 4096;
  return Result;
}

void appendU16(std::vector<uint8_t> &Output, uint16_t Value) {
  uint8_t Bytes[2];
  support::endian::write16le(Bytes, Value);
  Output.insert(Output.end(), Bytes, Bytes + 2);
}

void appendU32(std::vector<uint8_t> &Output, uint32_t Value) {
  uint8_t Bytes[4];
  support::endian::write32le(Bytes, Value);
  Output.insert(Output.end(), Bytes, Bytes + 4);
}

void appendU64(std::vector<uint8_t> &Output, uint64_t Value) {
  uint8_t Bytes[8];
  support::endian::write64le(Bytes, Value);
  Output.insert(Output.end(), Bytes, Bytes + 8);
}

void appendText(std::vector<uint8_t> &Output, StringRef Value) {
  appendU16(Output, static_cast<uint16_t>(Value.size()));
  Output.insert(Output.end(), Value.bytes_begin(), Value.bytes_end());
}

std::vector<uint8_t> encodeRequest(
    ArrayRef<uint8_t> Payload,
    ArrayRef<std::pair<std::string, PhysicalMachineEffectBudget>> Entries,
    std::optional<std::array<uint8_t, 32>> Digest = std::nullopt) {
  auto Identities = physicalMachineEffectIdentities();
  std::vector<uint8_t> Output;
  Output.insert(Output.end(), RequestDomain.bytes_begin(),
                RequestDomain.bytes_end());
  appendU32(Output, 0);
  appendU16(Output, 1);
  Output.insert(Output.end(), 32, 0x50);
  Output.insert(Output.end(), Identities.Analyzer.begin(),
                Identities.Analyzer.end());
  Output.insert(Output.end(), Identities.Toolchain.begin(),
                Identities.Toolchain.end());
  auto ActualDigest = Digest.value_or(SHA256::hash(Payload));
  Output.insert(Output.end(), ActualDigest.begin(), ActualDigest.end());
  appendU64(Output, Payload.size());
  appendU16(Output, static_cast<uint16_t>(Entries.size()));
  for (const auto &[Symbol, Budget] : Entries) {
    appendText(Output, Symbol);
    appendU32(Output, Budget.GlobalAddresses);
    appendU32(Output, Budget.GlobalReads);
    appendU32(Output, Budget.GlobalWrites);
    appendU32(Output, Budget.Returns);
    appendU32(Output, Budget.DirectCalls);
  }
  Output.insert(Output.end(), Payload.begin(), Payload.end());
  support::endian::write32le(Output.data() + RequestDomain.size(),
                             static_cast<uint32_t>(Output.size()));
  return Output;
}

std::array<uint8_t, 32> requestIdentity(ArrayRef<uint8_t> Bytes) {
  SHA256 Hash;
  Hash.update(arrayRefFromStringRef(RequestIdentityDomain));
  Hash.update(Bytes);
  return Hash.final();
}

void physicalAnalysisDerivesDeterministicClosedEffects() {
  auto Payload = finalize(makeKernelBitcode(false));
  auto RequestValue = directRequest(Payload);
  auto First = analyzeGfx942PhysicalMachineEffects(RequestValue);
  if (!First)
    fail(takeError(First.takeError()));
  auto Second = analyzeGfx942PhysicalMachineEffects(RequestValue);
  if (!Second)
    fail(takeError(Second.takeError()));
  auto FirstBytes = encodePhysicalMachineEffectEvidence(*First);
  if (!FirstBytes)
    fail(takeError(FirstBytes.takeError()));
  auto SecondBytes = encodePhysicalMachineEffectEvidence(*Second);
  if (!SecondBytes)
    fail(takeError(SecondBytes.takeError()));
  require(*FirstBytes == *SecondBytes, "evidence is not deterministic");
  require(First->Entries.size() == 2, "entry evidence count changed");
  require(First->Functions.size() == 2, "closed graph is not alpha/zeta");
  require(llvm::all_of(First->Functions,
                       [](const auto &Function) {
                         return Function.DirectCallees.empty();
                       }),
          "call-free fixture acquired call edges");
  require(llvm::any_of(First->Effects,
                       [](const auto &Effect) {
                         return Effect.Kind ==
                                PhysicalMachineEffectKind::GlobalRead;
                       }),
          "global read was not derived");
  require(llvm::any_of(First->Effects,
                       [](const auto &Effect) {
                         return Effect.Kind ==
                                PhysicalMachineEffectKind::GlobalWrite;
                       }),
          "global write was not derived");
  require(llvm::any_of(First->Effects,
                       [](const auto &Effect) {
                         return Effect.Kind ==
                                PhysicalMachineEffectKind::Return;
                       }),
          "return was not derived");
}

void identityProbeBindsFreshChallenge() {
  std::vector<uint8_t> Request;
  Request.insert(Request.end(), IdentityChallengeDomain.bytes_begin(),
                 IdentityChallengeDomain.bytes_end());
  appendU32(Request, 0);
  appendU16(Request, 1);
  Request.insert(Request.end(), 32, 0x61);
  support::endian::write32le(Request.data() + IdentityChallengeDomain.size(),
                             static_cast<uint32_t>(Request.size()));
  auto First = encodePhysicalMachineEffectIdentityResponse(Request);
  if (!First)
    fail(takeError(First.takeError()));
  auto Second = encodePhysicalMachineEffectIdentityResponse(Request);
  if (!Second)
    fail(takeError(Second.takeError()));
  require(*First == *Second, "identity response is not deterministic");
  require(std::search(First->begin(), First->end(), Request.end() - 32,
                      Request.end()) != First->end(),
          "identity response omitted challenge");

  Request.back() ^= 1;
  auto Changed = encodePhysicalMachineEffectIdentityResponse(Request);
  if (!Changed)
    fail(takeError(Changed.takeError()));
  require(*Changed != *First, "identity response replayed stale challenge");
}

void decoderBindsBytesSymbolsAndIdentities() {
  std::vector<uint8_t> Payload = finalize(makeKernelBitcode(false));
  std::vector<std::pair<std::string, PhysicalMachineEffectBudget>> Entries = {
      {"alpha", generousBudget()}, {"zeta", generousBudget()}};
  std::vector<uint8_t> Bytes = encodeRequest(Payload, Entries);
  auto Decoded = decodePhysicalMachineEffectRequest(Bytes);
  if (!Decoded)
    fail(takeError(Decoded.takeError()));
  require(Decoded->RequestIdentity == requestIdentity(Bytes),
          "request identity does not bind canonical bytes");

  Bytes.back() ^= 1;
  auto Mutated = decodePhysicalMachineEffectRequest(Bytes);
  require(!Mutated, "payload mutation retained its request");
  consumeError(Mutated.takeError());

  Entries[0].first = "omega";
  Bytes = encodeRequest(Payload, Entries);
  auto WrongSymbol = decodePhysicalMachineEffectRequest(Bytes);
  require(!WrongSymbol, "non alpha/zeta symbol was accepted");
  consumeError(WrongSymbol.takeError());

  Bytes = encodeRequest(Payload, {{"alpha", generousBudget()}});
  size_t AnalyzerOffset = RequestDomain.size() + 4 + 2 + 32;
  Bytes[AnalyzerOffset] ^= 1;
  auto WrongIdentity = decodePhysicalMachineEffectRequest(Bytes);
  require(!WrongIdentity, "analyzer identity substitution was accepted");
  consumeError(WrongIdentity.takeError());

  Bytes = encodeRequest(Payload, {{"scalar_gemm_v1", scalarGemmBudget()}});
  auto Scalar = decodePhysicalMachineEffectRequest(Bytes);
  if (!Scalar)
    fail(takeError(Scalar.takeError()));
  require(Scalar->Entries.size() == 1 &&
              Scalar->Entries.front().Symbol == "scalar_gemm_v1",
          "canonical scalar GEMM request was rejected");

  Bytes = encodeRequest(Payload, {{"alpha", generousBudget()},
                                  {"scalar_gemm_v1", scalarGemmBudget()}});
  auto MixedProfile = decodePhysicalMachineEffectRequest(Bytes);
  require(!MixedProfile, "mixed alpha/scalar profile was accepted");
  consumeError(MixedProfile.takeError());
}

size_t symbolFileOffset(ArrayRef<uint8_t> Payload, StringRef Name) {
  StringRef Data(reinterpret_cast<const char *>(Payload.data()),
                 Payload.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "mutation.hsaco"));
  if (!ObjectOrError)
    fail(takeError(ObjectOrError.takeError()));
  for (SymbolRef Symbol : (*ObjectOrError)->symbols()) {
    auto SymbolName = Symbol.getName();
    if (!SymbolName)
      fail(takeError(SymbolName.takeError()));
    if (*SymbolName != Name)
      continue;
    auto Address = Symbol.getAddress();
    if (!Address)
      fail(takeError(Address.takeError()));
    auto Section = Symbol.getSection();
    if (!Section)
      fail(takeError(Section.takeError()));
    require(*Section != (*ObjectOrError)->section_end(),
            "mutation symbol is absolute");
    auto Contents = (**Section).getContents();
    if (!Contents)
      fail(takeError(Contents.takeError()));
    uint64_t Offset = *Address - (**Section).getAddress();
    const char *Pointer = Contents->data() + Offset;
    require(Pointer >= Data.data() && Pointer < Data.data() + Data.size(),
            "mutation symbol is outside payload");
    return static_cast<size_t>(Pointer - Data.data());
  }
  fail("mutation symbol is absent");
}

struct DecodedSite {
  size_t FileOffset = 0;
  uint64_t Address = 0;
  uint64_t Size = 0;
  std::string Name;
  std::vector<std::string> RegisterOperands;
};

struct ElfMutationLayout {
  std::vector<size_t> ExecutableProgramHeaders;
  std::vector<size_t> NonLoadProgramHeaders;
  std::map<uint32_t, std::vector<size_t>> ProgramHeaders;
  std::map<std::string, size_t> SectionHeaders;
  std::map<std::string, uint32_t> SectionIndices;
  std::map<std::pair<uint32_t, std::string>, size_t> Symbols;
  std::map<int64_t, size_t> DynamicEntries;
};

size_t payloadOffset(ArrayRef<uint8_t> Payload, const void *Pointer,
                     size_t Size) {
  const auto *Bytes = static_cast<const uint8_t *>(Pointer);
  require(Bytes >= Payload.data() && Bytes <= Payload.data() + Payload.size() &&
              Size <=
                  static_cast<size_t>(Payload.data() + Payload.size() - Bytes),
          "ELF mutation target is outside payload");
  return static_cast<size_t>(Bytes - Payload.data());
}

ElfMutationLayout inspectElfMutationLayout(ArrayRef<uint8_t> Payload) {
  StringRef Data(reinterpret_cast<const char *>(Payload.data()),
                 Payload.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "layout.hsaco"));
  if (!ObjectOrError)
    fail(takeError(ObjectOrError.takeError()));
  auto *Object = dyn_cast<ELFObjectFile<ELF64LE>>(ObjectOrError->get());
  require(Object != nullptr, "mutation fixture is not ELF64LE");
  const ELFFile<ELF64LE> &File = Object->getELFFile();
  auto Headers = File.program_headers();
  if (!Headers)
    fail(takeError(Headers.takeError()));
  ElfMutationLayout Result;
  for (const ELF64LE::Phdr &Header : *Headers) {
    size_t Offset = payloadOffset(Payload, &Header, sizeof(Header));
    Result.ProgramHeaders[Header.p_type].push_back(Offset);
    if (Header.p_type == ELF::PT_LOAD && (Header.p_flags & ELF::PF_X) != 0)
      Result.ExecutableProgramHeaders.push_back(Offset);
    else if (Header.p_type != ELF::PT_LOAD)
      Result.NonLoadProgramHeaders.push_back(Offset);
  }

  auto Sections = File.sections();
  if (!Sections)
    fail(takeError(Sections.takeError()));
  for (size_t Index = 0; Index < Sections->size(); ++Index) {
    const ELF64LE::Shdr &Section = (*Sections)[Index];
    auto Name = File.getSectionName(Section);
    if (!Name)
      fail(takeError(Name.takeError()));
    Result.SectionHeaders.emplace(
        Name->str(), payloadOffset(Payload, &Section, sizeof(Section)));
    Result.SectionIndices.emplace(Name->str(), static_cast<uint32_t>(Index));
    if (Section.sh_type == ELF::SHT_DYNAMIC) {
      auto Entries = File.getSectionContentsAsArray<ELF64LE::Dyn>(Section);
      if (!Entries)
        fail(takeError(Entries.takeError()));
      for (const ELF64LE::Dyn &Entry : *Entries)
        require(Result.DynamicEntries
                    .emplace(Entry.getTag(),
                             payloadOffset(Payload, &Entry, sizeof(Entry)))
                    .second,
                "fixture dynamic table has duplicate tags");
    }
    if (Section.sh_type != ELF::SHT_SYMTAB &&
        Section.sh_type != ELF::SHT_DYNSYM)
      continue;
    auto StringTable = File.getStringTableForSymtab(Section, *Sections);
    if (!StringTable)
      fail(takeError(StringTable.takeError()));
    auto Symbols = File.symbols(&Section);
    if (!Symbols)
      fail(takeError(Symbols.takeError()));
    for (const ELF64LE::Sym &Symbol : *Symbols) {
      auto SymbolName = Symbol.getName(*StringTable);
      if (!SymbolName)
        fail(takeError(SymbolName.takeError()));
      if (!SymbolName->empty())
        Result.Symbols.emplace(
            std::pair<uint32_t, std::string>{Section.sh_type,
                                             SymbolName->str()},
            payloadOffset(Payload, &Symbol, sizeof(Symbol)));
    }
  }
  return Result;
}

std::vector<DecodedSite> decodeSymbolSites(ArrayRef<uint8_t> Payload,
                                           StringRef Name,
                                           bool RequireValid = true) {
  StringRef Data(reinterpret_cast<const char *>(Payload.data()),
                 Payload.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "decode.hsaco"));
  if (!ObjectOrError)
    fail(takeError(ObjectOrError.takeError()));
  uint64_t Address = 0;
  uint64_t Size = 0;
  bool Found = false;
  for (SymbolRef Symbol : (*ObjectOrError)->symbols()) {
    auto SymbolName = Symbol.getName();
    if (!SymbolName)
      fail(takeError(SymbolName.takeError()));
    if (*SymbolName != Name)
      continue;
    auto SymbolAddress = Symbol.getAddress();
    if (!SymbolAddress)
      fail(takeError(SymbolAddress.takeError()));
    Address = *SymbolAddress;
    Size = ELFSymbolRef(Symbol).getSize();
    Found = true;
    break;
  }
  require(Found && Size != 0, "decode symbol is absent or empty");
  size_t BaseOffset = symbolFileOffset(Payload, Name);
  require(BaseOffset <= Payload.size() && Size <= Payload.size() - BaseOffset,
          "decode symbol bytes are outside payload");

  Triple TripleValue(TripleName);
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  require(TargetValue != nullptr, LookupError);
  std::unique_ptr<MCRegisterInfo> Registers(
      TargetValue->createMCRegInfo(TripleValue));
  std::unique_ptr<MCInstrInfo> Instructions(TargetValue->createMCInstrInfo());
  std::unique_ptr<MCSubtargetInfo> Subtarget(
      TargetValue->createMCSubtargetInfo(TripleValue, "gfx942", "-xnack"));
  MCTargetOptions Options;
  std::unique_ptr<MCAsmInfo> AsmInfo(
      TargetValue->createMCAsmInfo(*Registers, TripleValue, Options));
  MCContext Context(TripleValue, AsmInfo.get(), Registers.get(),
                    Subtarget.get(), nullptr, &Options);
  std::unique_ptr<MCDisassembler> Disassembler(
      TargetValue->createMCDisassembler(*Subtarget, Context));
  require(Registers && Instructions && Subtarget && AsmInfo && Disassembler,
          "test MC tables are unavailable");

  ArrayRef<uint8_t> Bytes = Payload.slice(BaseOffset, Size);
  std::vector<DecodedSite> Result;
  uint64_t Offset = 0;
  while (Offset < Bytes.size()) {
    if (llvm::all_of(Bytes.drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Instruction;
    uint64_t InstructionSize = 0;
    auto Status = Disassembler->getInstruction(Instruction, InstructionSize,
                                               Bytes.drop_front(Offset),
                                               Address + Offset, nulls());
    if (Status != MCDisassembler::Success || InstructionSize == 0 ||
        InstructionSize > Bytes.size() - Offset) {
      require(!RequireValid, "reviewer repro cannot decode symbol");
      return {};
    }
    std::vector<std::string> RegisterOperands;
    for (const MCOperand &Operand : Instruction)
      RegisterOperands.push_back(
          Operand.isReg() ? Registers->getName(Operand.getReg()) : "");
    Result.push_back({BaseOffset + static_cast<size_t>(Offset),
                      Address + Offset, InstructionSize,
                      Instructions->getName(Instruction.getOpcode()).str(),
                      std::move(RegisterOperands)});
    Offset += InstructionSize;
  }
  return Result;
}

std::optional<std::vector<uint8_t>> scalarArtifactFromEnvironment() {
  const char *Path = std::getenv("FE2O3_SCALAR_GEMM_V1_HSACO");
  if (Path == nullptr)
    return std::nullopt;
  std::ifstream Input(Path, std::ios::binary);
  require(Input.good(), "cannot open scalar GEMM artifact");
  std::vector<uint8_t> Payload((std::istreambuf_iterator<char>(Input)),
                               std::istreambuf_iterator<char>());
  require(!Payload.empty(), "scalar GEMM artifact is empty");
  return Payload;
}

std::string rejectedScalarDiagnostic(std::vector<uint8_t> Payload) {
  auto Result = analyzeGfx942PhysicalMachineEffects(
      directScalarGemmRequest(std::move(Payload)));
  require(!Result, "scalar GEMM mutation was accepted");
  return takeError(Result.takeError());
}

void requireScalarRejectedWith(std::vector<uint8_t> Payload,
                               StringRef Fragment) {
  std::string Diagnostic = rejectedScalarDiagnostic(std::move(Payload));
  require(StringRef(Diagnostic).contains(Fragment),
          (Twine("unexpected scalar rejection; wanted '") + Fragment +
           "': " + Diagnostic)
              .str());
}

void exactScalarGemmArtifactIsAccepted() {
  auto MaybePayload = scalarArtifactFromEnvironment();
  if (!MaybePayload)
    return;
  const std::vector<uint8_t> &Payload = *MaybePayload;
  constexpr std::array<uint8_t, 32> ExpectedDigest = {
      0xac, 0x1d, 0xa7, 0x0c, 0x69, 0xa5, 0x03, 0x8b, 0x88, 0x7b, 0x45,
      0x9d, 0xec, 0xe4, 0x08, 0x02, 0x66, 0x8c, 0x41, 0xbc, 0xf9, 0x8f,
      0x62, 0x1d, 0x7d, 0x12, 0x73, 0xd2, 0xf6, 0x1b, 0xa2, 0xc9,
  };
  require(Payload.size() == 10128, "scalar GEMM artifact length changed");
  require(SHA256::hash(Payload) == ExpectedDigest,
          "scalar GEMM artifact digest changed");

  std::set<std::string> Opcodes;
  for (const DecodedSite &Site : decodeSymbolSites(Payload, "scalar_gemm_v1"))
    Opcodes.insert(Site.Name);
  const std::set<std::string> ExpectedOpcodes = {
      "GLOBAL_LOAD_DWORD_vi",
      "GLOBAL_STORE_DWORD_vi",
      "S_ADDC_U32_vi",
      "S_ADD_I32_vi",
      "S_ADD_U32_vi",
      "S_AND_B64_vi",
      "S_BRANCH_vi",
      "S_CBRANCH_EXECZ_vi",
      "S_CBRANCH_SCC1_vi",
      "S_CBRANCH_VCCNZ_vi",
      "S_CMP_GE_U32_vi",
      "S_CMP_LG_U64_vi",
      "S_CSELECT_B64_vi",
      "S_ENDPGM_vi",
      "S_LOAD_DWORDX2_IMM_vi",
      "S_LOAD_DWORD_IMM_vi",
      "S_LSHL_B64_vi",
      "S_LSHR_B64_vi",
      "S_MOV_B32_vi",
      "S_MOV_B64_vi",
      "S_MUL_HI_U32_vi",
      "S_MUL_I32_vi",
      "S_NOP_vi",
      "S_OR_B64_vi",
      "S_OR_SAVEEXEC_B64_vi",
      "S_SUBB_U32_vi",
      "S_SUB_U32_vi",
      "S_WAITCNT_vi",
      "V_ACCVGPR_READ_B32_vi",
      "V_ACCVGPR_WRITE_B32_vi",
      "V_ADD3_U32_vi",
      "V_ADDC_CO_U32_e32_gfx9",
      "V_ADD_CO_U32_e32_gfx9",
      "V_ADD_F32_e64_vi",
      "V_CMP_EQ_U32_e64_vi",
      "V_CMP_GE_U32_e64_vi",
      "V_CMP_LT_U64_e64_vi",
      "V_CMP_NE_U32_e64_vi",
      "V_CNDMASK_B32_e64_vi",
      "V_CVT_F32_U32_e64_vi",
      "V_CVT_U32_F32_e64_vi",
      "V_FMAC_F32_e64_vi",
      "V_LSHLREV_B64_vi",
      "V_LSHL_ADD_U64_vi",
      "V_LSHRREV_B64_vi",
      "V_MAD_U64_U32_vi",
      "V_MOV_B32_e32_vi",
      "V_MOV_B64_e32_vi",
      "V_MUL_F32_e64_vi",
      "V_MUL_HI_U32_vi",
      "V_MUL_LO_U32_vi",
      "V_OR_B32_e64_vi",
      "V_RCP_F32_e64_vi",
      "V_READFIRSTLANE_B32_vi",
      "V_READLANE_B32_vi",
      "V_SUBB_CO_U32_e64_gfx9",
      "V_SUB_CO_U32_e64_gfx9",
      "V_SUB_U32_e64_gfx9",
      "V_TRUNC_F32_e64_vi",
      "V_WRITELANE_B32_vi",
  };
  require(Opcodes == ExpectedOpcodes,
          "scalar GEMM LLVM MC opcode closure changed");

  auto First =
      analyzeGfx942PhysicalMachineEffects(directScalarGemmRequest(Payload));
  if (!First)
    fail(takeError(First.takeError()));
  auto Second =
      analyzeGfx942PhysicalMachineEffects(directScalarGemmRequest(Payload));
  if (!Second)
    fail(takeError(Second.takeError()));
  auto FirstBytes = encodePhysicalMachineEffectEvidence(*First);
  if (!FirstBytes)
    fail(takeError(FirstBytes.takeError()));
  auto SecondBytes = encodePhysicalMachineEffectEvidence(*Second);
  if (!SecondBytes)
    fail(takeError(SecondBytes.takeError()));
  require(*FirstBytes == *SecondBytes,
          "scalar GEMM evidence is not deterministic");
  require(First->Entries.size() == 1 &&
              First->Entries.front().Symbol == "scalar_gemm_v1" &&
              First->Entries.front().CodeOffset == 0x1b00 &&
              First->Entries.front().CodeSize == 0xad0,
          "scalar GEMM entry closure changed");
  require(First->Functions.size() == 1 &&
              First->Functions.front().Symbol == "scalar_gemm_v1" &&
              First->Functions.front().DirectCallees.empty(),
          "scalar GEMM function closure changed");

  size_t Addresses = 0;
  size_t Reads = 0;
  size_t Writes = 0;
  size_t Returns = 0;
  size_t FourByteReads = 0;
  size_t EightByteReads = 0;
  for (const PhysicalMachineEffect &Effect : First->Effects) {
    require(Effect.EntrySymbol == "scalar_gemm_v1" &&
                Effect.FunctionSymbol == "scalar_gemm_v1",
            "scalar GEMM effect escaped its closed function");
    switch (Effect.Kind) {
    case PhysicalMachineEffectKind::GlobalAddress:
      ++Addresses;
      require(Effect.ByteWidth == 8, "scalar GEMM address width changed");
      break;
    case PhysicalMachineEffectKind::GlobalRead:
      ++Reads;
      FourByteReads += Effect.ByteWidth == 4;
      EightByteReads += Effect.ByteWidth == 8;
      break;
    case PhysicalMachineEffectKind::GlobalWrite:
      ++Writes;
      require(Effect.ByteWidth == 4, "scalar GEMM write width changed");
      break;
    case PhysicalMachineEffectKind::Return:
      ++Returns;
      require(Effect.ByteWidth == 0, "scalar GEMM return width changed");
      break;
    }
  }
  require(Addresses == 9 && Reads == 8 && Writes == 1 && Returns == 1 &&
              FourByteReads == 5 && EightByteReads == 3,
          "scalar GEMM static effect counts changed");
  outs() << "accepted scalar_gemm_v1: address=9 read=8 write=1 return=1 "
            "call=0\n";

  auto Sites = decodeSymbolSites(Payload, "scalar_gemm_v1");
  auto Nop = llvm::find_if(Sites, [](const DecodedSite &Site) {
    return Site.Name == "S_NOP_vi" && Site.Size == 4;
  });
  require(Nop != Sites.end(), "scalar GEMM lacks mutation NOP");
  std::vector<uint8_t> Mutated = Payload;
  support::endian::write32le(Mutated.data() + Nop->FileOffset, 0xbf8e0000);
  requireScalarRejectedWith(std::move(Mutated),
                            "unsupported instruction S_SLEEP_vi");

  auto GlobalLoad = llvm::find_if(Sites, [](const DecodedSite &Site) {
    return Site.Name == "GLOBAL_LOAD_DWORD_vi" && Site.Size == 8;
  });
  require(GlobalLoad != Sites.end(), "scalar GEMM lacks mutation load");
  Mutated = Payload;
  uint32_t Load =
      support::endian::read32le(Payload.data() + GlobalLoad->FileOffset);
  support::endian::write32le(Mutated.data() + GlobalLoad->FileOffset,
                             Load + 0x00040000);
  requireScalarRejectedWith(
      std::move(Mutated),
      "unsupported memory instruction GLOBAL_LOAD_DWORDX2_vi");

  auto Backward = llvm::find_if(Sites, [](const DecodedSite &Site) {
    return Site.Name == "S_CBRANCH_VCCNZ_vi" && Site.Size == 4;
  });
  require(Backward != Sites.end(), "scalar GEMM lacks inner loop latch");
  uint32_t Branch =
      support::endian::read32le(Payload.data() + Backward->FileOffset);
  Mutated = Payload;
  support::endian::write32le(Mutated.data() + Backward->FileOffset,
                             0xbf850000 | (Branch & 0xffff));
  requireScalarRejectedWith(std::move(Mutated),
                            "unsupported scalar GEMM backward branch");

  Mutated = Payload;
  support::endian::write32le(Mutated.data() + Backward->FileOffset,
                             (Branch & 0xffff0000) | ((Branch + 1) & 0xffff));
  requireScalarRejectedWith(std::move(Mutated),
                            "scalar GEMM branch profile mismatch");
}

void patchSwapCallToImmediate(std::vector<uint8_t> &Payload, StringRef Caller,
                              StringRef Callee, uint8_t DestinationRegister) {
  auto CallerSites = decodeSymbolSites(Payload, Caller);
  auto CalleeSites = decodeSymbolSites(Payload, Callee);
  require(!CalleeSites.empty(), "immediate-call callee is empty");
  auto Call = llvm::find_if(CallerSites, [](const DecodedSite &Site) {
    return Site.Name == "S_SWAPPC_B64_vi";
  });
  require(Call != CallerSites.end() && Call->Size == 4,
          "immediate-call mutation lacks S_SWAPPC");
  int64_t Delta = static_cast<int64_t>(CalleeSites.front().Address) -
                  static_cast<int64_t>(Call->Address + Call->Size);
  require(Delta % 4 == 0 && Delta / 4 >= std::numeric_limits<int16_t>::min() &&
              Delta / 4 <= std::numeric_limits<int16_t>::max(),
          "immediate-call target is outside SOPK displacement");
  uint32_t Encoding = 0xba800000 |
                      (static_cast<uint32_t>(DestinationRegister) << 16) |
                      static_cast<uint16_t>(Delta / 4);
  support::endian::write32le(Payload.data() + Call->FileOffset, Encoding);

  auto Changed = decodeSymbolSites(Payload, Caller);
  auto Immediate = llvm::find_if(Changed, [&](const DecodedSite &Site) {
    return Site.Address == Call->Address && Site.Name == "S_CALL_B64_vi" &&
           Site.Size == Call->Size;
  });
  require(Immediate != Changed.end() && Immediate->RegisterOperands.size() == 2,
          "SOPK mutation did not encode immediate S_CALL_B64_vi");
  if (DestinationRegister == 30)
    require(Immediate->RegisterOperands[0] == "SGPR30_SGPR31",
            "S_CALL ABI destination did not decode exactly");
  else
    require(Immediate->RegisterOperands[0] != "SGPR30_SGPR31",
            "alternate S_CALL destination decoded as ABI pair");
}

std::string rejectedDiagnostic(std::vector<uint8_t> Payload) {
  auto Result =
      analyzeGfx942PhysicalMachineEffects(directRequest(std::move(Payload)));
  require(!Result, "reviewer mutation was accepted");
  return takeError(Result.takeError());
}

void requireRejectedWith(std::vector<uint8_t> Payload, StringRef Fragment) {
  std::string Diagnostic = rejectedDiagnostic(std::move(Payload));
  std::string Message =
      (Twine("unexpected rejection; wanted '") + Fragment + "': " + Diagnostic)
          .str();
  require(StringRef(Diagnostic).contains(Fragment), Message);
}

void replaceText(std::vector<uint8_t> &Bytes, StringRef Expected,
                 StringRef Replacement) {
  require(Expected.size() == Replacement.size(),
          "test mutation changes encoded text length");
  auto Position = std::search(Bytes.begin(), Bytes.end(),
                              Expected.bytes_begin(), Expected.bytes_end());
  require(Position != Bytes.end(), "test mutation text is absent");
  llvm::copy(Replacement, Position);
}

void exactProductionProfileRejectsAlternatives() {
  constexpr StringLiteral MetadataTarget = "amdgcn-amd-amdhsa--gfx942:xnack-";
  require(matchesPhysicalMachineEffectMetadataTargetV1(MetadataTarget),
          "exact production metadata target was rejected");
  for (StringRef Rejected : {
           StringRef("amdgcn-amd-amdhsa--gfx942"),
           StringRef("amdgcn-amd-amdhsa--gfx942:xnack+"),
           StringRef("amdgcn-amd-amdhsa--gfx942:xnack?"),
           StringRef("amdgcn-amd-amdhsa--gfx942:xnack-:xnack-"),
           StringRef("amdgcn-amd-amdhsa--gfx942:sramecc-:xnack-"),
       })
    require(!matchesPhysicalMachineEffectMetadataTargetV1(Rejected),
            "non-production metadata target was accepted");

  auto RequireRejected = [](std::vector<uint8_t> Payload, StringRef Message) {
    auto RequestValue = directRequest(std::move(Payload));
    auto Result = analyzeGfx942PhysicalMachineEffects(RequestValue);
    require(!Result, Message);
    consumeError(Result.takeError());
  };
  RequireRejected(finalize(makeKernelBitcode(false), "gfx942", 6),
                  "bare gfx942 HSACO was accepted");
  RequireRejected(finalize(makeKernelBitcode(false), "gfx942:xnack+", 6),
                  "xnack+ HSACO was accepted");
  RequireRejected(finalize(makeKernelBitcode(false, 500), "gfx942:xnack-", 5),
                  "COV5 HSACO was accepted");

  std::vector<uint8_t> Malformed = finalize(makeKernelBitcode(false));
  replaceText(Malformed, MetadataTarget, "amdgcn-amd-amdhsa--gfx942:xnack?");
  RequireRejected(std::move(Malformed),
                  "malformed metadata target was accepted");
}

void targetDescriptorAndEffectExpansionFailClosed() {
  std::vector<uint8_t> Payload = finalize(makeKernelBitcode(false));

  std::vector<uint8_t> WrongTarget = Payload;
  constexpr size_t FlagsOffset = 48;
  require(WrongTarget.size() > FlagsOffset + 4, "ELF header is truncated");
  support::endian::write32le(WrongTarget.data() + FlagsOffset, 0);
  auto TargetRequest = directRequest(std::move(WrongTarget));
  auto TargetResult = analyzeGfx942PhysicalMachineEffects(TargetRequest);
  require(!TargetResult, "wrong gfx target was accepted");
  consumeError(TargetResult.takeError());

  std::vector<uint8_t> WrongDescriptor = Payload;
  size_t DescriptorOffset = symbolFileOffset(WrongDescriptor, "alpha.kd");
  WrongDescriptor[DescriptorOffset + 12] = 1;
  auto DescriptorRequest = directRequest(std::move(WrongDescriptor));
  auto DescriptorResult =
      analyzeGfx942PhysicalMachineEffects(DescriptorRequest);
  require(!DescriptorResult, "descriptor mutation was accepted");
  consumeError(DescriptorResult.takeError());

  PhysicalMachineEffectBudget Tight = generousBudget();
  Tight.GlobalReads = 0;
  auto TightRequest = directRequest(Payload, Tight);
  auto TightResult = analyzeGfx942PhysicalMachineEffects(TightRequest);
  require(!TightResult, "effect expansion was accepted");
  consumeError(TightResult.takeError());
}

void loaderViewMutationsFailClosed() {
  std::vector<uint8_t> Payload = finalize(makeKernelBitcode(false));
  ElfMutationLayout Layout = inspectElfMutationLayout(Payload);
  require(Layout.ExecutableProgramHeaders.size() == 1,
          "fixture does not have one executable PT_LOAD");
  require(!Layout.NonLoadProgramHeaders.empty(),
          "fixture lacks a non-PT_LOAD header for alias repro");
  require(Layout.ProgramHeaders[ELF::PT_NOTE].size() == 1 &&
              Layout.ProgramHeaders[ELF::PT_DYNAMIC].size() == 1,
          "fixture lacks exact PT_NOTE/PT_DYNAMIC views");
  require(Layout.SectionHeaders.contains(".text") &&
              Layout.SectionHeaders.contains(".note") &&
              Layout.SectionHeaders.contains(".symtab") &&
              Layout.SectionIndices.contains(".text"),
          "fixture lacks loader-view mutation sections");
  auto StaticAlpha =
      Layout.Symbols.find({ELF::SHT_SYMTAB, std::string("alpha")});
  auto DynamicAlpha =
      Layout.Symbols.find({ELF::SHT_DYNSYM, std::string("alpha")});
  require(StaticAlpha != Layout.Symbols.end() &&
              DynamicAlpha != Layout.Symbols.end(),
          "fixture lacks alpha in both symbol tables");

  const size_t Executable = Layout.ExecutableProgramHeaders.front();
  const size_t NoteProgramHeader =
      Layout.ProgramHeaders.at(ELF::PT_NOTE).front();
  const size_t DynamicProgramHeader =
      Layout.ProgramHeaders.at(ELF::PT_DYNAMIC).front();
  auto SpareProgramHeader =
      llvm::find_if(Layout.NonLoadProgramHeaders, [&](size_t Offset) {
        return Offset != NoteProgramHeader && Offset != DynamicProgramHeader;
      });
  require(SpareProgramHeader != Layout.NonLoadProgramHeaders.end(),
          "fixture lacks a spare program header for duplicate-view repro");
  {
    std::vector<uint8_t> Mutated = Payload;
    support::endian::write64le(Mutated.data() + Executable + 32,
                               Mutated.size() + 1);
    requireRejectedWith(std::move(Mutated),
                        "program-header file range is outside payload");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    uint32_t Flags = support::endian::read32le(Mutated.data() + Executable + 4);
    support::endian::write32le(Mutated.data() + Executable + 4,
                               Flags | ELF::PF_W);
    requireRejectedWith(std::move(Mutated),
                        "PT_LOAD permissions are outside bounded profile");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    std::array<uint8_t, sizeof(ELF64LE::Phdr)> Copy{};
    llvm::copy(ArrayRef<uint8_t>(Mutated).slice(Executable, Copy.size()),
               Copy.begin());
    llvm::copy(Copy, Mutated.begin() + Layout.NonLoadProgramHeaders.front());
    requireRejectedWith(std::move(Mutated), "PT_LOAD virtual mappings overlap");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    support::endian::write32le(Mutated.data() + NoteProgramHeader,
                               ELF::PT_NULL);
    requireRejectedWith(
        std::move(Mutated),
        "bounded loader profile requires one PT_NOTE and one PT_DYNAMIC");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    llvm::copy(ArrayRef<uint8_t>(Mutated).slice(NoteProgramHeader,
                                                sizeof(ELF64LE::Phdr)),
               Mutated.begin() + *SpareProgramHeader);
    requireRejectedWith(std::move(Mutated), "multiple PT_NOTE program headers");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    uint64_t Offset =
        support::endian::read64le(Mutated.data() + NoteProgramHeader + 8);
    support::endian::write64le(Mutated.data() + NoteProgramHeader + 8,
                               Offset + 4);
    requireRejectedWith(std::move(Mutated),
                        "PT_NOTE and PT_LOAD file views disagree");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    support::endian::write32le(Mutated.data() + DynamicProgramHeader,
                               ELF::PT_NULL);
    requireRejectedWith(
        std::move(Mutated),
        "bounded loader profile requires one PT_NOTE and one PT_DYNAMIC");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    llvm::copy(ArrayRef<uint8_t>(Mutated).slice(DynamicProgramHeader,
                                                sizeof(ELF64LE::Phdr)),
               Mutated.begin() + *SpareProgramHeader);
    requireRejectedWith(std::move(Mutated),
                        "multiple PT_DYNAMIC program headers");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    uint64_t Size =
        support::endian::read64le(Mutated.data() + DynamicProgramHeader + 32);
    require(Size > sizeof(ELF64LE::Dyn),
            "fixture PT_DYNAMIC is too small for alternate-view repro");
    support::endian::write64le(Mutated.data() + DynamicProgramHeader + 32,
                               Size - sizeof(ELF64LE::Dyn));
    support::endian::write64le(Mutated.data() + DynamicProgramHeader + 40,
                               Size - sizeof(ELF64LE::Dyn));
    requireRejectedWith(std::move(Mutated),
                        ".dynamic section and program-header views disagree");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Text = Layout.SectionHeaders.at(".text");
    uint64_t Offset = support::endian::read64le(Mutated.data() + Text + 24);
    support::endian::write64le(Mutated.data() + Text + 24, Offset + 4);
    requireRejectedWith(std::move(Mutated),
                        "section and PT_LOAD file views disagree");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Note = Layout.SectionHeaders.at(".note");
    uint64_t Address = support::endian::read64le(Mutated.data() + Note + 16);
    support::endian::write64le(Mutated.data() + Note + 16, Address + 4);
    requireRejectedWith(std::move(Mutated),
                        "section and PT_LOAD file views disagree");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    uint64_t Value =
        support::endian::read64le(Mutated.data() + DynamicAlpha->second + 8);
    support::endian::write64le(Mutated.data() + DynamicAlpha->second + 8,
                               Value + 4);
    requireRejectedWith(std::move(Mutated), ".symtab/.dynsym export mismatch");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    support::endian::write64le(Mutated.data() + StaticAlpha->second + 16,
                               std::numeric_limits<uint64_t>::max());
    support::endian::write64le(Mutated.data() + DynamicAlpha->second + 16,
                               std::numeric_limits<uint64_t>::max());
    requireRejectedWith(std::move(Mutated), "symbol range is outside section");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Symtab = Layout.SectionHeaders.at(".symtab");
    support::endian::write32le(Mutated.data() + Symtab + 4, ELF::SHT_RELA);
    support::endian::write32le(Mutated.data() + Symtab + 44,
                               Layout.SectionIndices.at(".text"));
    requireRejectedWith(std::move(Mutated),
                        "unsupported finalized-image relocations");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Entry = Layout.DynamicEntries.at(ELF::DT_STRTAB);
    uint64_t Address = support::endian::read64le(Mutated.data() + Entry + 8);
    support::endian::write64le(Mutated.data() + Entry + 8, Address + 1);
    requireRejectedWith(std::move(Mutated),
                        "dynamic declarations disagree with loadable sections");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Hash = Layout.SectionHeaders.at(".hash");
    uint64_t Offset = support::endian::read64le(Mutated.data() + Hash + 24);
    require(Offset + 8 <= Mutated.size(),
            "fixture .hash bytes are outside payload");
    uint32_t ChainCount =
        support::endian::read32le(Mutated.data() + Offset + 4);
    support::endian::write32le(Mutated.data() + Offset + 4, ChainCount + 1);
    requireRejectedWith(std::move(Mutated),
                        ".hash does not exactly describe .dynsym");
  }
  {
    std::vector<uint8_t> Mutated = Payload;
    size_t Symtab = Layout.SectionHeaders.at(".symtab");
    uint64_t Offset = support::endian::read64le(Mutated.data() + Symtab + 24);
    constexpr uint64_t OversizedSymbolBytes = 4097 * sizeof(ELF64LE::Sym);
    require(Offset <= std::numeric_limits<size_t>::max() - OversizedSymbolBytes,
            "symbol fanout mutation overflows host size");
    Mutated.resize(static_cast<size_t>(Offset + OversizedSymbolBytes));
    support::endian::write64le(Mutated.data() + Symtab + 32,
                               OversizedSymbolBytes);
    support::endian::write64le(Mutated.data() + Symtab + 56,
                               sizeof(ELF64LE::Sym));
    requireRejectedWith(std::move(Mutated),
                        ".symtab symbol count is outside bounded profile");
  }
}

void cfgReviewerReproductionsFailClosed() {
  {
    std::vector<uint8_t> Payload = finalize(makeKernelBitcode(false));
    auto Sites = decodeSymbolSites(Payload, "alpha");
    auto End = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return StringRef(Site.Name).starts_with("S_ENDPGM");
    });
    require(End != Sites.end() && End->Size == 4,
            "fallthrough repro lacks four-byte S_ENDPGM");
    support::endian::write32le(Payload.data() + End->FileOffset, 0xbf800000);
    auto Changed = decodeSymbolSites(Payload, "alpha");
    require(llvm::any_of(Changed,
                         [](const DecodedSite &Site) {
                           return StringRef(Site.Name).starts_with("S_NOP");
                         }),
            "fallthrough repro did not encode S_NOP");
    std::string Diagnostic = rejectedDiagnostic(std::move(Payload));
    require(StringRef(Diagnostic).contains("fallthrough exits symbol"),
            "fallthrough repro did not reach CFG rejection");
  }

  {
    std::vector<uint8_t> Payload = finalize(makeKernelBitcode(true));
    auto Sites = decodeSymbolSites(Payload, "alpha");
    auto GetPc = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_GETPC_B64_vi";
    });
    auto AddHigh = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_ADDC_U32_vi";
    });
    require(GetPc != Sites.end() && AddHigh != Sites.end() &&
                GetPc->Size == 4 && AddHigh->Address > GetPc->Address + 4,
            "skipped-definition repro lacks call materialization");
    uint64_t Distance = AddHigh->Address - (GetPc->Address + 4);
    require(Distance % 4 == 0 && Distance / 4 <= 0x7fff,
            "skipped-definition branch is not encodable");
    uint32_t Branch = 0xbf840000 | static_cast<uint16_t>(Distance / 4);
    support::endian::write32le(Payload.data() + GetPc->FileOffset, Branch);
    auto Changed = decodeSymbolSites(Payload, "alpha");
    require(llvm::any_of(Changed,
                         [](const DecodedSite &Site) {
                           return StringRef(Site.Name).starts_with("S_CBRANCH");
                         }),
            "skipped-definition repro did not encode S_BRANCH");
    std::string Diagnostic = rejectedDiagnostic(std::move(Payload));
    require(StringRef(Diagnostic).contains("definitions are ambiguous") ||
                StringRef(Diagnostic).contains("provenance is not exact"),
            "skipped-definition repro did not reach dataflow rejection: " +
                Diagnostic);
  }

  {
    std::vector<uint8_t> Payload = finalize(makeKernelBitcode(true));
    auto Sites = decodeSymbolSites(Payload, "alpha_helper");
    auto Return = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_SETPC_B64_vi";
    });
    require(Return != Sites.end() && Return != Sites.begin() &&
                std::prev(Return)->Size == 4,
            "return-provenance repro lacks replaceable predecessor");
    support::endian::write32le(Payload.data() + std::prev(Return)->FileOffset,
                               0xbe9e0180);
    auto Changed = decodeSymbolSites(Payload, "alpha_helper");
    require(llvm::any_of(Changed,
                         [](const DecodedSite &Site) {
                           return StringRef(Site.Name).starts_with("S_MOV_B64");
                         }),
            "return-provenance repro did not encode S_MOV_B64");
    std::string Diagnostic = rejectedDiagnostic(std::move(Payload));
    require(StringRef(Diagnostic).contains("return pair was modified"),
            "return-provenance repro did not reach dataflow rejection");
  }

  {
    std::vector<uint8_t> Payload = finalize(makeKernelBitcode(true));
    auto Sites = decodeSymbolSites(Payload, "alpha");
    auto Call = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_SWAPPC_B64_vi";
    });
    require(Call != Sites.end() && Call->Size == 4 &&
                Call->RegisterOperands.size() == 2 &&
                Call->RegisterOperands[0] == "SGPR30_SGPR31",
            "alternate-destination repro lacks ABI S_SWAPPC");
    const uint32_t Original =
        support::endian::read32le(Payload.data() + Call->FileOffset);
    bool ChangedDestination = false;
    for (unsigned Bit = 0; Bit != 32 && !ChangedDestination; ++Bit) {
      std::vector<uint8_t> Candidate = Payload;
      support::endian::write32le(Candidate.data() + Call->FileOffset,
                                 Original ^ (uint32_t{1} << Bit));
      auto Changed = decodeSymbolSites(Candidate, "alpha", false);
      auto ChangedCall = llvm::find_if(Changed, [&](const DecodedSite &Site) {
        return Site.Address == Call->Address &&
               Site.Name == "S_SWAPPC_B64_vi" && Site.Size == Call->Size;
      });
      if (ChangedCall == Changed.end() ||
          ChangedCall->RegisterOperands.size() != 2 ||
          ChangedCall->RegisterOperands[0] == "SGPR30_SGPR31" ||
          ChangedCall->RegisterOperands[1] != Call->RegisterOperands[1])
        continue;
      requireRejectedWith(
          std::move(Candidate),
          "call destination is not ABI return pair SGPR30_SGPR31");
      ChangedDestination = true;
    }
    require(ChangedDestination,
            "could not encode alternate S_SWAPPC destination pair");
  }
}

void directCallEdgesAreResolvedExactly() {
  auto Payload = finalize(makeKernelBitcode(true));
  auto RequestValue = directRequest(std::move(Payload));
  auto Result = analyzeGfx942PhysicalMachineEffects(RequestValue);
  if (!Result)
    fail(takeError(Result.takeError()));
  auto Alpha = llvm::find_if(Result->Functions, [](const auto &Function) {
    return Function.Symbol == "alpha";
  });
  require(Alpha != Result->Functions.end(),
          "alpha function evidence is absent");
  require(Alpha->DirectCallees == std::vector<std::string>{"alpha_helper"},
          "direct call edge was not resolved exactly");
  require(llvm::any_of(Result->Functions,
                       [](const auto &Function) {
                         return Function.Symbol == "alpha_helper";
                       }),
          "direct-call closure omitted helper");
}

void everyCallEncodingUsesTheAbiReturnPair() {
  {
    auto Payload = finalize(makeKernelBitcode(true));
    patchSwapCallToImmediate(Payload, "alpha", "alpha_helper", 30);
    auto Result =
        analyzeGfx942PhysicalMachineEffects(directRequest(std::move(Payload)));
    if (!Result)
      fail(takeError(Result.takeError()));
    auto Alpha = llvm::find_if(Result->Functions, [](const auto &Function) {
      return Function.Symbol == "alpha";
    });
    require(Alpha != Result->Functions.end() &&
                Alpha->DirectCallees ==
                    std::vector<std::string>{"alpha_helper"},
            "immediate S_CALL edge was not resolved exactly");
  }

  {
    auto Payload = finalize(makeKernelBitcode(true));
    patchSwapCallToImmediate(Payload, "alpha", "alpha_helper", 28);
    requireRejectedWith(
        std::move(Payload),
        "call destination is not ABI return pair SGPR30_SGPR31");
  }

  {
    // Nested calls currently require scratch spills outside this bounded
    // static profile. Keep the compiler-emitted valid ABI call visible and
    // make that unsupported boundary deterministic.
    auto Payload = finalize(makeKernelBitcode(true, 600, true));
    auto Sites = decodeSymbolSites(Payload, "alpha_helper");
    auto NestedCall = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_SWAPPC_B64_vi";
    });
    require(NestedCall != Sites.end() &&
                NestedCall->RegisterOperands.size() == 2 &&
                NestedCall->RegisterOperands[0] == "SGPR30_SGPR31",
            "nested compiler call does not use the exact ABI return pair");
    requireRejectedWith(std::move(Payload),
                        "unsupported instruction S_XOR_SAVEEXEC_B64_vi");
  }

  {
    // Remove only instructions before the nested call that are already
    // outside the profile, so the malformed immediate call itself is the
    // first rejection.
    auto Payload = finalize(makeKernelBitcode(true, 600, true));
    patchSwapCallToImmediate(Payload, "alpha_helper", "alpha_nested_helper",
                             28);
    auto Sites = decodeSymbolSites(Payload, "alpha_helper");
    auto Call = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return Site.Name == "S_CALL_B64_vi";
    });
    require(Call != Sites.end(), "nested immediate call is absent");
    for (const DecodedSite &Site : Sites) {
      if (Site.Address >= Call->Address)
        break;
      if (Site.Name != "S_XOR_SAVEEXEC_B64_vi" &&
          Site.Name != "V_WRITELANE_B32_vi" &&
          !StringRef(Site.Name).starts_with("SCRATCH_"))
        continue;
      require(Site.Size % 4 == 0,
              "unsupported nested prefix is not word sized");
      for (size_t Offset = 0; Offset < Site.Size; Offset += 4)
        support::endian::write32le(Payload.data() + Site.FileOffset + Offset,
                                   0xbf800000);
    }
    requireRejectedWith(
        std::move(Payload),
        "call destination is not ABI return pair SGPR30_SGPR31");
  }
}

void scalarLoadWidthsUseExactMcEncodings() {
  struct WidthCase {
    uint8_t Opcode;
    StringLiteral Name;
    uint16_t Bytes;
  };
  constexpr std::array<WidthCase, 5> Cases = {
      WidthCase{0x00, "S_LOAD_DWORD", 4},
      WidthCase{0x01, "S_LOAD_DWORDX2", 8},
      WidthCase{0x02, "S_LOAD_DWORDX4", 16},
      WidthCase{0x03, "S_LOAD_DWORDX8", 32},
      WidthCase{0x04, "S_LOAD_DWORDX16", 64},
  };

  for (const WidthCase &Case : Cases) {
    auto Payload = finalize(makeKernelBitcode(false));
    auto Sites = decodeSymbolSites(Payload, "alpha");
    auto ScalarLoad = llvm::find_if(Sites, [](const DecodedSite &Site) {
      return StringRef(Site.Name).starts_with("S_LOAD_DWORD") && Site.Size == 8;
    });
    require(ScalarLoad != Sites.end(),
            "scalar-width fixture has no eight-byte S_LOAD");

    uint32_t Low =
        support::endian::read32le(Payload.data() + ScalarLoad->FileOffset);
    constexpr uint32_t OpcodeMask = 0xffu << 18;
    constexpr uint32_t DestinationMask = 0x7fu << 6;
    Low &= ~(OpcodeMask | DestinationMask);
    Low |= static_cast<uint32_t>(Case.Opcode) << 18;
    support::endian::write32le(Payload.data() + ScalarLoad->FileOffset, Low);

    auto Changed = decodeSymbolSites(Payload, "alpha");
    auto ChangedLoad = llvm::find_if(Changed, [&](const DecodedSite &Site) {
      return Site.Address == ScalarLoad->Address &&
             StringRef(Site.Name).starts_with(Case.Name);
    });
    require(ChangedLoad != Changed.end(),
            "scalar-load opcode mutation did not decode exactly");

    auto Result =
        analyzeGfx942PhysicalMachineEffects(directRequest(std::move(Payload)));
    if (!Result)
      fail(takeError(Result.takeError()));
    auto Effect = llvm::find_if(Result->Effects, [&](const auto &Candidate) {
      return Candidate.EntrySymbol == "alpha" &&
             Candidate.FunctionSymbol == "alpha" &&
             Candidate.InstructionOffset == ScalarLoad->Address &&
             Candidate.Kind == PhysicalMachineEffectKind::GlobalRead;
    });
    require(Effect != Result->Effects.end() && Effect->ByteWidth == Case.Bytes,
            "scalar-load effect width does not match MC opcode");
  }
}

} // namespace

int main() {
  identityProbeBindsFreshChallenge();
  exactProductionProfileRejectsAlternatives();
  physicalAnalysisDerivesDeterministicClosedEffects();
  decoderBindsBytesSymbolsAndIdentities();
  targetDescriptorAndEffectExpansionFailClosed();
  loaderViewMutationsFailClosed();
  cfgReviewerReproductionsFailClosed();
  directCallEdgesAreResolvedExactly();
  everyCallEncodingUsesTheAbiReturnPair();
  scalarLoadWidthsUseExactMcEncodings();
  exactScalarGemmArtifactIsAccepted();
  return 0;
}
