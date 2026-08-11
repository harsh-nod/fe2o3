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
constexpr size_t MaxElfSections = 256;
constexpr size_t MaxProgramHeaders = 32;
constexpr size_t MaxSymbolsPerTable = 4096;
constexpr size_t MaxStringTableBytes = 1024 * 1024;
constexpr size_t MaxMetadataBytes = 1024 * 1024;

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

struct LoaderSegment {
  uint64_t FileOffset = 0;
  uint64_t Address = 0;
  uint64_t FileSize = 0;
  uint64_t MemorySize = 0;
  uint32_t Flags = 0;
};

struct LoaderView {
  ArrayRef<uint8_t> Payload;
  std::vector<LoaderSegment> Segments;
  std::optional<LoaderSegment> MetadataNote;
  std::optional<LoaderSegment> DynamicTable;

  Expected<uint64_t> fileOffset(uint64_t Address, uint64_t Size,
                                uint32_t RequiredFlags, uint32_t ForbiddenFlags,
                                StringRef Description) const {
    std::optional<uint64_t> Result;
    for (const LoaderSegment &Segment : Segments) {
      if ((Segment.Flags & RequiredFlags) != RequiredFlags ||
          (Segment.Flags & ForbiddenFlags) != 0 || Address < Segment.Address)
        continue;
      uint64_t Delta = Address - Segment.Address;
      if (Delta > Segment.FileSize || Size > Segment.FileSize - Delta)
        continue;
      if (Segment.FileOffset > std::numeric_limits<uint64_t>::max() - Delta)
        return analysisError(Twine(Description) +
                             " loader file offset overflows");
      uint64_t Current = Segment.FileOffset + Delta;
      if (Result && *Result != Current)
        return analysisError(Twine(Description) +
                             " has ambiguous PT_LOAD mappings");
      Result = Current;
    }
    if (!Result)
      return analysisError(Twine(Description) +
                           " is outside a permitted file-backed PT_LOAD");
    if (*Result > Payload.size() || Size > Payload.size() - *Result)
      return analysisError(Twine(Description) +
                           " loader bytes are outside payload");
    return *Result;
  }

  Error validateSection(const ELF64LE::Shdr &Section,
                        StringRef Description) const {
    if (Section.sh_type != ELF::SHT_NOBITS &&
        (Section.sh_offset > Payload.size() ||
         Section.sh_size > Payload.size() - Section.sh_offset))
      return analysisError(Twine(Description) +
                           " section bytes are outside payload");
    if ((Section.sh_flags & ELF::SHF_ALLOC) == 0 || Section.sh_size == 0)
      return Error::success();
    if (Section.sh_type == ELF::SHT_NOBITS) {
      size_t Matches = 0;
      for (const LoaderSegment &Segment : Segments) {
        if (Section.sh_addr < Segment.Address)
          continue;
        uint64_t Delta = Section.sh_addr - Segment.Address;
        if (Delta <= Segment.MemorySize &&
            Section.sh_size <= Segment.MemorySize - Delta)
          ++Matches;
      }
      if (Matches != 1)
        return analysisError(Twine(Description) +
                             " has ambiguous loader memory mapping");
      return Error::success();
    }

    uint32_t Required = ELF::PF_R;
    uint32_t Forbidden = 0;
    if ((Section.sh_flags & ELF::SHF_EXECINSTR) != 0) {
      Required |= ELF::PF_X;
      Forbidden |= ELF::PF_W;
    }
    if ((Section.sh_flags & ELF::SHF_WRITE) != 0)
      Required |= ELF::PF_W;
    auto Offset = fileOffset(Section.sh_addr, Section.sh_size, Required,
                             Forbidden, Description);
    if (!Offset)
      return Offset.takeError();
    if (*Offset != Section.sh_offset)
      return analysisError(Twine(Description) +
                           " section and PT_LOAD file views disagree");
    return Error::success();
  }

  Error validateExactSegment(const ELF64LE::Shdr &Section,
                             const LoaderSegment &Segment,
                             StringRef Description) const {
    if (Error ErrorValue = validateSection(Section, Description))
      return ErrorValue;
    if (Section.sh_offset != Segment.FileOffset ||
        Section.sh_addr != Segment.Address ||
        Section.sh_size != Segment.FileSize ||
        Segment.FileSize != Segment.MemorySize)
      return analysisError(Twine(Description) +
                           " section and program-header views disagree");
    if (Section.sh_type == ELF::SHT_NOBITS)
      return analysisError(Twine(Description) + " has no file bytes");
    return Error::success();
  }

  Expected<ArrayRef<uint8_t>> bytes(uint64_t Address, uint64_t Size,
                                    uint32_t RequiredFlags,
                                    uint32_t ForbiddenFlags,
                                    uint64_t ExpectedSectionOffset,
                                    StringRef Description) const {
    auto Offset =
        fileOffset(Address, Size, RequiredFlags, ForbiddenFlags, Description);
    if (!Offset)
      return Offset.takeError();
    if (*Offset != ExpectedSectionOffset)
      return analysisError(Twine(Description) +
                           " section and loader bytes disagree");
    return Payload.slice(static_cast<size_t>(*Offset),
                         static_cast<size_t>(Size));
  }
};

bool rangesOverlap(uint64_t Left, uint64_t LeftSize, uint64_t Right,
                   uint64_t RightSize) {
  if (LeftSize == 0 || RightSize == 0)
    return false;
  return Left < Right + RightSize && Right < Left + LeftSize;
}

Expected<LoaderView> buildLoaderView(const ELFObjectFile<ELF64LE> &Object,
                                     ArrayRef<uint8_t> Payload) {
  const ELFFile<ELF64LE> &File = Object.getELFFile();
  auto Headers = File.program_headers();
  if (!Headers)
    return Headers.takeError();
  if (Headers->empty() || Headers->size() > MaxProgramHeaders)
    return analysisError("program-header count is outside bounded profile");

  LoaderView Result{Payload, {}, std::nullopt, std::nullopt};
  for (const ELF64LE::Phdr &Header : *Headers) {
    if (Header.p_filesz != 0 &&
        (Header.p_offset > Payload.size() ||
         Header.p_filesz > Payload.size() - Header.p_offset))
      return analysisError("program-header file range is outside payload");
    if (Header.p_type == ELF::PT_NOTE || Header.p_type == ELF::PT_DYNAMIC) {
      std::optional<LoaderSegment> &Destination = Header.p_type == ELF::PT_NOTE
                                                      ? Result.MetadataNote
                                                      : Result.DynamicTable;
      StringRef Description =
          Header.p_type == ELF::PT_NOTE ? "PT_NOTE" : "PT_DYNAMIC";
      uint32_t ExpectedFlags =
          Header.p_type == ELF::PT_NOTE ? ELF::PF_R : ELF::PF_R | ELF::PF_W;
      uint64_t ExpectedAlignment = Header.p_type == ELF::PT_NOTE ? 4 : 8;
      if (Destination)
        return analysisError(Twine("multiple ") + Description +
                             " program headers");
      if (Header.p_filesz == 0 || Header.p_filesz != Header.p_memsz ||
          Header.p_flags != ExpectedFlags ||
          Header.p_align != ExpectedAlignment)
        return analysisError(Twine(Description) +
                             " is outside bounded loader profile");
      Destination =
          LoaderSegment{Header.p_offset, Header.p_vaddr, Header.p_filesz,
                        Header.p_memsz, Header.p_flags};
    }
    if (Header.p_type != ELF::PT_LOAD)
      continue;
    if (Header.p_memsz == 0 || Header.p_filesz > Header.p_memsz)
      return analysisError("PT_LOAD size is invalid");
    if ((Header.p_flags & ~(ELF::PF_R | ELF::PF_W | ELF::PF_X)) != 0 ||
        (Header.p_flags & ELF::PF_R) == 0 ||
        (Header.p_flags & (ELF::PF_W | ELF::PF_X)) == (ELF::PF_W | ELF::PF_X))
      return analysisError("PT_LOAD permissions are outside bounded profile");
    if (Header.p_vaddr > std::numeric_limits<uint64_t>::max() - Header.p_memsz)
      return analysisError("PT_LOAD virtual range overflows");
    if (Header.p_align > 1 &&
        ((Header.p_align & (Header.p_align - 1)) != 0 ||
         Header.p_offset % Header.p_align != Header.p_vaddr % Header.p_align))
      return analysisError("PT_LOAD alignment is invalid");
    Result.Segments.push_back({Header.p_offset, Header.p_vaddr, Header.p_filesz,
                               Header.p_memsz, Header.p_flags});
  }
  if (Result.Segments.empty() ||
      !llvm::any_of(Result.Segments, [](const LoaderSegment &Segment) {
        return (Segment.Flags & ELF::PF_X) != 0 && Segment.FileSize != 0;
      }))
    return analysisError("loadable executable segment is absent");
  if (!Result.MetadataNote || !Result.DynamicTable)
    return analysisError(
        "bounded loader profile requires one PT_NOTE and one PT_DYNAMIC");
  for (size_t I = 0; I < Result.Segments.size(); ++I) {
    const LoaderSegment &Left = Result.Segments[I];
    for (size_t J = I + 1; J < Result.Segments.size(); ++J) {
      const LoaderSegment &Right = Result.Segments[J];
      if (rangesOverlap(Left.Address, Left.MemorySize, Right.Address,
                        Right.MemorySize))
        return analysisError("PT_LOAD virtual mappings overlap");
      if (((Left.Flags | Right.Flags) & ELF::PF_X) != 0 &&
          rangesOverlap(Left.FileOffset, Left.FileSize, Right.FileOffset,
                        Right.FileSize))
        return analysisError("executable PT_LOAD file mappings alias");
    }
  }
  for (const auto &[Description, Segment] :
       {std::pair<StringRef, const LoaderSegment *>("PT_NOTE",
                                                    &*Result.MetadataNote),
        std::pair<StringRef, const LoaderSegment *>("PT_DYNAMIC",
                                                    &*Result.DynamicTable)}) {
    uint32_t Forbidden =
        Description == "PT_NOTE" ? ELF::PF_W | ELF::PF_X : ELF::PF_X;
    auto Offset = Result.fileOffset(Segment->Address, Segment->FileSize,
                                    Segment->Flags, Forbidden, Description);
    if (!Offset)
      return Offset.takeError();
    if (*Offset != Segment->FileOffset)
      return analysisError(Twine(Description) +
                           " and PT_LOAD file views disagree");
  }

  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  if (Sections->empty() || Sections->size() > MaxElfSections)
    return analysisError("section count is outside bounded profile");
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (Error ErrorValue = Result.validateSection(Section, *Name))
      return ErrorValue;
  }
  return Result;
}

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
readMetadata(const ELFObjectFile<ELF64LE> &Object, const LoaderView &Loader) {
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
    if (Error ErrorValue = Loader.validateSection(Section, "metadata note"))
      return ErrorValue;
    Error NoteError = Error::success();
    for (const ELF64LE::Note Note : File.notes(Section, NoteError)) {
      if (Note.getName() != "AMDGPU" ||
          Note.getType() != ELF::NT_AMDGPU_METADATA)
        continue;
      if (++MetadataNoteCount != 1)
        return analysisError("multiple AMDGPU metadata notes");
      if (Error ErrorValue = Loader.validateExactSegment(
              Section, *Loader.MetadataNote, "AMDGPU metadata note"))
        return ErrorValue;
      if (Section.sh_addralign != 4)
        return analysisError("metadata note alignment is not four");
      StringRef Blob = Note.getDescAsStringRef(Section.sh_addralign);
      if (Blob.empty() || Blob.size() > MaxMetadataBytes)
        return analysisError("metadata note size is outside bounded profile");
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
      if ((**Kernels).getArray().empty() ||
          (**Kernels).getArray().size() > MaxEntries)
        return analysisError("metadata kernel count exceeds bounded profile");
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
  uint64_t FileOffset = 0;
  uint32_t SectionIndex = 0;
  uint8_t Type = ELF::STT_NOTYPE;
  uint8_t Binding = ELF::STB_LOCAL;
  uint8_t Visibility = ELF::STV_DEFAULT;
  bool Text = false;
  bool Data = false;
  ArrayRef<uint8_t> Bytes;
};

struct DynamicLoaderSections {
  size_t Dynsym = 0;
  size_t Dynstr = 0;
  size_t GnuHash = 0;
  size_t Hash = 0;
  size_t Dynamic = 0;
};

const SymbolRecord *findSymbol(ArrayRef<SymbolRecord> Symbols, StringRef Name);

Expected<std::vector<SymbolRecord>>
readSymbolTable(const ELFObjectFile<ELF64LE> &Object,
                ArrayRef<ELF64LE::Shdr> Sections, const ELF64LE::Shdr &Table,
                const LoaderView &Loader, StringRef TableName) {
  const ELFFile<ELF64LE> &File = Object.getELFFile();
  if (Table.sh_entsize != sizeof(ELF64LE::Sym) ||
      Table.sh_size % sizeof(ELF64LE::Sym) != 0 ||
      Table.sh_size / sizeof(ELF64LE::Sym) > MaxSymbolsPerTable)
    return analysisError(Twine(TableName) +
                         " symbol count is outside bounded profile");
  auto Symbols = File.symbols(&Table);
  if (!Symbols)
    return Symbols.takeError();
  auto StringTable = File.getStringTableForSymtab(Table, Sections);
  if (!StringTable)
    return StringTable.takeError();
  if (StringTable->size() > MaxStringTableBytes)
    return analysisError(Twine(TableName) +
                         " string table exceeds bounded profile");

  std::vector<SymbolRecord> Result;
  Result.reserve(Symbols->size());
  for (const ELF64LE::Sym &Symbol : *Symbols) {
    auto Name = Symbol.getName(*StringTable);
    if (!Name)
      return Name.takeError();
    if (Name->empty())
      continue;
    if (Name->size() > MaxSymbolBytes || !Reader::validSymbol(*Name))
      return analysisError(Twine(TableName) + " contains invalid symbol name");
    if (Symbol.isUndefined() || Symbol.isAbsolute() || Symbol.isCommon())
      continue;
    if (Symbol.st_shndx == ELF::SHN_XINDEX ||
        Symbol.st_shndx >= Sections.size())
      return analysisError(Twine(TableName) +
                           " symbol section index is unsupported");
    const ELF64LE::Shdr &Section = Sections[Symbol.st_shndx];
    if (Symbol.st_value < Section.sh_addr)
      return analysisError(Twine(TableName) + " symbol precedes section");
    uint64_t Delta = Symbol.st_value - Section.sh_addr;
    if (Delta > Section.sh_size || Symbol.st_size > Section.sh_size - Delta)
      return analysisError(Twine(TableName) +
                           " symbol range is outside section");

    bool Text = (Section.sh_flags & (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR)) ==
                (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR);
    bool Data = (Section.sh_flags & ELF::SHF_ALLOC) != 0 && !Text;
    uint64_t FileOffset = 0;
    ArrayRef<uint8_t> Bytes;
    if (Symbol.st_size != 0 && (Text || Data)) {
      if (Section.sh_type == ELF::SHT_NOBITS ||
          Section.sh_offset > std::numeric_limits<uint64_t>::max() - Delta)
        return analysisError(Twine(TableName) +
                             " symbol has no bounded file bytes");
      FileOffset = Section.sh_offset + Delta;
      uint32_t Required = ELF::PF_R;
      uint32_t Forbidden = 0;
      if (Text) {
        Required |= ELF::PF_X;
        Forbidden |= ELF::PF_W;
      } else if ((Section.sh_flags & ELF::SHF_WRITE) != 0) {
        Required |= ELF::PF_W;
      }
      auto Mapped = Loader.bytes(Symbol.st_value, Symbol.st_size, Required,
                                 Forbidden, FileOffset, *Name);
      if (!Mapped)
        return Mapped.takeError();
      Bytes = *Mapped;
    }
    Result.push_back({Name->str(), Symbol.st_value, Symbol.st_size, FileOffset,
                      Symbol.st_shndx, Symbol.getType(), Symbol.getBinding(),
                      Symbol.getVisibility(), Text, Data, Bytes});
  }
  llvm::sort(Result, [](const SymbolRecord &Left, const SymbolRecord &Right) {
    return std::tie(Left.Name, Left.Address, Left.Size) <
           std::tie(Right.Name, Right.Address, Right.Size);
  });
  for (size_t I = 1; I < Result.size(); ++I)
    if (Result[I - 1].Name == Result[I].Name)
      return analysisError(Twine(TableName) +
                           " duplicate symbol: " + Result[I].Name);
  return Result;
}

Expected<std::vector<SymbolRecord>>
readSymbols(const ELFObjectFile<ELF64LE> &Object, const LoaderView &Loader,
            const DynamicLoaderSections &DynamicLoader,
            ArrayRef<MetadataKernel> Metadata) {
  const ELFFile<ELF64LE> &File = Object.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  const ELF64LE::Shdr *StaticTable = nullptr;
  const ELF64LE::Shdr *DynamicTable = &(*Sections)[DynamicLoader.Dynsym];
  for (const ELF64LE::Shdr &Section : *Sections) {
    if (Section.sh_type == ELF::SHT_SYMTAB) {
      if (StaticTable)
        return analysisError("multiple .symtab sections");
      StaticTable = &Section;
    }
  }
  if (!StaticTable)
    return analysisError("bounded profile requires .symtab and .dynsym");
  auto Static =
      readSymbolTable(Object, *Sections, *StaticTable, Loader, ".symtab");
  if (!Static)
    return Static.takeError();
  auto Dynamic =
      readSymbolTable(Object, *Sections, *DynamicTable, Loader, ".dynsym");
  if (!Dynamic)
    return Dynamic.takeError();

  for (const MetadataKernel &Kernel : Metadata) {
    for (StringRef Name :
         {StringRef(Kernel.Name), StringRef(Kernel.Descriptor)}) {
      const SymbolRecord *StaticSymbol = findSymbol(*Static, Name);
      const SymbolRecord *DynamicSymbol = findSymbol(*Dynamic, Name);
      if (!StaticSymbol || !DynamicSymbol)
        return analysisError(
            Twine("kernel export is absent from symbol view: ") + Name);
      if (std::tie(StaticSymbol->Address, StaticSymbol->Size,
                   StaticSymbol->SectionIndex, StaticSymbol->Type) !=
              std::tie(DynamicSymbol->Address, DynamicSymbol->Size,
                       DynamicSymbol->SectionIndex, DynamicSymbol->Type) ||
          DynamicSymbol->Binding != ELF::STB_GLOBAL ||
          (DynamicSymbol->Visibility != ELF::STV_DEFAULT &&
           DynamicSymbol->Visibility != ELF::STV_PROTECTED))
        return analysisError(Twine(".symtab/.dynsym export mismatch: ") + Name);
    }
  }
  return Static;
}

bool isRelocationSection(uint32_t Type) {
  return Type == ELF::SHT_REL || Type == ELF::SHT_RELA ||
         Type == ELF::SHT_RELR || Type == ELF::SHT_CREL ||
         Type == ELF::SHT_ANDROID_REL || Type == ELF::SHT_ANDROID_RELA ||
         Type == ELF::SHT_ANDROID_RELR;
}

bool isRelocationDynamicTag(int64_t Tag) {
  // Generic ELF DT_* relocation tables plus Android packed relocations.
  switch (Tag) {
  case 2:          // DT_PLTRELSZ
  case 7:          // DT_RELA
  case 8:          // DT_RELASZ
  case 9:          // DT_RELAENT
  case 17:         // DT_REL
  case 18:         // DT_RELSZ
  case 19:         // DT_RELENT
  case 20:         // DT_PLTREL
  case 23:         // DT_JMPREL
  case 35:         // DT_RELRSZ
  case 36:         // DT_RELR
  case 37:         // DT_RELRENT
  case 0x6000000f: // DT_ANDROID_REL
  case 0x60000010: // DT_ANDROID_RELSZ
  case 0x60000011: // DT_ANDROID_RELA
  case 0x60000012: // DT_ANDROID_RELASZ
    return true;
  default:
    return false;
  }
}

Expected<DynamicLoaderSections>
validateDynamicLoaderView(const ELFObjectFile<ELF64LE> &Object,
                          const LoaderView &Loader) {
  const ELFFile<ELF64LE> &File = Object.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  std::optional<size_t> Dynsym;
  std::optional<size_t> Dynstr;
  std::optional<size_t> GnuHash;
  std::optional<size_t> Hash;
  std::optional<size_t> Dynamic;
  for (size_t Index = 0; Index < Sections->size(); ++Index) {
    const ELF64LE::Shdr &Section = (*Sections)[Index];
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    bool Relocation = isRelocationSection(Section.sh_type);
    bool RelocationName = *Name == ".rel" || Name->starts_with(".rel.") ||
                          *Name == ".rela" || Name->starts_with(".rela.") ||
                          *Name == ".relr" || Name->starts_with(".relr.");
    if (RelocationName && !Relocation)
      return analysisError(Twine("relocation-named section has wrong type: ") +
                           *Name + " type=" + Twine(Section.sh_type));
    if (Relocation && Section.sh_size != 0) {
      if (Section.sh_info >= Sections->size() && Section.sh_info != 0)
        return analysisError("relocation target section is invalid");
      return analysisError(Twine("unsupported finalized-image relocations: ") +
                           *Name);
    }
    auto Select = [&](StringRef ExpectedName, uint32_t ExpectedType,
                      std::optional<size_t> &Slot) -> Error {
      if (*Name != ExpectedName && Section.sh_type != ExpectedType)
        return Error::success();
      if (*Name != ExpectedName || Section.sh_type != ExpectedType)
        return analysisError(Twine(ExpectedName) +
                             " name/type view is inconsistent");
      if (Slot)
        return analysisError(Twine("multiple ") + ExpectedName + " sections");
      Slot = Index;
      return Error::success();
    };
    if (Error ErrorValue = Select(".dynsym", ELF::SHT_DYNSYM, Dynsym))
      return ErrorValue;
    if (*Name == ".dynstr") {
      if (Section.sh_type != ELF::SHT_STRTAB || Dynstr)
        return analysisError(".dynstr section view is inconsistent");
      Dynstr = Index;
    }
    if (Error ErrorValue = Select(".gnu.hash", ELF::SHT_GNU_HASH, GnuHash))
      return ErrorValue;
    if (Error ErrorValue = Select(".hash", ELF::SHT_HASH, Hash))
      return ErrorValue;
    if (Error ErrorValue = Select(".dynamic", ELF::SHT_DYNAMIC, Dynamic))
      return ErrorValue;
  }
  if (!Dynsym || !Dynstr || !GnuHash || !Hash || !Dynamic)
    return analysisError(
        "bounded dynamic loader sections are absent or incomplete");

  const ELF64LE::Shdr &DynsymSection = (*Sections)[*Dynsym];
  const ELF64LE::Shdr &DynstrSection = (*Sections)[*Dynstr];
  const ELF64LE::Shdr &GnuHashSection = (*Sections)[*GnuHash];
  const ELF64LE::Shdr &HashSection = (*Sections)[*Hash];
  const ELF64LE::Shdr &DynamicSection = (*Sections)[*Dynamic];
  if (DynsymSection.sh_link != *Dynstr || GnuHashSection.sh_link != *Dynsym ||
      HashSection.sh_link != *Dynsym || DynamicSection.sh_link != *Dynstr)
    return analysisError("dynamic loader section links are inconsistent");
  if (Error ErrorValue = Loader.validateExactSegment(
          DynamicSection, *Loader.DynamicTable, ".dynamic"))
    return ErrorValue;
  for (const auto &[Section, Description] :
       {std::pair<const ELF64LE::Shdr *, StringRef>(&DynsymSection, ".dynsym"),
        std::pair<const ELF64LE::Shdr *, StringRef>(&DynstrSection, ".dynstr"),
        std::pair<const ELF64LE::Shdr *, StringRef>(&GnuHashSection,
                                                    ".gnu.hash"),
        std::pair<const ELF64LE::Shdr *, StringRef>(&HashSection, ".hash")})
    if (Error ErrorValue = Loader.validateSection(*Section, Description))
      return ErrorValue;

  if (DynsymSection.sh_entsize != sizeof(ELF64LE::Sym) ||
      DynsymSection.sh_size % sizeof(ELF64LE::Sym) != 0)
    return analysisError(".dynsym has invalid entry geometry");
  const uint64_t SymbolCount = DynsymSection.sh_size / sizeof(ELF64LE::Sym);
  if (SymbolCount == 0 || SymbolCount > MaxSymbolsPerTable)
    return analysisError(".dynsym symbol count is outside bounded profile");

  auto HashBytes =
      Loader.bytes(HashSection.sh_addr, HashSection.sh_size, ELF::PF_R,
                   ELF::PF_W | ELF::PF_X, HashSection.sh_offset, ".hash");
  if (!HashBytes)
    return HashBytes.takeError();
  if (HashBytes->size() < 8 || HashBytes->size() % 4 != 0)
    return analysisError(".hash geometry is invalid");
  uint64_t BucketCount = support::endian::read32le(HashBytes->data());
  uint64_t ChainCount = support::endian::read32le(HashBytes->data() + 4);
  if (BucketCount == 0 || ChainCount != SymbolCount ||
      BucketCount >
          (std::numeric_limits<uint64_t>::max() / 4) - ChainCount - 2 ||
      HashBytes->size() != (2 + BucketCount + ChainCount) * 4)
    return analysisError(".hash does not exactly describe .dynsym");

  auto GnuHashBytes = Loader.bytes(
      GnuHashSection.sh_addr, GnuHashSection.sh_size, ELF::PF_R,
      ELF::PF_W | ELF::PF_X, GnuHashSection.sh_offset, ".gnu.hash");
  if (!GnuHashBytes)
    return GnuHashBytes.takeError();
  if (GnuHashBytes->size() < 16)
    return analysisError(".gnu.hash geometry is invalid");
  uint64_t GnuBucketCount = support::endian::read32le(GnuHashBytes->data());
  uint64_t SymbolOffset = support::endian::read32le(GnuHashBytes->data() + 4);
  uint64_t BloomCount = support::endian::read32le(GnuHashBytes->data() + 8);
  if (GnuBucketCount == 0 || BloomCount == 0 || SymbolOffset > SymbolCount ||
      BloomCount > (std::numeric_limits<uint64_t>::max() - 16) / 8 ||
      GnuBucketCount >
          (std::numeric_limits<uint64_t>::max() - 16 - BloomCount * 8) / 4)
    return analysisError(".gnu.hash header is outside bounded profile");
  uint64_t PrefixBytes = 16 + BloomCount * 8 + GnuBucketCount * 4;
  uint64_t ChainCountGnu = SymbolCount - SymbolOffset;
  if (ChainCountGnu >
          (std::numeric_limits<uint64_t>::max() - PrefixBytes) / 4 ||
      GnuHashBytes->size() != PrefixBytes + ChainCountGnu * 4)
    return analysisError(".gnu.hash does not exactly describe .dynsym");
  for (uint64_t Index = 0; Index < GnuBucketCount; ++Index) {
    uint32_t Bucket = support::endian::read32le(GnuHashBytes->data() + 16 +
                                                BloomCount * 8 + Index * 4);
    if (Bucket != 0 && (Bucket < SymbolOffset || Bucket >= SymbolCount))
      return analysisError(".gnu.hash bucket is outside .dynsym");
  }

  if (DynamicSection.sh_entsize != sizeof(ELF64LE::Dyn) ||
      DynamicSection.sh_size % sizeof(ELF64LE::Dyn) != 0 ||
      DynamicSection.sh_size / sizeof(ELF64LE::Dyn) > 256)
    return analysisError("dynamic table exceeds bounded profile");
  auto Entries = File.getSectionContentsAsArray<ELF64LE::Dyn>(DynamicSection);
  if (!Entries)
    return Entries.takeError();
  std::map<int64_t, uint64_t> Tags;
  bool Terminated = false;
  for (const ELF64LE::Dyn &Entry : *Entries) {
    int64_t Tag = Entry.getTag();
    if (Tag == ELF::DT_NULL) {
      if (Terminated)
        return analysisError("dynamic table has duplicate terminators");
      Terminated = true;
      continue;
    }
    if (Terminated)
      return analysisError("dynamic table has declarations after DT_NULL");
    if (isRelocationDynamicTag(Tag))
      return analysisError("dynamic relocation table is unsupported");
    if (Tag != ELF::DT_SYMTAB && Tag != ELF::DT_SYMENT &&
        Tag != ELF::DT_STRTAB && Tag != ELF::DT_STRSZ &&
        Tag != ELF::DT_GNU_HASH && Tag != ELF::DT_HASH)
      return analysisError("dynamic declaration is outside bounded profile");
    if (!Tags.emplace(Tag, Entry.getVal()).second)
      return analysisError("dynamic table repeats a declaration");
  }
  if (!Terminated || Tags.size() != 6 ||
      Tags[ELF::DT_SYMTAB] != DynsymSection.sh_addr ||
      Tags[ELF::DT_SYMENT] != sizeof(ELF64LE::Sym) ||
      Tags[ELF::DT_STRTAB] != DynstrSection.sh_addr ||
      Tags[ELF::DT_STRSZ] != DynstrSection.sh_size ||
      Tags[ELF::DT_GNU_HASH] != GnuHashSection.sh_addr ||
      Tags[ELF::DT_HASH] != HashSection.sh_addr)
    return analysisError(
        "dynamic declarations disagree with loadable sections");

  return DynamicLoaderSections{*Dynsym, *Dynstr, *GnuHash, *Hash, *Dynamic};
}

Expected<ArrayRef<uint8_t>> symbolBytes(const SymbolRecord &Symbol) {
  if (Symbol.Size != Symbol.Bytes.size())
    return analysisError(Twine("symbol has no exact loader bytes: ") +
                         Symbol.Name);
  return Symbol.Bytes;
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
  if (Entry.Type != ELF::STT_FUNC || Entry.Size == 0 || !Entry.Text)
    return analysisError(Twine("entry is not a bounded text function: ") +
                         Entry.Name);
  if (Descriptor.Type != ELF::STT_OBJECT || Descriptor.Size != 64 ||
      !Descriptor.Data)
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
  if (Name.contains("DWORDX16"))
    return 64;
  if (Name.contains("DWORDX8"))
    return 32;
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

bool isEndProgram(const DecodedInstruction &Instruction) {
  return StringRef(Instruction.Name).starts_with("S_ENDPGM");
}

bool isSetPc(const DecodedInstruction &Instruction) {
  return Instruction.Name == "S_SETPC_B64_vi";
}

struct FunctionCfg {
  struct Block {
    size_t Begin = 0;
    size_t End = 0;
    std::vector<size_t> Successors;
    std::vector<size_t> Predecessors;
    bool Reachable = false;
  };

  std::vector<Block> Blocks;
  std::vector<size_t> InstructionBlocks;
};

void appendUnique(std::vector<size_t> &Values, size_t Value) {
  if (llvm::find(Values, Value) == Values.end())
    Values.push_back(Value);
}

Expected<FunctionCfg>
buildFunctionCfg(ArrayRef<DecodedInstruction> Instructions, McState &Mc,
                 StringRef FunctionName) {
  std::map<uint64_t, size_t> Boundaries;
  for (size_t I = 0; I < Instructions.size(); ++I)
    if (!Boundaries.emplace(Instructions[I].Address, I).second)
      return analysisError(Twine("duplicate instruction boundary in ") +
                           FunctionName);

  std::set<size_t> Leaders{0};
  std::map<size_t, size_t> BranchTargets;
  for (size_t I = 0; I < Instructions.size(); ++I) {
    const DecodedInstruction &Instruction = Instructions[I];
    const MCInstrDesc &Descriptor =
        Mc.Instructions->get(Instruction.Inst.getOpcode());
    bool SetPc = isSetPc(Instruction);
    if (Descriptor.isIndirectBranch() && !SetPc)
      return analysisError(Twine("indirect branch in ") + FunctionName);
    if (Descriptor.isBranch() && !Descriptor.isCall() && !SetPc) {
      uint64_t Target = 0;
      if (!Mc.Analysis->evaluateBranch(Instruction.Inst, Instruction.Address,
                                       Instruction.Size, Target))
        return analysisError(Twine("unknown branch target in ") + FunctionName);
      auto Boundary = Boundaries.find(Target);
      if (Target <= Instruction.Address || Boundary == Boundaries.end())
        return analysisError(
            Twine("unsupported backward or external branch in ") +
            FunctionName);
      BranchTargets.emplace(I, Boundary->second);
      Leaders.insert(Boundary->second);
    }
    if ((Descriptor.isBranch() || Descriptor.isCall() ||
         isEndProgram(Instruction) || SetPc) &&
        I + 1 < Instructions.size())
      Leaders.insert(I + 1);
  }

  FunctionCfg Result;
  Result.InstructionBlocks.resize(Instructions.size());
  std::vector<size_t> OrderedLeaders(Leaders.begin(), Leaders.end());
  for (size_t I = 0; I < OrderedLeaders.size(); ++I) {
    size_t End = I + 1 < OrderedLeaders.size() ? OrderedLeaders[I + 1]
                                               : Instructions.size();
    size_t BlockIndex = Result.Blocks.size();
    Result.Blocks.push_back({OrderedLeaders[I], End, {}, {}, false});
    for (size_t Instruction = OrderedLeaders[I]; Instruction < End;
         ++Instruction)
      Result.InstructionBlocks[Instruction] = BlockIndex;
  }

  for (size_t BlockIndex = 0; BlockIndex < Result.Blocks.size(); ++BlockIndex) {
    FunctionCfg::Block &Block = Result.Blocks[BlockIndex];
    size_t LastIndex = Block.End - 1;
    const DecodedInstruction &Last = Instructions[LastIndex];
    const MCInstrDesc &Descriptor = Mc.Instructions->get(Last.Inst.getOpcode());
    auto AddFallthrough = [&]() -> Error {
      if (Block.End >= Instructions.size())
        return Error::success();
      appendUnique(Block.Successors, Result.InstructionBlocks[Block.End]);
      return Error::success();
    };

    if (isEndProgram(Last) || isSetPc(Last)) {
      // Proven terminal kind and return-pair provenance are checked below.
    } else if (Descriptor.isCall()) {
      if (Error ErrorValue = AddFallthrough())
        return ErrorValue;
    } else if (Descriptor.isBranch()) {
      auto Target = BranchTargets.find(LastIndex);
      if (Target == BranchTargets.end())
        return analysisError(Twine("branch target is absent in ") +
                             FunctionName);
      appendUnique(Block.Successors, Result.InstructionBlocks[Target->second]);
      if (Mc.Analysis->isConditionalBranch(Last.Inst)) {
        if (Error ErrorValue = AddFallthrough())
          return ErrorValue;
      } else if (!Mc.Analysis->isUnconditionalBranch(Last.Inst)) {
        return analysisError(Twine("unsupported branch kind in ") +
                             FunctionName);
      }
    } else if (Error ErrorValue = AddFallthrough()) {
      return ErrorValue;
    }
  }

  for (size_t BlockIndex = 0; BlockIndex < Result.Blocks.size(); ++BlockIndex)
    for (size_t Successor : Result.Blocks[BlockIndex].Successors)
      appendUnique(Result.Blocks[Successor].Predecessors, BlockIndex);

  std::vector<size_t> Pending{0};
  while (!Pending.empty()) {
    size_t BlockIndex = Pending.back();
    Pending.pop_back();
    FunctionCfg::Block &Block = Result.Blocks[BlockIndex];
    if (Block.Reachable)
      continue;
    Block.Reachable = true;
    Pending.insert(Pending.end(), Block.Successors.begin(),
                   Block.Successors.end());
  }
  for (const FunctionCfg::Block &Block : Result.Blocks) {
    if (!Block.Reachable || !Block.Successors.empty())
      continue;
    const DecodedInstruction &Last = Instructions[Block.End - 1];
    if (!isEndProgram(Last) && !isSetPc(Last))
      return analysisError(Twine("reachable fallthrough exits symbol ") +
                           FunctionName);
  }
  return Result;
}

struct ReachingDefinitions {
  std::set<size_t> Instructions;
  bool LiveIn = false;
};

Expected<ReachingDefinitions> reachingDefinitions(
    const FunctionCfg &Cfg, ArrayRef<DecodedInstruction> Instructions,
    size_t BeforeInstruction, unsigned Register, const McState &Mc) {
  if (BeforeInstruction >= Instructions.size())
    return analysisError("reaching-definition query is outside function");
  size_t InitialBlock = Cfg.InstructionBlocks[BeforeInstruction];
  std::vector<std::pair<size_t, size_t>> Pending{
      {InitialBlock, BeforeInstruction}};
  std::set<std::pair<size_t, size_t>> Visited;
  ReachingDefinitions Result;
  while (!Pending.empty()) {
    auto [BlockIndex, Before] = Pending.back();
    Pending.pop_back();
    if (!Visited.insert({BlockIndex, Before}).second)
      continue;
    const FunctionCfg::Block &Block = Cfg.Blocks[BlockIndex];
    bool Found = false;
    for (size_t I = Before; I-- > Block.Begin;) {
      if (definesRegister(Instructions[I], Register, Mc)) {
        Result.Instructions.insert(I);
        Found = true;
        break;
      }
    }
    if (Found)
      continue;
    if (Block.Predecessors.empty()) {
      Result.LiveIn = true;
      continue;
    }
    for (size_t Predecessor : Block.Predecessors) {
      const FunctionCfg::Block &Previous = Cfg.Blocks[Predecessor];
      if (Previous.Reachable)
        Pending.push_back({Predecessor, Previous.End});
    }
  }
  return Result;
}

bool isUniqueDefinition(const ReachingDefinitions &Definitions, size_t Site) {
  return !Definitions.LiveIn && Definitions.Instructions.size() == 1 &&
         *Definitions.Instructions.begin() == Site;
}

bool instructionDominates(const FunctionCfg &Cfg, size_t Definition,
                          size_t Use) {
  size_t DefinitionBlock = Cfg.InstructionBlocks[Definition];
  size_t UseBlock = Cfg.InstructionBlocks[Use];
  if (DefinitionBlock == UseBlock)
    return Definition < Use;
  std::vector<size_t> Pending{0};
  std::set<size_t> Visited;
  while (!Pending.empty()) {
    size_t Block = Pending.back();
    Pending.pop_back();
    if (Block == DefinitionBlock || !Visited.insert(Block).second)
      continue;
    if (Block == UseBlock)
      return false;
    Pending.insert(Pending.end(), Cfg.Blocks[Block].Successors.begin(),
                   Cfg.Blocks[Block].Successors.end());
  }
  return true;
}

Expected<std::pair<unsigned, unsigned>> splitSgprPair(unsigned PairRegister,
                                                      const McState &Mc) {
  StringRef PairName = Mc.Registers->getName(PairRegister);
  auto PairParts = PairName.split('_');
  if (!PairParts.first.starts_with("SGPR") ||
      !PairParts.second.starts_with("SGPR") || PairParts.second.contains('_'))
    return analysisError("register is not one SGPR pair");
  auto Low = registerNamed(PairParts.first, Mc);
  auto High = registerNamed(PairParts.second, Mc);
  if (!Low || !High || *Low == *High)
    return analysisError("SGPR pair is malformed");
  return std::pair<unsigned, unsigned>{*Low, *High};
}

Expected<bool> validatePhysicalReturn(ArrayRef<DecodedInstruction> Instructions,
                                      size_t InstructionIndex,
                                      const FunctionCfg &Cfg,
                                      bool ReturnPairIsLiveIn,
                                      const McState &Mc) {
  const DecodedInstruction &Instruction = Instructions[InstructionIndex];
  if (isEndProgram(Instruction)) {
    if (ReturnPairIsLiveIn)
      return analysisError("callable helper terminates with S_ENDPGM");
    return true;
  }
  if (!isSetPc(Instruction))
    return false;
  if (!ReturnPairIsLiveIn)
    return analysisError("kernel entry attempts S_SETPC return");
  if (Instruction.Inst.size() != 1 || !Instruction.Inst.getOperand(0).isReg() ||
      StringRef(Mc.Registers->getName(
          Instruction.Inst.getOperand(0).getReg())) != "SGPR30_SGPR31")
    return analysisError("S_SETPC does not use the ABI return pair");
  auto Pair = splitSgprPair(Instruction.Inst.getOperand(0).getReg(), Mc);
  if (!Pair)
    return Pair.takeError();
  auto Low =
      reachingDefinitions(Cfg, Instructions, InstructionIndex, Pair->first, Mc);
  if (!Low)
    return Low.takeError();
  auto High = reachingDefinitions(Cfg, Instructions, InstructionIndex,
                                  Pair->second, Mc);
  if (!High)
    return High.takeError();
  if (!Low->LiveIn || !Low->Instructions.empty() || !High->LiveIn ||
      !High->Instructions.empty())
    return analysisError("S_SETPC return pair was modified or is ambiguous");
  return true;
}

Expected<uint64_t> directCallTargets(ArrayRef<DecodedInstruction> Instructions,
                                     size_t CallIndex, const FunctionCfg &Cfg,
                                     McState &Mc) {
  const DecodedInstruction &Call = Instructions[CallIndex];
  if (Call.Inst.size() == 0 || !Call.Inst.getOperand(0).isReg() ||
      StringRef(Mc.Registers->getName(Call.Inst.getOperand(0).getReg())) !=
          "SGPR30_SGPR31")
    return analysisError(
        "call destination is not ABI return pair SGPR30_SGPR31");

  uint64_t ImmediateTarget = 0;
  if (Mc.Analysis->evaluateBranch(Call.Inst, Call.Address, Call.Size,
                                  ImmediateTarget))
    return ImmediateTarget;
  if (Call.Name == "S_CALL_B64_vi") {
    if (Call.Inst.size() != 2 || !Call.Inst.getOperand(1).isImm())
      return analysisError("malformed immediate S_CALL_B64_vi");
    int64_t Encoded = Call.Inst.getOperand(1).getImm();
    if (Encoded < 0 || Encoded > std::numeric_limits<uint16_t>::max())
      return analysisError("S_CALL_B64_vi displacement is not u16");
    int64_t Displacement = static_cast<int16_t>(Encoded);
    if (Call.Address >
        static_cast<uint64_t>(std::numeric_limits<int64_t>::max()) - Call.Size)
      return analysisError("S_CALL_B64_vi address is outside i64 range");
    int64_t Base = static_cast<int64_t>(Call.Address + Call.Size);
    int64_t Delta = Displacement * 4;
    if ((Delta < 0 && Base < -Delta) ||
        (Delta > 0 && Base > std::numeric_limits<int64_t>::max() - Delta))
      return analysisError("S_CALL_B64_vi target overflows address range");
    return static_cast<uint64_t>(Base + Delta);
  }

  if (Call.Name != "S_SWAPPC_B64_vi" || Call.Inst.size() != 2 ||
      !Call.Inst.getOperand(1).isReg())
    return analysisError(Twine("indirect or unknown call: ") +
                         instructionDescription(Call, Mc));

  unsigned PairRegister = Call.Inst.getOperand(1).getReg();
  auto Pair = splitSgprPair(PairRegister, Mc);
  if (!Pair)
    return Pair.takeError();
  unsigned LowRegister = Pair->first;
  unsigned HighRegister = Pair->second;
  auto LowAtCall =
      reachingDefinitions(Cfg, Instructions, CallIndex, LowRegister, Mc);
  auto HighAtCall =
      reachingDefinitions(Cfg, Instructions, CallIndex, HighRegister, Mc);
  if (!LowAtCall)
    return LowAtCall.takeError();
  if (!HighAtCall)
    return HighAtCall.takeError();
  if (LowAtCall->LiveIn || LowAtCall->Instructions.size() != 1 ||
      HighAtCall->LiveIn || HighAtCall->Instructions.size() != 1)
    return analysisError("direct-call target definitions are ambiguous");
  size_t AddLowIndex = *LowAtCall->Instructions.begin();
  size_t AddHighIndex = *HighAtCall->Instructions.begin();
  const DecodedInstruction *AddLow = &Instructions[AddLowIndex];
  const DecodedInstruction *AddHigh = &Instructions[AddHighIndex];
  if (AddLow->Name != "S_ADD_U32_vi" ||
      !isRegisterOperand(AddLow->Inst, 0, LowRegister) ||
      !isRegisterOperand(AddLow->Inst, 1, LowRegister) ||
      !isImmediateOperand(AddLow->Inst, 2) ||
      AddHigh->Name != "S_ADDC_U32_vi" ||
      !isRegisterOperand(AddHigh->Inst, 0, HighRegister) ||
      !isRegisterOperand(AddHigh->Inst, 1, HighRegister) ||
      !isImmediateOperand(AddHigh->Inst, 2))
    return analysisError(
        "direct-call target has ambiguous or skipped definitions");
  auto LowAtAdd =
      reachingDefinitions(Cfg, Instructions, AddLowIndex, LowRegister, Mc);
  auto HighAtAdd =
      reachingDefinitions(Cfg, Instructions, AddHighIndex, HighRegister, Mc);
  if (!LowAtAdd)
    return LowAtAdd.takeError();
  if (!HighAtAdd)
    return HighAtAdd.takeError();
  if (LowAtAdd->LiveIn || LowAtAdd->Instructions.size() != 1 ||
      HighAtAdd->LiveIn || HighAtAdd->Instructions.size() != 1 ||
      LowAtAdd->Instructions != HighAtAdd->Instructions)
    return analysisError("direct-call GETPC definition is ambiguous");
  size_t GetPcIndex = *LowAtAdd->Instructions.begin();
  const DecodedInstruction *GetPc = &Instructions[GetPcIndex];
  if (GetPc->Name != "S_GETPC_B64_vi" || GetPc->Inst.size() != 1 ||
      !isRegisterOperand(GetPc->Inst, 0, PairRegister))
    return analysisError("direct-call provenance is not exact GETPC");
  if (GetPc->Address + GetPc->Size != AddLow->Address ||
      AddLow->Address + AddLow->Size != AddHigh->Address)
    return analysisError("direct-call carry materialization is not contiguous");
  if (!isUniqueDefinition(*LowAtCall, AddLowIndex) ||
      !isUniqueDefinition(*HighAtCall, AddHighIndex) ||
      !isUniqueDefinition(*LowAtAdd, GetPcIndex) ||
      !isUniqueDefinition(*HighAtAdd, GetPcIndex) ||
      !instructionDominates(Cfg, GetPcIndex, CallIndex) ||
      !instructionDominates(Cfg, AddLowIndex, CallIndex) ||
      !instructionDominates(Cfg, AddHighIndex, CallIndex))
    return analysisError(
        "direct-call target definitions do not uniquely dominate call");

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

  // GFX942 S_GETPC_B64 returns the address of its next instruction.
  return Materialize(GetPc->Address + GetPc->Size);
}

Expected<AnalyzedFunction> analyzeFunction(const SymbolRecord &Function,
                                           ArrayRef<SymbolRecord> Symbols,
                                           bool ReturnPairIsLiveIn,
                                           McState &Mc) {
  auto Decoded = decodeFunction(Function, Mc);
  if (!Decoded)
    return Decoded.takeError();
  auto Cfg = buildFunctionCfg(*Decoded, Mc, Function.Name);
  if (!Cfg)
    return Cfg.takeError();

  AnalyzedFunction Result;
  Result.Evidence = {Function.Name, Function.Address, Function.Size, {}};
  for (size_t Index = 0; Index < Decoded->size(); ++Index) {
    if (!Cfg->Blocks[Cfg->InstructionBlocks[Index]].Reachable)
      continue;
    const DecodedInstruction &Instruction = (*Decoded)[Index];
    const MCInstrDesc &Descriptor =
        Mc.Instructions->get(Instruction.Inst.getOpcode());
    StringRef Name = Instruction.Name;
    if (forbiddenOpcodeFamily(Name))
      return analysisError(Twine("unsupported opcode family ") + Name + " in " +
                           Function.Name);
    auto PhysicalReturn =
        validatePhysicalReturn(*Decoded, Index, *Cfg, ReturnPairIsLiveIn, Mc);
    if (!PhysicalReturn)
      return PhysicalReturn.takeError();
    if (*PhysicalReturn) {
      Result.Effects.push_back(
          {Instruction.Address, PhysicalMachineEffectKind::Return, 0});
      continue;
    }
    if (Descriptor.isIndirectBranch())
      return analysisError(Twine("indirect branch in ") + Function.Name);
    if (Descriptor.isCall()) {
      auto Target = directCallTargets(*Decoded, Index, *Cfg, Mc);
      if (!Target)
        return analysisError(Twine("cannot resolve call in ") + Function.Name +
                             ": " + toString(Target.takeError()));
      const SymbolRecord *Match = nullptr;
      for (const SymbolRecord &Candidate : Symbols) {
        if (Candidate.Address != *Target || Candidate.Type != ELF::STT_FUNC ||
            Candidate.Size == 0 || !Candidate.Text)
          continue;
        if (Match)
          return analysisError(Twine("call target aliases functions in ") +
                               Function.Name);
        Match = &Candidate;
      }
      if (!Match) {
        std::string Detail;
        raw_string_ostream Stream(Detail);
        Stream << "call target is not one exact function in " << Function.Name
               << "; computed=" << *Target << "; functions=";
        for (const SymbolRecord &Candidate : Symbols)
          if (Candidate.Type == ELF::STT_FUNC && Candidate.Size != 0 &&
              Candidate.Text)
            Stream << ' ' << Candidate.Name << '@' << Candidate.Address;
        Stream.flush();
        return analysisError(Detail);
      }
      Result.Evidence.DirectCallees.push_back(Match->Name);
      continue;
    }
    if (Descriptor.isBranch()) {
      if (Descriptor.isBarrier())
        return analysisError(Twine("barrier branch in ") + Function.Name);
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

  auto Loader = buildLoaderView(*Object, Request.Payload);
  if (!Loader)
    return Loader.takeError();
  auto DynamicLoader = validateDynamicLoaderView(*Object, *Loader);
  if (!DynamicLoader)
    return DynamicLoader.takeError();

  auto Metadata = readMetadata(*Object, *Loader);
  if (!Metadata)
    return Metadata.takeError();
  if (Metadata->size() != Request.Entries.size())
    return analysisError("metadata entry set differs from request");
  for (size_t I = 0; I < Metadata->size(); ++I)
    if ((*Metadata)[I].Name != Request.Entries[I].Symbol)
      return analysisError("metadata entry symbol differs from request");

  auto Symbols = readSymbols(*Object, *Loader, *DynamicLoader, *Metadata);
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
  std::set<std::string> KernelEntries;
  for (const auto &Entry : Request.Entries)
    KernelEntries.insert(Entry.Symbol);
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
    auto Analyzed = analyzeFunction(*Function, *Symbols,
                                    !KernelEntries.contains(Name), *Mc);
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
