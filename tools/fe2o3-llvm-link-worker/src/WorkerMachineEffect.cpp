#include "WorkerMachineEffect.h"

#include "llvm/ADT/STLExtras.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Config/llvm-config.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrAnalysis.h"
#include "llvm/MC/MCInstrDesc.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCTargetOptions.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/MemoryBufferRef.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"

#include <algorithm>
#include <cstring>
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
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace llvm;
using namespace llvm::object;

namespace fe2o3::worker {
namespace {

constexpr StringLiteral RequestDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST/V1\0";
constexpr StringLiteral EvidenceDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE/V1\0";
constexpr StringLiteral IdentityChallengeDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE/V1\0";
constexpr StringLiteral IdentityResponseDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-RESPONSE/V1\0";
constexpr StringLiteral RequestIdentityDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY/V1\0";
constexpr StringLiteral AnalyzerIdentityDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-ANALYZER/V1\0";
constexpr StringLiteral ToolchainIdentityDomain =
    "FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-TOOLCHAIN/V1\0";
constexpr StringLiteral PhysicalProfileMetadataTarget =
    "amdgcn-amd-amdhsa--gfx942:xnack-";
constexpr uint32_t PhysicalProfileElfFlags =
    ELF::EF_AMDGPU_MACH_AMDGCN_GFX942 | ELF::EF_AMDGPU_FEATURE_XNACK_OFF_V4 |
    ELF::EF_AMDGPU_FEATURE_SRAMECC_ANY_V4;
constexpr uint16_t SchemaVersion = 1;
constexpr size_t MaxEntries = 2;
constexpr size_t MaxEdges = 256;
constexpr size_t MaxSymbolBytes = 256;

Error analysisError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(),
                           Twine("gfx942 physical machine-effect: ") + Message);
}

std::array<uint8_t, 32> domainHash(StringRef Domain, ArrayRef<uint8_t> Bytes) {
  SHA256 Hash;
  Hash.update(arrayRefFromStringRef(Domain));
  Hash.update(Bytes);
  return Hash.final();
}

std::array<uint8_t, 32> domainHash(StringRef Domain, StringRef Text) {
  return domainHash(Domain, arrayRefFromStringRef(Text));
}

class Reader {
public:
  explicit Reader(ArrayRef<uint8_t> Bytes) : Bytes(Bytes) {}

  Expected<ArrayRef<uint8_t>> take(size_t Count) {
    if (Count > Bytes.size() - Position)
      return analysisError("truncated request");
    ArrayRef<uint8_t> Result = Bytes.slice(Position, Count);
    Position += Count;
    return Result;
  }

  Expected<uint16_t> u16() {
    auto Value = take(2);
    if (!Value)
      return Value.takeError();
    return support::endian::read16le(Value->data());
  }

  Expected<uint32_t> u32() {
    auto Value = take(4);
    if (!Value)
      return Value.takeError();
    return support::endian::read32le(Value->data());
  }

  Expected<uint64_t> u64() {
    auto Value = take(8);
    if (!Value)
      return Value.takeError();
    return support::endian::read64le(Value->data());
  }

  Expected<std::array<uint8_t, 32>> digest() {
    auto Value = take(32);
    if (!Value)
      return Value.takeError();
    std::array<uint8_t, 32> Result{};
    llvm::copy(*Value, Result.begin());
    return Result;
  }

  Expected<std::string> symbol() {
    auto Length = u16();
    if (!Length)
      return Length.takeError();
    if (*Length == 0 || *Length > MaxSymbolBytes)
      return analysisError("invalid symbol length");
    auto Value = take(*Length);
    if (!Value)
      return Value.takeError();
    std::string Result(reinterpret_cast<const char *>(Value->data()),
                       Value->size());
    if (!validSymbol(Result))
      return analysisError("invalid symbol text");
    return Result;
  }

  Error finish() const {
    if (Position != Bytes.size())
      return analysisError("request has trailing bytes");
    return Error::success();
  }

  static bool validSymbol(StringRef Symbol) {
    if (Symbol.empty() || Symbol.size() > MaxSymbolBytes)
      return false;
    if (!isAlpha(Symbol.front()) && Symbol.front() != '_' &&
        Symbol.front() != '.' && Symbol.front() != '$')
      return false;
    return llvm::all_of(Symbol, [](char Byte) {
      return isAlnum(Byte) || Byte == '_' || Byte == '.' || Byte == '$';
    });
  }

private:
  ArrayRef<uint8_t> Bytes;
  size_t Position = 0;
};

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

Error appendText(std::vector<uint8_t> &Output, StringRef Value) {
  if (!Reader::validSymbol(Value))
    return analysisError("cannot encode invalid symbol");
  appendU16(Output, static_cast<uint16_t>(Value.size()));
  Output.insert(Output.end(), Value.bytes_begin(), Value.bytes_end());
  return Error::success();
}

struct MetadataKernel {
  std::string Name;
  std::string Descriptor;
  uint64_t KernargSize = 0;
  uint64_t GroupSize = 0;
  uint64_t PrivateSize = 0;
};

Expected<msgpack::DocNode *> requiredField(msgpack::MapDocNode &Map,
                                           StringRef Name) {
  auto Field = Map.find(Name);
  if (Field == Map.end())
    return analysisError(Twine("metadata missing ") + Name);
  return &Field->second;
}

Expected<StringRef> metadataString(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredField(Map, Name);
  if (!Field)
    return Field.takeError();
  if (!(**Field).isString())
    return analysisError(Twine("metadata field is not text: ") + Name);
  return (**Field).getString();
}

Expected<uint64_t> metadataUnsigned(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredField(Map, Name);
  if (!Field)
    return Field.takeError();
  if ((**Field).getKind() == msgpack::Type::UInt)
    return (**Field).getUInt();
  if ((**Field).getKind() == msgpack::Type::Int && (**Field).getInt() >= 0)
    return static_cast<uint64_t>((**Field).getInt());
  return analysisError(Twine("metadata field is not unsigned: ") + Name);
}

Expected<std::vector<MetadataKernel>>
readMetadata(const ELFObjectFile<ELF64LE> &Object) {
  const ELFFile<ELF64LE> &File = Object.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();

  std::optional<std::string> Target;
  std::vector<MetadataKernel> Result;
  std::set<std::string> Names;
  size_t MetadataNoteCount = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    if (Section.sh_type != ELF::SHT_NOTE)
      continue;
    Error NoteError = Error::success();
    for (const ELF64LE::Note Note : File.notes(Section, NoteError)) {
      if (Note.getName() != "AMDGPU" ||
          Note.getType() != ELF::NT_AMDGPU_METADATA)
        continue;
      if (++MetadataNoteCount != 1)
        return analysisError("multiple AMDGPU metadata notes");
      if (Section.sh_addralign != 4)
        return analysisError("metadata note alignment is not four");
      StringRef Blob = Note.getDescAsStringRef(Section.sh_addralign);
      if (Blob.empty())
        return analysisError("metadata note is empty");
      msgpack::Document Document;
      if (!Document.readFromBlob(Blob, false))
        return analysisError("metadata note is malformed");
      AMDGPU::HSAMD::V3::MetadataVerifier Verifier(true);
      if (!Verifier.verify(Document.getRoot()))
        return analysisError("metadata schema is invalid");
      auto &Root = Document.getRoot().getMap();
      auto CurrentTarget = metadataString(Root, "amdhsa.target");
      if (!CurrentTarget)
        return CurrentTarget.takeError();
      if (!matchesPhysicalMachineEffectMetadataTargetV1(*CurrentTarget))
        return analysisError(
            "metadata target is not exact gfx942:xnack- profile");
      if (Target && *Target != *CurrentTarget)
        return analysisError("metadata target records disagree");
      Target = CurrentTarget->str();

      auto Kernels = requiredField(Root, "amdhsa.kernels");
      if (!Kernels)
        return Kernels.takeError();
      if (!(**Kernels).isArray())
        return analysisError("metadata kernels are not an array");
      for (msgpack::DocNode &Node : (**Kernels).getArray()) {
        if (!Node.isMap())
          return analysisError("metadata kernel is not a map");
        auto &Map = Node.getMap();
        auto Name = metadataString(Map, ".name");
        if (!Name)
          return Name.takeError();
        auto Descriptor = metadataString(Map, ".symbol");
        if (!Descriptor)
          return Descriptor.takeError();
        auto Kernarg = metadataUnsigned(Map, ".kernarg_segment_size");
        if (!Kernarg)
          return Kernarg.takeError();
        auto Group = metadataUnsigned(Map, ".group_segment_fixed_size");
        if (!Group)
          return Group.takeError();
        auto Private = metadataUnsigned(Map, ".private_segment_fixed_size");
        if (!Private)
          return Private.takeError();
        if (!Reader::validSymbol(*Name) ||
            *Descriptor != (Twine(*Name) + ".kd").str())
          return analysisError("metadata kernel descriptor mismatch");
        if (!Names.insert(Name->str()).second)
          return analysisError("metadata repeats a kernel");
        Result.push_back(
            {Name->str(), Descriptor->str(), *Kernarg, *Group, *Private});
      }
    }
    if (NoteError)
      return NoteError;
  }
  if (!Target || Result.empty())
    return analysisError("AMDGPU metadata is absent");
  llvm::sort(Result,
             [](const MetadataKernel &Left, const MetadataKernel &Right) {
               return Left.Name < Right.Name;
             });
  return Result;
}

struct SymbolRecord {
  std::string Name;
  uint64_t Address = 0;
  uint64_t Size = 0;
  SectionRef Section;
  SymbolRef::Type Type = SymbolRef::ST_Unknown;
};

Expected<std::vector<SymbolRecord>>
readSymbols(const ELFObjectFile<ELF64LE> &Object) {
  std::vector<SymbolRecord> Result;
  for (SymbolRef Symbol : Object.symbols()) {
    auto Name = Symbol.getName();
    if (!Name)
      return Name.takeError();
    if (Name->empty())
      continue;
    auto Address = Symbol.getAddress();
    if (!Address)
      return Address.takeError();
    auto Type = Symbol.getType();
    if (!Type)
      return Type.takeError();
    auto Section = Symbol.getSection();
    if (!Section)
      return Section.takeError();
    if (*Section == Object.section_end())
      continue;
    uint64_t Size = ELFSymbolRef(Symbol).getSize();
    Result.push_back({Name->str(), *Address, Size, **Section, *Type});
  }
  llvm::sort(Result, [](const SymbolRecord &Left, const SymbolRecord &Right) {
    return std::tie(Left.Name, Left.Address, Left.Size) <
           std::tie(Right.Name, Right.Address, Right.Size);
  });
  for (size_t I = 1; I < Result.size(); ++I)
    if (Result[I - 1].Name == Result[I].Name)
      return analysisError(Twine("duplicate symbol: ") + Result[I].Name);
  return Result;
}

Expected<ArrayRef<uint8_t>> symbolBytes(const SymbolRecord &Symbol) {
  auto Contents = Symbol.Section.getContents();
  if (!Contents)
    return Contents.takeError();
  uint64_t SectionAddress = Symbol.Section.getAddress();
  if (Symbol.Address < SectionAddress)
    return analysisError(Twine("symbol precedes section: ") + Symbol.Name);
  uint64_t Offset = Symbol.Address - SectionAddress;
  if (Offset > Contents->size() || Symbol.Size > Contents->size() - Offset)
    return analysisError(Twine("symbol range is outside section: ") +
                         Symbol.Name);
  const uint8_t *Begin =
      reinterpret_cast<const uint8_t *>(Contents->data() + Offset);
  return ArrayRef<uint8_t>(Begin, static_cast<size_t>(Symbol.Size));
}

const SymbolRecord *findSymbol(ArrayRef<SymbolRecord> Symbols, StringRef Name) {
  auto Iterator = llvm::lower_bound(
      Symbols, Name, [](const SymbolRecord &Symbol, StringRef Value) {
        return Symbol.Name < Value;
      });
  if (Iterator == Symbols.end() || Iterator->Name != Name)
    return nullptr;
  return &*Iterator;
}

Expected<PhysicalMachineEntryEvidence>
validateDescriptor(const MetadataKernel &Metadata, const SymbolRecord &Entry,
                   const SymbolRecord &Descriptor) {
  if (Entry.Type != SymbolRef::ST_Function || Entry.Size == 0 ||
      !Entry.Section.isText())
    return analysisError(Twine("entry is not a bounded text function: ") +
                         Entry.Name);
  if (Descriptor.Type != SymbolRef::ST_Data || Descriptor.Size != 64 ||
      !Descriptor.Section.isData())
    return analysisError(Twine("kernel descriptor has invalid shape: ") +
                         Descriptor.Name);
  auto Bytes = symbolBytes(Descriptor);
  if (!Bytes)
    return Bytes.takeError();
  uint32_t Group = support::endian::read32le(Bytes->data());
  uint32_t Private = support::endian::read32le(Bytes->data() + 4);
  uint32_t Kernarg = support::endian::read32le(Bytes->data() + 8);
  int64_t EntryOffset =
      static_cast<int64_t>(support::endian::read64le(Bytes->data() + 16));
  if (Group != Metadata.GroupSize || Private != Metadata.PrivateSize ||
      Kernarg != Metadata.KernargSize)
    return analysisError(Twine("kernel descriptor disagrees with metadata: ") +
                         Entry.Name);
  static constexpr std::pair<size_t, size_t> Reserved[] = {
      {12, 16}, {24, 44}, {60, 64}};
  for (auto [Begin, End] : Reserved)
    if (llvm::any_of(Bytes->slice(Begin, End - Begin),
                     [](uint8_t Byte) { return Byte != 0; }))
      return analysisError(Twine("kernel descriptor reserved bytes changed: ") +
                           Entry.Name);
  uint64_t ExpectedEntry = 0;
  if (EntryOffset >= 0) {
    if (Descriptor.Address > std::numeric_limits<uint64_t>::max() -
                                 static_cast<uint64_t>(EntryOffset))
      return analysisError("kernel descriptor entry address overflows");
    ExpectedEntry = Descriptor.Address + static_cast<uint64_t>(EntryOffset);
  } else {
    uint64_t Magnitude = static_cast<uint64_t>(-(EntryOffset + 1)) + 1;
    if (Descriptor.Address < Magnitude)
      return analysisError("kernel descriptor entry address underflows");
    ExpectedEntry = Descriptor.Address - Magnitude;
  }
  if (ExpectedEntry != Entry.Address)
    return analysisError(Twine("kernel descriptor points at another entry: ") +
                         Entry.Name);
  return PhysicalMachineEntryEvidence{Entry.Name, SHA256::hash(*Bytes),
                                      Entry.Address, Entry.Size};
}

struct DecodedInstruction {
  uint64_t Address = 0;
  uint64_t Size = 0;
  MCInst Inst;
  std::string Name;
};

struct LocalEffect {
  uint64_t Offset = 0;
  PhysicalMachineEffectKind Kind = PhysicalMachineEffectKind::GlobalAddress;
  uint16_t Width = 0;
};

struct AnalyzedFunction {
  PhysicalMachineFunctionEvidence Evidence;
  std::vector<LocalEffect> Effects;
};

struct McState {
  std::unique_ptr<MCRegisterInfo> Registers;
  std::unique_ptr<MCAsmInfo> AsmInfo;
  std::unique_ptr<MCSubtargetInfo> Subtarget;
  std::unique_ptr<MCInstrInfo> Instructions;
  std::unique_ptr<MCContext> Context;
  std::unique_ptr<MCDisassembler> Disassembler;
  std::unique_ptr<MCInstrAnalysis> Analysis;
};

Expected<McState> createMcState() {
  static bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUDisassembler();
    return true;
  }();
  (void)Initialized;

  Triple TripleValue("amdgcn-amd-amdhsa");
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  if (!TargetValue)
    return analysisError(Twine("AMDGPU target unavailable: ") + LookupError);
  McState Result;
  Result.Registers.reset(TargetValue->createMCRegInfo(TripleValue));
  Result.Instructions.reset(TargetValue->createMCInstrInfo());
  Result.Subtarget.reset(
      TargetValue->createMCSubtargetInfo(TripleValue, "gfx942", "-xnack"));
  if (!Result.Registers || !Result.Instructions || !Result.Subtarget)
    return analysisError("AMDGPU MC tables are unavailable");
  MCTargetOptions Options;
  Result.AsmInfo.reset(
      TargetValue->createMCAsmInfo(*Result.Registers, TripleValue, Options));
  if (!Result.AsmInfo)
    return analysisError("AMDGPU MC assembly info is unavailable");
  Result.Context = std::make_unique<MCContext>(
      TripleValue, Result.AsmInfo.get(), Result.Registers.get(),
      Result.Subtarget.get(), nullptr, &Options);
  Result.Disassembler.reset(
      TargetValue->createMCDisassembler(*Result.Subtarget, *Result.Context));
  Result.Analysis.reset(
      TargetValue->createMCInstrAnalysis(Result.Instructions.get()));
  if (!Result.Disassembler || !Result.Analysis)
    return analysisError("AMDGPU MC disassembler is unavailable");
  return Result;
}

Expected<std::vector<DecodedInstruction>>
decodeFunction(const SymbolRecord &Function, McState &Mc) {
  auto Bytes = symbolBytes(Function);
  if (!Bytes)
    return Bytes.takeError();
  std::vector<DecodedInstruction> Result;
  uint64_t Offset = 0;
  while (Offset < Bytes->size()) {
    if (llvm::all_of(Bytes->drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Inst;
    uint64_t Size = 0;
    auto Status =
        Mc.Disassembler->getInstruction(Inst, Size, Bytes->drop_front(Offset),
                                        Function.Address + Offset, nulls());
    if (Status != MCDisassembler::Success || Size == 0 ||
        Size > Bytes->size() - Offset)
      return analysisError(Twine("cannot decode instruction in ") +
                           Function.Name);
    StringRef Name = Mc.Instructions->getName(Inst.getOpcode());
    Result.push_back({Function.Address + Offset, Size, Inst, Name.str()});
    Offset += Size;
    if (Result.size() > MaxPhysicalMachineEffectEffects)
      return analysisError("instruction count exceeds bound");
  }
  if (Result.empty())
    return analysisError(Twine("function has no decoded instructions: ") +
                         Function.Name);
  return Result;
}

std::optional<uint16_t> memoryWidth(StringRef Name) {
  if (Name.contains("DWORDX4"))
    return 16;
  if (Name.contains("DWORDX3"))
    return 12;
  if (Name.contains("DWORDX2"))
    return 8;
  if (Name.contains("DWORD"))
    return 4;
  if (Name.contains("SHORT"))
    return 2;
  if (Name.contains("BYTE"))
    return 1;
  return std::nullopt;
}

bool forbiddenOpcodeFamily(StringRef Name) {
  return Name.contains("ATOMIC") || Name.starts_with("DS_") ||
         Name.starts_with("FLAT_") || Name.starts_with("BUFFER_") ||
         Name.starts_with("TBUFFER_") || Name.starts_with("IMAGE_") ||
         Name.starts_with("SCRATCH_");
}

bool acceptedAlphaZetaOpcode(StringRef Name) {
  return Name.starts_with("S_WAITCNT") || Name.starts_with("S_NOP") ||
         Name.starts_with("S_ENDPGM") || Name.starts_with("S_CLAUSE") ||
         Name == "S_DELAY_ALU" || Name.starts_with("V_MOV_") ||
         Name.starts_with("S_MOV_") || Name.starts_with("V_ADD_") ||
         Name.starts_with("V_MUL_") || Name.starts_with("V_FMA_") ||
         Name.starts_with("V_MAD_") || Name.starts_with("S_GETPC_") ||
         Name.starts_with("S_ADD_") || Name.starts_with("S_ADDC_") ||
         Name.starts_with("S_SUB_");
}

std::string instructionDescription(const DecodedInstruction &Instruction,
                                   const McState &Mc) {
  std::string Result;
  raw_string_ostream Stream(Result);
  Stream << Instruction.Name << '(';
  for (size_t I = 0; I < Instruction.Inst.size(); ++I) {
    if (I != 0)
      Stream << ',';
    const MCOperand &Operand = Instruction.Inst.getOperand(I);
    if (Operand.isReg())
      Stream << Mc.Registers->getName(Operand.getReg());
    else if (Operand.isImm())
      Stream << Operand.getImm();
    else
      Stream << "unsupported";
  }
  Stream << ')';
  Stream.flush();
  return Result;
}

bool definesRegister(const DecodedInstruction &Instruction, unsigned Register,
                     const McState &Mc) {
  const MCInstrDesc &Descriptor =
      Mc.Instructions->get(Instruction.Inst.getOpcode());
  size_t DefinitionCount =
      std::min<size_t>(Descriptor.getNumDefs(), Instruction.Inst.size());
  for (size_t I = 0; I < DefinitionCount; ++I) {
    const MCOperand &Operand = Instruction.Inst.getOperand(I);
    if (Operand.isReg() &&
        Mc.Registers->regsOverlap(Operand.getReg(), Register))
      return true;
  }
  return false;
}

std::optional<unsigned> registerNamed(StringRef Name, const McState &Mc) {
  for (unsigned Register = 1; Register < Mc.Registers->getNumRegs(); ++Register)
    if (Mc.Registers->getName(Register) == Name)
      return Register;
  return std::nullopt;
}

bool isRegisterOperand(const MCInst &Instruction, size_t Index,
                       unsigned Register) {
  return Index < Instruction.size() && Instruction.getOperand(Index).isReg() &&
         Instruction.getOperand(Index).getReg() == Register;
}

bool isImmediateOperand(const MCInst &Instruction, size_t Index) {
  return Index < Instruction.size() && Instruction.getOperand(Index).isImm();
}

bool isPhysicalReturn(const DecodedInstruction &Instruction,
                      const McState &Mc) {
  if (StringRef(Instruction.Name).starts_with("S_ENDPGM"))
    return true;
  return Instruction.Name == "S_SETPC_B64_vi" && Instruction.Inst.size() == 1 &&
         Instruction.Inst.getOperand(0).isReg() &&
         StringRef(Mc.Registers->getName(
             Instruction.Inst.getOperand(0).getReg())) == "SGPR30_SGPR31";
}

Expected<SmallVector<uint64_t, 2>>
directCallTargets(ArrayRef<DecodedInstruction> Instructions, size_t CallIndex,
                  McState &Mc) {
  const DecodedInstruction &Call = Instructions[CallIndex];
  uint64_t ImmediateTarget = 0;
  if (Mc.Analysis->evaluateBranch(Call.Inst, Call.Address, Call.Size,
                                  ImmediateTarget))
    return SmallVector<uint64_t, 2>{ImmediateTarget};

  if (Call.Name != "S_SWAPPC_B64_vi" || Call.Inst.size() != 2 ||
      !Call.Inst.getOperand(1).isReg())
    return analysisError(Twine("indirect or unknown call: ") +
                         instructionDescription(Call, Mc));

  unsigned PairRegister = Call.Inst.getOperand(1).getReg();
  StringRef PairName = Mc.Registers->getName(PairRegister);
  auto PairParts = PairName.split('_');
  if (!PairParts.first.starts_with("SGPR") ||
      !PairParts.second.starts_with("SGPR") || PairParts.second.contains('_'))
    return analysisError("direct-call target is not one SGPR pair");
  auto LowRegister = registerNamed(PairParts.first, Mc);
  auto HighRegister = registerNamed(PairParts.second, Mc);
  if (!LowRegister || !HighRegister || *LowRegister == *HighRegister)
    return analysisError("direct-call SGPR pair is malformed");

  const DecodedInstruction *GetPc = nullptr;
  const DecodedInstruction *AddLow = nullptr;
  const DecodedInstruction *AddHigh = nullptr;
  constexpr size_t MaxMaterializationInstructions = 16;
  size_t Begin = CallIndex > MaxMaterializationInstructions
                     ? CallIndex - MaxMaterializationInstructions
                     : 0;
  unsigned Step = 0;
  for (size_t I = CallIndex; I-- > Begin;) {
    const DecodedInstruction &Candidate = Instructions[I];
    bool DefinesTarget = definesRegister(Candidate, *LowRegister, Mc) ||
                         definesRegister(Candidate, *HighRegister, Mc);
    if (!DefinesTarget)
      continue;
    if (Step == 0 && Candidate.Name == "S_ADDC_U32_vi" &&
        isRegisterOperand(Candidate.Inst, 0, *HighRegister) &&
        isRegisterOperand(Candidate.Inst, 1, *HighRegister) &&
        isImmediateOperand(Candidate.Inst, 2)) {
      AddHigh = &Candidate;
    } else if (Step == 1 && Candidate.Name == "S_ADD_U32_vi" &&
               isRegisterOperand(Candidate.Inst, 0, *LowRegister) &&
               isRegisterOperand(Candidate.Inst, 1, *LowRegister) &&
               isImmediateOperand(Candidate.Inst, 2)) {
      AddLow = &Candidate;
    } else if (Step == 2 && Candidate.Name == "S_GETPC_B64_vi" &&
               Candidate.Inst.size() == 1 &&
               isRegisterOperand(Candidate.Inst, 0, PairRegister)) {
      GetPc = &Candidate;
    } else {
      return analysisError(
          "direct-call target register has unsupported definitions");
    }
    ++Step;
    if (Step == 3)
      break;
  }
  if (!GetPc || !AddLow || !AddHigh)
    return analysisError("direct-call materialization is incomplete");
  if (GetPc->Address + GetPc->Size != AddLow->Address ||
      AddLow->Address + AddLow->Size != AddHigh->Address)
    return analysisError("direct-call materialization is not contiguous");

  int64_t LowImmediate = AddLow->Inst.getOperand(2).getImm();
  int64_t HighImmediate = AddHigh->Inst.getOperand(2).getImm();
  if (LowImmediate < 0 || HighImmediate < 0 ||
      static_cast<uint64_t>(LowImmediate) >
          std::numeric_limits<uint32_t>::max() ||
      static_cast<uint64_t>(HighImmediate) >
          std::numeric_limits<uint32_t>::max())
    return analysisError("direct-call materialization immediate is not u32");

  auto Materialize = [&](uint64_t Pc) {
    uint64_t LowSum = static_cast<uint64_t>(static_cast<uint32_t>(Pc)) +
                      static_cast<uint32_t>(LowImmediate);
    uint64_t HighSum = static_cast<uint32_t>(Pc >> 32) +
                       static_cast<uint32_t>(HighImmediate) + (LowSum >> 32);
    return (static_cast<uint64_t>(static_cast<uint32_t>(HighSum)) << 32) |
           static_cast<uint32_t>(LowSum);
  };

  SmallVector<uint64_t, 2> Result;
  Result.push_back(Materialize(GetPc->Address));
  uint64_t NextPc = GetPc->Address + GetPc->Size;
  uint64_t NextTarget = Materialize(NextPc);
  if (NextTarget != Result.front())
    Result.push_back(NextTarget);
  return Result;
}

Expected<AnalyzedFunction> analyzeFunction(const SymbolRecord &Function,
                                           ArrayRef<SymbolRecord> Symbols,
                                           McState &Mc) {
  auto Decoded = decodeFunction(Function, Mc);
  if (!Decoded)
    return Decoded.takeError();
  std::set<uint64_t> Boundaries;
  for (const DecodedInstruction &Instruction : *Decoded)
    Boundaries.insert(Instruction.Address);

  AnalyzedFunction Result;
  Result.Evidence = {Function.Name, Function.Address, Function.Size, {}};
  for (const DecodedInstruction &Instruction : *Decoded) {
    const MCInstrDesc &Descriptor =
        Mc.Instructions->get(Instruction.Inst.getOpcode());
    StringRef Name = Instruction.Name;
    if (forbiddenOpcodeFamily(Name))
      return analysisError(Twine("unsupported opcode family ") + Name + " in " +
                           Function.Name);
    if (isPhysicalReturn(Instruction, Mc)) {
      Result.Effects.push_back(
          {Instruction.Address, PhysicalMachineEffectKind::Return, 0});
      continue;
    }
    if (Descriptor.isIndirectBranch())
      return analysisError(Twine("indirect branch in ") + Function.Name);
    if (Descriptor.isCall()) {
      size_t Index = static_cast<size_t>(&Instruction - Decoded->data());
      auto Targets = directCallTargets(*Decoded, Index, Mc);
      if (!Targets)
        return analysisError(Twine("cannot resolve call in ") + Function.Name +
                             ": " + toString(Targets.takeError()));
      std::vector<const SymbolRecord *> Matches;
      for (uint64_t Target : *Targets)
        for (const SymbolRecord &Candidate : Symbols)
          if (Candidate.Address == Target &&
              Candidate.Type == SymbolRef::ST_Function && Candidate.Size != 0 &&
              Candidate.Section.isText() &&
              llvm::find(Matches, &Candidate) == Matches.end())
            Matches.push_back(&Candidate);
      if (Matches.size() != 1) {
        std::string Detail;
        raw_string_ostream Stream(Detail);
        Stream << "call target is not one exact function in " << Function.Name
               << "; computed=";
        for (uint64_t Target : *Targets)
          Stream << ' ' << Target;
        Stream << "; functions=";
        for (const SymbolRecord &Candidate : Symbols)
          if (Candidate.Type == SymbolRef::ST_Function && Candidate.Size != 0 &&
              Candidate.Section.isText())
            Stream << ' ' << Candidate.Name << '@' << Candidate.Address;
        Stream.flush();
        return analysisError(Detail);
      }
      Result.Evidence.DirectCallees.push_back(Matches.front()->Name);
      continue;
    }
    if (Descriptor.isBranch()) {
      if (Descriptor.isBarrier())
        return analysisError(Twine("barrier branch in ") + Function.Name);
      uint64_t Target = 0;
      if (!Mc.Analysis->evaluateBranch(Instruction.Inst, Instruction.Address,
                                       Instruction.Size, Target))
        return analysisError(Twine("unknown branch target in ") +
                             Function.Name);
      if (Target <= Instruction.Address || !Boundaries.contains(Target))
        return analysisError(
            Twine("unsupported backward or external branch in ") +
            Function.Name);
      continue;
    }
    if (Descriptor.mayLoad() || Descriptor.mayStore()) {
      bool Read = Descriptor.mayLoad();
      bool Write = Descriptor.mayStore();
      bool SupportedRead =
          Name.starts_with("GLOBAL_LOAD_") || Name.starts_with("S_LOAD_");
      bool SupportedWrite = Name.starts_with("GLOBAL_STORE_");
      if ((Read && !SupportedRead) || (Write && !SupportedWrite) ||
          (Read && Write))
        return analysisError(Twine("unsupported memory instruction ") + Name +
                             " in " + Function.Name);
      auto Width = memoryWidth(Name);
      if (!Width)
        return analysisError(Twine("unknown memory width for ") + Name);
      Result.Effects.push_back(
          {Instruction.Address, PhysicalMachineEffectKind::GlobalAddress, 8});
      Result.Effects.push_back({Instruction.Address,
                                Read ? PhysicalMachineEffectKind::GlobalRead
                                     : PhysicalMachineEffectKind::GlobalWrite,
                                *Width});
      continue;
    }
    if (!acceptedAlphaZetaOpcode(Name))
      return analysisError(Twine("unsupported instruction ") + Name + " in " +
                           Function.Name);
  }
  llvm::sort(Result.Evidence.DirectCallees);
  if (std::adjacent_find(Result.Evidence.DirectCallees.begin(),
                         Result.Evidence.DirectCallees.end()) !=
      Result.Evidence.DirectCallees.end())
    return analysisError(Twine("duplicate direct call edge in ") +
                         Function.Name);
  if (llvm::none_of(Result.Effects, [](const LocalEffect &Effect) {
        return Effect.Kind == PhysicalMachineEffectKind::Return;
      }))
    return analysisError(Twine("function has no physical return: ") +
                         Function.Name);
  return Result;
}

std::set<std::string>
reachableFrom(StringRef Root,
              const std::map<std::string, AnalyzedFunction> &Functions) {
  std::set<std::string> Result;
  std::vector<std::string> Pending{Root.str()};
  while (!Pending.empty()) {
    std::string Current = std::move(Pending.back());
    Pending.pop_back();
    if (!Result.insert(Current).second)
      continue;
    const auto &Function = Functions.at(Current);
    Pending.insert(Pending.end(), Function.Evidence.DirectCallees.begin(),
                   Function.Evidence.DirectCallees.end());
  }
  return Result;
}

bool visitAcyclicCallGraph(
    StringRef Name, const std::map<std::string, AnalyzedFunction> &Functions,
    std::map<std::string, uint8_t> &States) {
  uint8_t &State = States[Name.str()];
  if (State == 1)
    return false;
  if (State == 2)
    return true;
  State = 1;
  for (const std::string &Callee :
       Functions.at(Name.str()).Evidence.DirectCallees)
    if (!visitAcyclicCallGraph(Callee, Functions, States))
      return false;
  State = 2;
  return true;
}

bool hasAcyclicCallGraph(
    const std::map<std::string, AnalyzedFunction> &Functions) {
  std::map<std::string, uint8_t> States;
  for (const auto &[Name, Function] : Functions) {
    (void)Function;
    if (!visitAcyclicCallGraph(Name, Functions, States))
      return false;
  }
  return true;
}

Error validateBudget(const PhysicalMachineEffectEntryRequest &Entry,
                     const std::set<std::string> &Closure,
                     const std::map<std::string, AnalyzedFunction> &Functions) {
  uint64_t Addresses = 0;
  uint64_t Reads = 0;
  uint64_t Writes = 0;
  uint64_t Returns = 0;
  uint64_t Calls = 0;
  for (const std::string &Name : Closure) {
    const AnalyzedFunction &Function = Functions.at(Name);
    Calls += Function.Evidence.DirectCallees.size();
    for (const LocalEffect &Effect : Function.Effects)
      switch (Effect.Kind) {
      case PhysicalMachineEffectKind::GlobalAddress:
        ++Addresses;
        break;
      case PhysicalMachineEffectKind::GlobalRead:
        ++Reads;
        break;
      case PhysicalMachineEffectKind::GlobalWrite:
        ++Writes;
        break;
      case PhysicalMachineEffectKind::Return:
        ++Returns;
        break;
      }
  }
  if (Addresses > Entry.Budget.GlobalAddresses ||
      Reads > Entry.Budget.GlobalReads || Writes > Entry.Budget.GlobalWrites ||
      Returns > Entry.Budget.Returns || Calls > Entry.Budget.DirectCalls)
    return analysisError(Twine("effect expansion exceeds request for ") +
                         Entry.Symbol);
  return Error::success();
}

} // namespace

bool matchesPhysicalMachineEffectMetadataTargetV1(StringRef Target) {
  return Target == PhysicalProfileMetadataTarget;
}

PhysicalMachineEffectIdentities physicalMachineEffectIdentities() {
  std::string Analyzer = (Twine(FE2O3_WORKER_BUILD_ID) +
                          "|target=gfx942:xnack-|cov=6|profile=alpha-zeta-v1")
                             .str();
  std::string Toolchain =
      (Twine(FE2O3_LLVM_BUILD_ID) + "|llvm=" + LLVM_VERSION_STRING).str();
  return {domainHash(AnalyzerIdentityDomain, Analyzer),
          domainHash(ToolchainIdentityDomain, Toolchain)};
}

Expected<std::vector<uint8_t>>
encodePhysicalMachineEffectIdentityResponse(ArrayRef<uint8_t> Request) {
  Reader Input(Request);
  auto Domain = Input.take(IdentityChallengeDomain.size());
  if (!Domain)
    return Domain.takeError();
  if (*Domain != arrayRefFromStringRef(IdentityChallengeDomain))
    return analysisError("identity challenge domain mismatch");
  auto Length = Input.u32();
  if (!Length)
    return Length.takeError();
  if (*Length != Request.size())
    return analysisError("identity challenge length mismatch");
  auto Version = Input.u16();
  if (!Version)
    return Version.takeError();
  if (*Version != SchemaVersion)
    return analysisError("identity challenge version mismatch");
  auto Challenge = Input.digest();
  if (!Challenge)
    return Challenge.takeError();
  if (*Challenge == std::array<uint8_t, 32>{})
    return analysisError("identity challenge is zero");
  if (Error ErrorValue = Input.finish())
    return ErrorValue;

  PhysicalMachineEffectIdentities Identities =
      physicalMachineEffectIdentities();
  std::vector<uint8_t> Output;
  Output.insert(Output.end(), IdentityResponseDomain.bytes_begin(),
                IdentityResponseDomain.bytes_end());
  appendU32(Output, 0);
  appendU16(Output, SchemaVersion);
  Output.insert(Output.end(), Challenge->begin(), Challenge->end());
  Output.insert(Output.end(), Identities.Analyzer.begin(),
                Identities.Analyzer.end());
  Output.insert(Output.end(), Identities.Toolchain.begin(),
                Identities.Toolchain.end());
  support::endian::write32le(Output.data() + IdentityResponseDomain.size(),
                             static_cast<uint32_t>(Output.size()));
  return Output;
}

Expected<PhysicalMachineEffectRequest>
decodePhysicalMachineEffectRequest(ArrayRef<uint8_t> Bytes) {
  if (Bytes.size() > MaxPhysicalMachineEffectPayloadBytes + 1024)
    return analysisError("request exceeds byte bound");
  Reader Input(Bytes);
  auto Domain = Input.take(RequestDomain.size());
  if (!Domain)
    return Domain.takeError();
  if (*Domain != arrayRefFromStringRef(RequestDomain))
    return analysisError("request domain mismatch");
  auto Length = Input.u32();
  if (!Length)
    return Length.takeError();
  if (*Length != Bytes.size())
    return analysisError("request length mismatch");
  auto Version = Input.u16();
  if (!Version)
    return Version.takeError();
  if (*Version != SchemaVersion)
    return analysisError("unsupported request version");

  PhysicalMachineEffectRequest Result;
  auto ExecutionChallenge = Input.digest();
  if (!ExecutionChallenge)
    return ExecutionChallenge.takeError();
  if (*ExecutionChallenge == std::array<uint8_t, 32>{})
    return analysisError("execution challenge is zero");
  Result.ExecutionChallenge = *ExecutionChallenge;
  auto Analyzer = Input.digest();
  if (!Analyzer)
    return Analyzer.takeError();
  Result.AnalyzerIdentity = *Analyzer;
  auto Toolchain = Input.digest();
  if (!Toolchain)
    return Toolchain.takeError();
  Result.ToolchainIdentity = *Toolchain;
  auto PayloadDigest = Input.digest();
  if (!PayloadDigest)
    return PayloadDigest.takeError();
  Result.PayloadDigest = *PayloadDigest;
  auto PayloadBytes = Input.u64();
  if (!PayloadBytes)
    return PayloadBytes.takeError();
  Result.PayloadBytes = *PayloadBytes;
  if (Result.PayloadBytes == 0 ||
      Result.PayloadBytes > MaxPhysicalMachineEffectPayloadBytes)
    return analysisError("payload length exceeds bound");

  auto EntryCount = Input.u16();
  if (!EntryCount)
    return EntryCount.takeError();
  if (*EntryCount == 0 || *EntryCount > MaxEntries)
    return analysisError("entry count exceeds alpha/zeta bound");
  for (uint16_t I = 0; I < *EntryCount; ++I) {
    auto Symbol = Input.symbol();
    if (!Symbol)
      return Symbol.takeError();
    if (*Symbol != "alpha" && *Symbol != "zeta")
      return analysisError("entry is outside alpha/zeta slice");
    PhysicalMachineEffectBudget Budget;
    auto Addresses = Input.u32();
    if (!Addresses)
      return Addresses.takeError();
    Budget.GlobalAddresses = *Addresses;
    auto Reads = Input.u32();
    if (!Reads)
      return Reads.takeError();
    Budget.GlobalReads = *Reads;
    auto Writes = Input.u32();
    if (!Writes)
      return Writes.takeError();
    Budget.GlobalWrites = *Writes;
    auto Returns = Input.u32();
    if (!Returns)
      return Returns.takeError();
    Budget.Returns = *Returns;
    auto Calls = Input.u32();
    if (!Calls)
      return Calls.takeError();
    Budget.DirectCalls = *Calls;
    Result.Entries.push_back({std::move(*Symbol), Budget});
  }
  for (size_t I = 1; I < Result.Entries.size(); ++I)
    if (Result.Entries[I - 1].Symbol >= Result.Entries[I].Symbol)
      return analysisError("entries are duplicate or noncanonical");

  auto Payload = Input.take(static_cast<size_t>(Result.PayloadBytes));
  if (!Payload)
    return Payload.takeError();
  if (Error ErrorValue = Input.finish())
    return ErrorValue;
  if (SHA256::hash(*Payload) != Result.PayloadDigest)
    return analysisError("payload digest mismatch");
  Result.Payload.assign(Payload->begin(), Payload->end());

  PhysicalMachineEffectIdentities Measured = physicalMachineEffectIdentities();
  if (Result.AnalyzerIdentity != Measured.Analyzer)
    return analysisError("analyzer identity mismatch");
  if (Result.ToolchainIdentity != Measured.Toolchain)
    return analysisError("toolchain identity mismatch");
  Result.RequestIdentity = domainHash(RequestIdentityDomain, Bytes);
  Result.RequestBytes = Bytes.size();
  return Result;
}

Expected<PhysicalMachineEffectEvidence> analyzeGfx942PhysicalMachineEffects(
    const PhysicalMachineEffectRequest &Request) {
  if (Request.Entries.empty() || Request.Entries.size() > MaxEntries)
    return analysisError("request entry count exceeds alpha/zeta bound");
  for (size_t I = 0; I < Request.Entries.size(); ++I) {
    StringRef Symbol = Request.Entries[I].Symbol;
    if ((Symbol != "alpha" && Symbol != "zeta") ||
        (I != 0 && Request.Entries[I - 1].Symbol >= Symbol))
      return analysisError("request entries are outside canonical alpha/zeta");
  }
  if (Request.Payload.empty() ||
      SHA256::hash(Request.Payload) != Request.PayloadDigest ||
      Request.Payload.size() != Request.PayloadBytes)
    return analysisError("request payload binding is invalid");
  PhysicalMachineEffectIdentities Measured = physicalMachineEffectIdentities();
  if (Request.AnalyzerIdentity != Measured.Analyzer ||
      Request.ToolchainIdentity != Measured.Toolchain)
    return analysisError("request measured identity is invalid");

  StringRef Data(reinterpret_cast<const char *>(Request.Payload.data()),
                 Request.Payload.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<exact-hsaco>"));
  if (!ObjectOrError)
    return analysisError(Twine("payload is not an object: ") +
                         toString(ObjectOrError.takeError()));
  auto *Base = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  auto *Object = dyn_cast<ELFObjectFile<ELF64LE>>(ObjectOrError->get());
  if (!Base || !Object || Base->getEMachine() != ELF::EM_AMDGPU ||
      Base->getEType() != ELF::ET_DYN || Base->getBytesInAddress() != 8 ||
      !Base->isLittleEndian())
    return analysisError("payload is not an AMDGPU ELF64LE shared object");
  if (Request.Payload.size() < ELF::EI_NIDENT ||
      Request.Payload[ELF::EI_OSABI] != ELF::ELFOSABI_AMDGPU_HSA ||
      Base->getEIdentABIVersion() != ELF::ELFABIVERSION_AMDGPU_HSA_V6)
    return analysisError("payload is not AMDHSA code object V6");
  if (Base->getPlatformFlags() != PhysicalProfileElfFlags)
    return analysisError("ELF target is not exact gfx942:xnack- profile");

  auto Metadata = readMetadata(*Object);
  if (!Metadata)
    return Metadata.takeError();
  if (Metadata->size() != Request.Entries.size())
    return analysisError("metadata entry set differs from request");
  for (size_t I = 0; I < Metadata->size(); ++I)
    if ((*Metadata)[I].Name != Request.Entries[I].Symbol)
      return analysisError("metadata entry symbol differs from request");

  auto Symbols = readSymbols(*Object);
  if (!Symbols)
    return Symbols.takeError();

  PhysicalMachineEffectEvidence Evidence;
  Evidence.ExecutionChallenge = Request.ExecutionChallenge;
  Evidence.RequestIdentity = Request.RequestIdentity;
  Evidence.RequestBytes = Request.RequestBytes;
  Evidence.PayloadDigest = Request.PayloadDigest;
  Evidence.PayloadBytes = Request.PayloadBytes;
  Evidence.AnalyzerIdentity = Request.AnalyzerIdentity;
  Evidence.ToolchainIdentity = Request.ToolchainIdentity;

  for (const MetadataKernel &Kernel : *Metadata) {
    const SymbolRecord *Entry = findSymbol(*Symbols, Kernel.Name);
    const SymbolRecord *Descriptor = findSymbol(*Symbols, Kernel.Descriptor);
    if (!Entry || !Descriptor)
      return analysisError(Twine("entry or descriptor symbol is absent: ") +
                           Kernel.Name);
    auto EntryEvidence = validateDescriptor(Kernel, *Entry, *Descriptor);
    if (!EntryEvidence)
      return EntryEvidence.takeError();
    Evidence.Entries.push_back(std::move(*EntryEvidence));
  }

  auto Mc = createMcState();
  if (!Mc)
    return Mc.takeError();
  std::map<std::string, AnalyzedFunction> Functions;
  std::vector<std::string> Pending;
  for (const auto &Entry : Request.Entries)
    Pending.push_back(Entry.Symbol);
  while (!Pending.empty()) {
    std::string Name = std::move(Pending.back());
    Pending.pop_back();
    if (Functions.contains(Name))
      continue;
    if (Functions.size() >= MaxPhysicalMachineEffectFunctions)
      return analysisError("reachable function count exceeds bound");
    const SymbolRecord *Function = findSymbol(*Symbols, Name);
    if (!Function)
      return analysisError(Twine("reachable function symbol is absent: ") +
                           Name);
    auto Analyzed = analyzeFunction(*Function, *Symbols, *Mc);
    if (!Analyzed)
      return Analyzed.takeError();
    for (const std::string &Callee : Analyzed->Evidence.DirectCallees)
      Pending.push_back(Callee);
    Functions.emplace(Name, std::move(*Analyzed));
  }
  if (!hasAcyclicCallGraph(Functions))
    return analysisError("recursive direct-call graph is unsupported");

  for (const auto &[Name, Function] : Functions)
    Evidence.Functions.push_back(Function.Evidence);

  for (const auto &Entry : Request.Entries) {
    std::set<std::string> Closure = reachableFrom(Entry.Symbol, Functions);
    if (Error ErrorValue = validateBudget(Entry, Closure, Functions))
      return ErrorValue;
    for (const std::string &Name : Closure)
      for (const LocalEffect &Effect : Functions.at(Name).Effects) {
        if (Evidence.Effects.size() >= MaxPhysicalMachineEffectEffects)
          return analysisError("effect count exceeds bound");
        Evidence.Effects.push_back(
            {Entry.Symbol, Name, Effect.Offset, Effect.Kind, Effect.Width});
      }
  }
  llvm::sort(Evidence.Effects, [](const PhysicalMachineEffect &Left,
                                  const PhysicalMachineEffect &Right) {
    return std::tie(Left.EntrySymbol, Left.FunctionSymbol,
                    Left.InstructionOffset, Left.Kind, Left.ByteWidth) <
           std::tie(Right.EntrySymbol, Right.FunctionSymbol,
                    Right.InstructionOffset, Right.Kind, Right.ByteWidth);
  });
  return Evidence;
}

Expected<std::vector<uint8_t>> encodePhysicalMachineEffectEvidence(
    const PhysicalMachineEffectEvidence &Evidence) {
  if (Evidence.Entries.empty() || Evidence.Entries.size() > MaxEntries ||
      Evidence.Functions.empty() ||
      Evidence.Functions.size() > MaxPhysicalMachineEffectFunctions ||
      Evidence.Effects.size() > MaxPhysicalMachineEffectEffects)
    return analysisError("evidence count exceeds bound");

  std::vector<uint8_t> Output;
  Output.insert(Output.end(), EvidenceDomain.bytes_begin(),
                EvidenceDomain.bytes_end());
  appendU32(Output, 0);
  appendU16(Output, SchemaVersion);
  Output.insert(Output.end(), Evidence.ExecutionChallenge.begin(),
                Evidence.ExecutionChallenge.end());
  Output.insert(Output.end(), Evidence.RequestIdentity.begin(),
                Evidence.RequestIdentity.end());
  appendU64(Output, Evidence.RequestBytes);
  Output.insert(Output.end(), Evidence.PayloadDigest.begin(),
                Evidence.PayloadDigest.end());
  appendU64(Output, Evidence.PayloadBytes);
  Output.insert(Output.end(), Evidence.AnalyzerIdentity.begin(),
                Evidence.AnalyzerIdentity.end());
  Output.insert(Output.end(), Evidence.ToolchainIdentity.begin(),
                Evidence.ToolchainIdentity.end());
  appendU16(Output, 1);

  appendU16(Output, static_cast<uint16_t>(Evidence.Entries.size()));
  for (const PhysicalMachineEntryEvidence &Entry : Evidence.Entries) {
    if (Error ErrorValue = appendText(Output, Entry.Symbol))
      return ErrorValue;
    Output.insert(Output.end(), Entry.DescriptorIdentity.begin(),
                  Entry.DescriptorIdentity.end());
    appendU64(Output, Entry.CodeOffset);
    appendU64(Output, Entry.CodeSize);
  }

  appendU32(Output, static_cast<uint32_t>(Evidence.Functions.size()));
  size_t EdgeCount = 0;
  for (const PhysicalMachineFunctionEvidence &Function : Evidence.Functions) {
    if (Error ErrorValue = appendText(Output, Function.Symbol))
      return ErrorValue;
    appendU64(Output, Function.CodeOffset);
    appendU64(Output, Function.CodeSize);
    EdgeCount += Function.DirectCallees.size();
    if (EdgeCount > MaxEdges ||
        Function.DirectCallees.size() > std::numeric_limits<uint16_t>::max())
      return analysisError("evidence call edge count exceeds bound");
    appendU16(Output, static_cast<uint16_t>(Function.DirectCallees.size()));
    for (const std::string &Callee : Function.DirectCallees)
      if (Error ErrorValue = appendText(Output, Callee))
        return ErrorValue;
  }

  appendU32(Output, static_cast<uint32_t>(Evidence.Effects.size()));
  for (const PhysicalMachineEffect &Effect : Evidence.Effects) {
    if (Error ErrorValue = appendText(Output, Effect.EntrySymbol))
      return ErrorValue;
    if (Error ErrorValue = appendText(Output, Effect.FunctionSymbol))
      return ErrorValue;
    appendU64(Output, Effect.InstructionOffset);
    Output.push_back(static_cast<uint8_t>(Effect.Kind));
    appendU16(Output, Effect.ByteWidth);
  }
  if (Output.size() > MaxPhysicalMachineEffectEvidenceBytes ||
      Output.size() > std::numeric_limits<uint32_t>::max())
    return analysisError("evidence bytes exceed bound");
  support::endian::write32le(Output.data() + EvidenceDomain.size(),
                             static_cast<uint32_t>(Output.size()));
  return Output;
}

} // namespace fe2o3::worker
