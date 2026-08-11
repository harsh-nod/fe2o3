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
#include <limits>
#include <map>
#include <memory>
#include <optional>
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
                                       uint32_t CodeObjectFlag = 600) {
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
  if (WithHelper) {
    Helper = Function::Create(HelperType, GlobalValue::InternalLinkage,
                              "alpha_helper", ModuleValue);
    Helper->addFnAttr(Attribute::NoInline);
    Helper->addFnAttr("target-cpu", "gfx942");
    Helper->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
    BasicBlock *Block = BasicBlock::Create(Context, "entry", Helper);
    IRBuilder<> Builder(Block);
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
};

struct ElfMutationLayout {
  std::vector<size_t> ExecutableProgramHeaders;
  std::vector<size_t> NonLoadProgramHeaders;
  std::map<std::string, size_t> SectionHeaders;
  std::map<std::string, uint32_t> SectionIndices;
  std::map<std::pair<uint32_t, std::string>, size_t> Symbols;
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
                                           StringRef Name) {
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
    require(Status == MCDisassembler::Success && InstructionSize != 0 &&
                InstructionSize <= Bytes.size() - Offset,
            "reviewer repro cannot decode symbol");
    Result.push_back({BaseOffset + static_cast<size_t>(Offset),
                      Address + Offset, InstructionSize,
                      Instructions->getName(Instruction.getOpcode()).str()});
    Offset += InstructionSize;
  }
  return Result;
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
  return 0;
}
