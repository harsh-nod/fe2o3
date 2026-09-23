// Count-sized matcher and descriptor observer, not a second core decoder.
// Literals independently follow existing GFX9 VOP1/VOP2 field definitions.
// MOV: 7e000200 | 33<<17 | (256+34); ADD: 68000000 |33<<17|35<<9|(256+33).
struct RepeatStep {
  const char *Opcode; std::array<const char *, 3> Registers; size_t Arity; uint32_t Word;
};
constexpr RepeatStep RepeatMov{"V_MOV_B32_e32_vi", {"VGPR33", "VGPR34", ""}, 2, 0x7e420322};
constexpr RepeatStep RepeatAdd{"V_ADD_U32_e32_gfx9", {"VGPR33", "VGPR33", "VGPR35"}, 3, 0x68424721};
constexpr RepeatStep RepeatSub{"V_SUB_U32_e32_gfx9", {"VGPR33", "VGPR33", "VGPR35"}, 3, 0x6a424721};
static_assert((0x7e000200u | (33u << 17) | 290u) == RepeatMov.Word);
static_assert((0x68000000u | (33u << 17) | (35u << 9) | 289u) == RepeatAdd.Word);
bool repeatSiteMatches(const PhysicalMachineInstructionTrace &Site,
                       const RepeatStep &Expected, ArrayRef<uint8_t> Payload) {
  if (Site.FunctionSymbol != KernelName || Site.Opcode != Expected.Opcode ||
      Site.Encoding.size() != 4 || Site.ExplicitDefinitionCount != 1 ||
      Site.Operands.size() != Expected.Arity || !Site.ImplicitDefinitions.empty() ||
      Site.ImplicitUses != std::vector<std::string>{"EXEC"} ||
      Site.MemoryAccess != PhysicalMachineMemoryAccess::None || Site.MemoryWidth != 0 ||
      (Site.Flags & ~uint16_t(1 << 4)) != 0 ||
      Site.BranchKind != PhysicalMachineBranchKind::None || Site.BranchTarget != 0 ||
      Site.InstructionOffset > Payload.size() || Payload.size() - Site.InstructionOffset < 4)
    return false;
  for (size_t Index = 0; Index < Expected.Arity; ++Index) {
    const auto &Operand = Site.Operands[Index];
    if (Operand.Kind != PhysicalMachineOperandKind::Register || Operand.TiedTo != -1 ||
        Operand.Register != Expected.Registers[Index]) return false;
  }
  const auto Raw = Payload.slice(Site.InstructionOffset, 4);
  return std::equal(Raw.begin(), Raw.end(), Site.Encoding.begin()) &&
      support::endian::read32le(Raw.data()) == Expected.Word;
}
std::optional<size_t> repeatPosition(const PhysicalMachineEffectEvidence &Evidence,
                                    ArrayRef<uint8_t> Payload, unsigned Count) {
  const size_t Length = static_cast<size_t>(Count) + 1;
  if (!validRepeatCount(Count) || Payload.empty() || Payload.size() > PayloadByteLimit ||
      Evidence.Instructions.size() < Length || Evidence.Instructions.size() > InstructionLimit ||
      Evidence.Entries.size() != 1 || Evidence.Entries[0].Symbol != KernelName ||
      Evidence.PayloadBytes != Payload.size() || Evidence.PayloadDigest != SHA256::hash(Payload))
    return std::nullopt;
  const auto &Entry = Evidence.Entries[0];
  if (Entry.CodeOffset > Payload.size() || Entry.CodeSize > Payload.size() - Entry.CodeOffset)
    return std::nullopt;
  std::optional<size_t> Found;
  for (size_t Index = 0; Index + Length <= Evidence.Instructions.size(); ++Index) {
    const auto &First = Evidence.Instructions[Index]; bool Match = true;
    for (size_t Offset = 0; Offset < Length; ++Offset) {
      const auto &Site = Evidence.Instructions[Index + Offset];
      Match &= repeatSiteMatches(Site, Offset == 0 ? RepeatMov : RepeatAdd, Payload) &&
          Site.BlockOrdinal == First.BlockOrdinal &&
          Site.InstructionOffset == First.InstructionOffset + 4 * Offset &&
          Site.InstructionOffset >= Entry.CodeOffset &&
          Site.InstructionOffset - Entry.CodeOffset <= Entry.CodeSize &&
          Entry.CodeSize - (Site.InstructionOffset - Entry.CodeOffset) >= 4;
    }
    // A shorter prefix of a longer run is not the declared repeat count.
    if (Match && Index + Length < Evidence.Instructions.size()) {
      const auto &Next = Evidence.Instructions[Index + Length];
      if (Next.BlockOrdinal == First.BlockOrdinal &&
          Next.InstructionOffset == First.InstructionOffset + 4 * Length &&
          repeatSiteMatches(Next, RepeatAdd, Payload)) Match = false;
    }
    if (Match) { if (Found) return std::nullopt; Found = Index; }
  }
  return Found;
}
json::Object repeatSiteJson(const PhysicalMachineInstructionTrace &Site) {
  json::Array Registers, Uses, Definitions;
  for (const auto &Operand : Site.Operands) Registers.emplace_back(Operand.Register);
  for (const auto &Use : Site.ImplicitUses) Uses.emplace_back(Use);
  for (const auto &Definition : Site.ImplicitDefinitions) Definitions.emplace_back(Definition);
  return json::Object{{"file_offset", Site.InstructionOffset}, {"opcode", Site.Opcode},
      {"bytes_hex", hex(Site.Encoding)}, {"mc_flags", Site.Flags},
      {"register_operands", std::move(Registers)}, {"implicit_reads", std::move(Uses)},
      {"implicit_writes", std::move(Definitions)}};
}
bool repeatCapacity(uint32_t Rsrc1, uint32_t Rsrc3) {
  const uint32_t Capacity = ((Rsrc1 & 63) + 1) * 8;
  const uint32_t Boundary = ((Rsrc3 & 63) + 1) * 4;
  return Capacity >= Boundary && Boundary >= RequiredBindingExtent;
}
json::Object repeatDescriptor(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  StringRef Data(reinterpret_cast<const char *>(Payload.data()), Payload.size());
  auto Object = unwrap(object::ObjectFile::createObjectFile(MemoryBufferRef(Data, "<repeat-final-descriptor>")));
  std::optional<json::Object> Result; size_t Count = 0;
  for (const auto &Candidate : Object->symbols()) {
    require(++Count <= 4096, "bounded descriptor symbol scan");
    if (unwrap(Candidate.getName()) != KernelName.str() + ".kd") continue;
    require(!Result, "duplicate repeat descriptor");
    auto Section = unwrap(Candidate.getSection());
    require(Section != Object->section_end(), "descriptor section absent");
    auto Contents = unwrap(Section->getContents());
    const uint64_t Address = unwrap(Candidate.getAddress());
    require(Address >= Section->getAddress(), "descriptor precedes section");
    const uint64_t Relative = Address - Section->getAddress();
    require(Relative <= Contents.size() && Contents.size() - Relative >= 64, "descriptor section extent");
    const auto *Bytes = reinterpret_cast<const uint8_t *>(Contents.data()) + Relative;
    const uintptr_t Base = reinterpret_cast<uintptr_t>(Payload.data()), Begin = reinterpret_cast<uintptr_t>(Bytes);
    require(Begin >= Base && Begin - Base <= Payload.size() && Payload.size() - (Begin - Base) >= 64,
        "descriptor must reference exact full payload file storage");
    const auto Digest = SHA256::hash(ArrayRef<uint8_t>(Bytes, 64));
    require(Digest == Built.Evidence.Entries[0].DescriptorIdentity, "decoder/independent descriptor digest mismatch");
    const uint32_t Rsrc1 = support::endian::read32le(Bytes + 48), Rsrc3 = support::endian::read32le(Bytes + 44);
    require(repeatCapacity(Rsrc1, Rsrc3), "final encoded descriptor cannot cover declared v0..v36 extent");
    Result = json::Object{{"file_offset", Begin - Base}, {"bytes", 64}, {"sha256", hex(Digest)},
        {"compute_pgm_rsrc1", Rsrc1}, {"compute_pgm_rsrc3", Rsrc3},
        {"vgpr_capacity", ((Rsrc1 & 63) + 1) * 8}, {"architected_vgpr_boundary", ((Rsrc3 & 63) + 1) * 4},
        {"required_footprint_high_water", RequiredBindingExtent},
        {"interpretation", "encoded-capacity-not-metadata-usage-or-lifetime"}};
  }
  require(Result.has_value(), "repeat descriptor absent"); return std::move(*Result);
}
json::Object describeRepeat(const BuiltFixture &Built, OptimizationLevel Level, unsigned Count) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto Position = repeatPosition(Built.Evidence, Payload, Count);
  require(Position.has_value(), "exact count-sized contiguous MOV/ADD sequence absent or ambiguous");
  json::Array Program, Diagnostics;
  for (unsigned Offset = 0; Offset <= Count; ++Offset)
    Program.emplace_back(repeatSiteJson(Built.Evidence.Instructions[*Position + Offset]));
  for (const auto &Diagnostic : Built.InspectionDiagnostics) Diagnostics.emplace_back(Diagnostic);
  return json::Object{{"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
      {"repetitions", Count}, {"unique_sequence_matches", 1},
      {"llvm_sha256", hex(Built.RequestValue.CompilerModule.Digest)},
      {"llvm_bytes", Built.RequestValue.CompilerModule.Bytes.size()},
      {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)}, {"hsaco_bytes", Payload.size()},
      {"entry_file_offset", Built.Evidence.Entries[0].CodeOffset}, {"entry_code_bytes", Built.Evidence.Entries[0].CodeSize},
      {"static_instruction_count", Built.Evidence.Instructions.size()}, {"program", std::move(Program)},
      {"descriptor", repeatDescriptor(Built)}, {"post_link_checks", std::move(Diagnostics)},
      {"derivation_identity", hex(Built.ResponseValue.Derivation->EvidenceIdentity)},
      {"boundary_value_or_lifetime_proof", false}};
}
json::Object repeatMachineControls(const BuiltFixture &Built, unsigned Count) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto Original = SHA256::hash(Payload);
  const auto Position = repeatPosition(Built.Evidence, Payload, Count);
  require(Position.has_value(), "actual repeat machine-control baseline");
  size_t Decoded = 0;
  for (size_t Offset : {size_t(0), static_cast<size_t>(Count)}) {
    const auto &Site = Built.Evidence.Instructions[*Position + Offset];
    const auto &Expected = Offset == 0 ? RepeatMov : RepeatAdd;
    for (unsigned Kind = 0; Kind < 14; ++Kind) {
      auto Changed = Site;
      switch (Kind) {
      case 0: Changed.Opcode = Offset == 0 ? "V_MOV_B32_e64_vi" : "V_ADD_U32_e64_gfx9"; break;
      case 1: Changed.Operands[0].Register = "VGPR37"; break;
      case 2: Changed.Operands.push_back(Changed.Operands.back()); break;
      case 3: Changed.Operands[1].Kind = PhysicalMachineOperandKind::SignedImmediate; break;
      case 4: Changed.Operands[1].TiedTo = 0; break;
      case 5: Changed.Encoding.resize(8, 0); break;
      case 6: Changed.ImplicitUses.clear(); break;
      case 7: Changed.ImplicitDefinitions.push_back("VCC"); break;
      case 8: Changed.MemoryAccess = PhysicalMachineMemoryAccess::Write; break;
      case 9: Changed.MemoryWidth = 4; break;
      case 10: Changed.ExplicitDefinitionCount = 2; break;
      case 11: Changed.Flags |= 1; break;
      case 12: Changed.BranchKind = PhysicalMachineBranchKind::ConditionalDirect; break;
      case 13: Changed.FunctionSymbol = "other"; break;
      }
      require(!repeatSiteMatches(Changed, Expected, Payload), "mutated MOV/ADD field accepted"); ++Decoded;
    }
  }
  auto Stale = Built.Evidence; Stale.PayloadDigest[0] ^= 1;
  require(!repeatPosition(Stale, Payload, Count), "stale decoded payload identity accepted");
  // Synthetic views reuse actual decoded sites/words, not executable ELF files.
  auto View = [&](unsigned Copies, bool Gap, bool ExtraAdd) {
    auto Evidence = Built.Evidence; Evidence.Instructions.clear();
    const size_t Length = static_cast<size_t>(Count) + 1;
    std::vector<uint8_t> Bytes(4 * (Copies * Length + (Gap ? 1 : 0) + (ExtraAdd ? 1 : 0)), 0);
    size_t Word = 0;
    for (unsigned Copy = 0; Copy < Copies; ++Copy)
      for (size_t Index = 0; Index < Length; ++Index) {
        if (Gap && Copy == 0 && Index == 1) ++Word;
        auto Site = Built.Evidence.Instructions[*Position + Index];
        Site.InstructionOffset = 4 * Word++;
        std::copy(Site.Encoding.begin(), Site.Encoding.end(), Bytes.begin() + Site.InstructionOffset);
        require(repeatSiteMatches(Site, Index == 0 ? RepeatMov : RepeatAdd, Bytes),
            "synthetic view must preserve each exact instruction/raw join");
        Evidence.Instructions.push_back(std::move(Site));
      }
    if (ExtraAdd) {
      auto Site = Built.Evidence.Instructions[*Position + Count];
      Site.InstructionOffset = 4 * Word++;
      std::copy(Site.Encoding.begin(), Site.Encoding.end(), Bytes.begin() + Site.InstructionOffset);
      require(repeatSiteMatches(Site, RepeatAdd, Bytes), "extra ADD must individually match");
      Evidence.Instructions.push_back(std::move(Site));
    }
    Evidence.Entries[0].CodeOffset = 0; Evidence.Entries[0].CodeSize = Bytes.size();
    Evidence.PayloadBytes = Bytes.size(); Evidence.PayloadDigest = SHA256::hash(Bytes);
    return std::make_pair(std::move(Evidence), std::move(Bytes));
  };
  auto Baseline = View(1, false, false);
  require(repeatPosition(Baseline.first, Baseline.second, Count).has_value(), "synthetic isolated view positive");
  auto Gap = View(1, true, false);
  require(!repeatPosition(Gap.first, Gap.second, Count), "gapped individually matching sequence accepted");
  auto Duplicate = View(2, false, false);
  require(!repeatPosition(Duplicate.first, Duplicate.second, Count), "two complete identical sequences accepted");
  auto Extra = View(1, false, true);
  require(!repeatPosition(Extra.first, Extra.second, Count), "longer ADD run accepted as shorter declared count");
  const unsigned Wrong = Count == 1 ? 2 : 1;
  require(!repeatPosition(Baseline.first, Baseline.second, Wrong), "wrong closed repetition count accepted");
  const auto &Last = Built.Evidence.Instructions[*Position + Count];
  auto ChangedPayload = Payload;
  require(Last.InstructionOffset <= ChangedPayload.size() && ChangedPayload.size() - Last.InstructionOffset >= 4,
      "last ADD mutation bound");
  ChangedPayload[Last.InstructionOffset + 3] ^= 2; // ADD -> SUB, same register fields.
  require(!repeatSiteMatches(Last, RepeatAdd, ChangedPayload), "old trace accepted different raw bytes");
  const auto ChangedEvidence = analyzePayload(ChangedPayload);
  require(!repeatPosition(ChangedEvidence, ChangedPayload, Count), "redecoded SUB accepted as ADD repeat");
  size_t Opposite = 0;
  for (const auto &Site : ChangedEvidence.Instructions)
    if (Site.InstructionOffset == Last.InstructionOffset && repeatSiteMatches(Site, RepeatSub, ChangedPayload)) ++Opposite;
  require(Opposite == 1, "negative must decode one exact SUB, not merely fail decoding");
  // Isolate descriptor capacity guards with disclosed synthetic register fields.
  require(repeatCapacity(4, 9), "40/40 descriptor synthetic positive");
  require(!repeatCapacity(4, 8), "36-register boundary must not cover declared37");
  require(!repeatCapacity(3, 9), "capacity32 must not cover boundary40");
  require(!repeatCapacity(0, 0), "old tiny descriptor must not cover repeat extent");
  require(SHA256::hash(Payload) == Original && repeatPosition(Built.Evidence, Payload, Count).has_value(),
      "machine controls mutated original linked payload");
  return json::Object{{"decoded_field_refusals", Decoded}, {"stale_identity_refusals", 1},
      {"synthetic_view_positives", 1}, {"gapped_sequence_refusals", 1}, {"duplicate_sequence_refusals", 1},
      {"extra_add_refusals", 1}, {"wrong_count_refusals", 1}, {"raw_byte_mismatch_refusals", 1},
      {"redecoded_opposite_opcode_refusals", 1}, {"opposite_opcode_mutated_payload_observed", true},
      {"descriptor_capacity_positives", 1}, {"descriptor_capacity_refusals", 3},
      {"original_payload_unchanged", true}, {"captured_llvm_mutated", false},
      {"mutated_payload_executed_on_hardware", false}};
}
