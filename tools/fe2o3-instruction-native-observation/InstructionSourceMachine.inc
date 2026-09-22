// OR differs only in the independent VOP2 opcode base, never derived from
// caller JSON, a Rust descriptor decoder, assembler output or machine trace.
constexpr std::array<ExpectedStep, 3> EditedSteps{{
    {"V_XOR_B32_e32_vi", {"VGPR4", "VGPR0", "VGPR1"}, 0x2a080300},
    {"V_AND_B32_e32_vi", {"VGPR4", "VGPR4", "VGPR2"}, 0x26080504},
    {"V_OR_B32_e32_vi", {"VGPR5", "VGPR1", "VGPR4"}, 0x280a0901},
}};
static_assert((0x28000000 | (5 << 17) | (4 << 9) | 257) == 0x280a0901);
const std::array<ExpectedStep, 3> &instructionSteps(InstructionProfile Profile) {
  return Profile == InstructionProfile::Default ? DefaultSteps : EditedSteps;
}

std::optional<size_t> instructionPosition(const PhysicalMachineEffectEvidence &Evidence,
    ArrayRef<uint8_t> Payload, InstructionProfile Profile) {
  if (Payload.empty() || Payload.size() > PayloadByteLimit ||
      Evidence.Instructions.size() < 3 || Evidence.Instructions.size() > InstructionLimit ||
      Evidence.Entries.size() != 1 || Evidence.Entries[0].Symbol != KernelName ||
      Evidence.PayloadBytes != Payload.size() || Evidence.PayloadDigest != SHA256::hash(Payload))
    return std::nullopt;
  const auto &Entry = Evidence.Entries[0];
  if (Entry.CodeOffset > Payload.size() || Entry.CodeSize > Payload.size() - Entry.CodeOffset)
    return std::nullopt;
  std::optional<size_t> Found;
  const auto &Steps = instructionSteps(Profile);
  for (size_t Index = 0; Index + 2 < Evidence.Instructions.size(); ++Index) {
    const auto &First = Evidence.Instructions[Index]; bool Match = true;
    for (size_t Offset = 0; Offset < 3; ++Offset) {
      const auto &Site = Evidence.Instructions[Index + Offset];
      Match &= matchesDefault(Site, Steps[Offset], Payload) &&
          Site.BlockOrdinal == First.BlockOrdinal &&
          Site.InstructionOffset == First.InstructionOffset + 4 * Offset &&
          Site.InstructionOffset >= Entry.CodeOffset &&
          Site.InstructionOffset - Entry.CodeOffset <= Entry.CodeSize &&
          Entry.CodeSize - (Site.InstructionOffset - Entry.CodeOffset) >= 4;
    }
    if (Match) {
      if (Found) return std::nullopt;
      Found = Index;
    }
  }
  return Found;
}

json::Object describeInstruction(const BuiltFixture &Built, OptimizationLevel Level,
                                InstructionProfile Profile) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto Position = instructionPosition(Built.Evidence, Payload, Profile);
  require(Position.has_value(), "exact selected XOR/AND/XOR-or-OR sequence absent or ambiguous");
  if (Profile == InstructionProfile::Default) return describeDefault(Built, Level);
  json::Array Program, Diagnostics;
  for (size_t Index = 0; Index < 3; ++Index)
    Program.emplace_back(siteJson(Built.Evidence.Instructions[*Position + Index]));
  for (const auto &Diagnostic : Built.InspectionDiagnostics) Diagnostics.emplace_back(Diagnostic);
  // Same bounded low-machine case schema; OR facts are explicit, not accepted
  // by weakening the unchanged legacy XOR-only matcher.
  return json::Object{{"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
      {"llvm_sha256", hex(Built.RequestValue.CompilerModule.Digest)},
      {"llvm_bytes", Built.RequestValue.CompilerModule.Bytes.size()},
      {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
      {"hsaco_bytes", Payload.size()},
      {"entry_file_offset", Built.Evidence.Entries[0].CodeOffset},
      {"entry_code_bytes", Built.Evidence.Entries[0].CodeSize},
      {"static_instruction_count", Built.Evidence.Instructions.size()},
      {"program", std::move(Program)}, {"descriptor", defaultDescriptor(Built)},
      {"post_link_checks", std::move(Diagnostics)},
      {"derivation_identity", hex(Built.ResponseValue.Derivation->EvidenceIdentity)},
      {"boundary_value_or_lifetime_proof", false}};
}

json::Object instructionMachineControls(const BuiltFixture &Built,
                                       InstructionProfile Profile) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto OriginalDigest = SHA256::hash(Payload);
  const auto Position = instructionPosition(Built.Evidence, Payload, Profile);
  require(Position.has_value(), "actual native mutation baseline");
  const auto &Last = Built.Evidence.Instructions[*Position + 2];
  const auto &Expected = instructionSteps(Profile)[2];
  size_t Decoded = 0;
  for (size_t Kind = 0; Kind < 7; ++Kind) {
    auto Changed = Last;
    switch (Kind) {
    case 0: Changed.Opcode = Profile == InstructionProfile::Default
        ? "V_OR_B32_e32_vi" : "V_XOR_B32_e32_vi"; break;
    case 1: Changed.Operands[0].Register = "VGPR6"; break;
    case 2: Changed.Encoding.push_back(0); break;
    case 3: Changed.ImplicitUses.clear(); break;
    case 4: Changed.ImplicitDefinitions.push_back("VCC"); break;
    case 5: Changed.MemoryAccess = PhysicalMachineMemoryAccess::Write; break;
    case 6: Changed.Operands[1].TiedTo = 0; break;
    }
    require(!matchesDefault(Changed, Expected, Payload), "mutated decoded instruction accepted");
    ++Decoded;
  }
  auto Stale = Built.Evidence;
  Stale.PayloadDigest[0] ^= 1;
  require(!instructionPosition(Stale, Payload, Profile), "stale payload evidence accepted");
  auto Gap = Built.Evidence;
  // Explicitly synthetic bounded matcher view, not an ELF or decoded artifact:
  // every actual encoding/operand set must match its own raw bytes, while only
  // the required contiguous-three offsets fail. Do not conflate a stale byte
  // offset with this isolated sequence-contiguity control.
  Gap.Instructions.clear();
  std::vector<uint8_t> GapPayload(16, 0);
  const std::array<size_t, 3> GapOffsets{{0, 8, 12}};
  for (size_t Index = 0; Index < 3; ++Index) {
    auto Site = Built.Evidence.Instructions[*Position + Index];
    Site.InstructionOffset = GapOffsets[Index];
    require(Site.Encoding.size() == 4, "bounded synthetic gap encoding");
    std::copy(Site.Encoding.begin(), Site.Encoding.end(),
        GapPayload.begin() + GapOffsets[Index]);
    require(matchesDefault(Site, instructionSteps(Profile)[Index], GapPayload),
        "synthetic gap sites must individually match exact raw bytes");
    Gap.Instructions.push_back(std::move(Site));
  }
  Gap.Entries[0].CodeOffset = 0;
  Gap.Entries[0].CodeSize = GapPayload.size();
  Gap.PayloadBytes = GapPayload.size();
  Gap.PayloadDigest = SHA256::hash(GapPayload);
  require(!instructionPosition(Gap, GapPayload, Profile),
      "individually valid but gapped synthetic sequence accepted");
  auto ChangedPayload = Payload;
  require(Last.InstructionOffset <= ChangedPayload.size() &&
      ChangedPayload.size() - Last.InstructionOffset >= 4, "last instruction mutation bounds");
  ChangedPayload[Last.InstructionOffset + 3] ^= 2; // Exact XOR<->OR base change.
  require(SHA256::hash(ChangedPayload) != OriginalDigest, "payload mutation must change identity");
  require(!matchesDefault(Last, Expected, ChangedPayload), "trace accepted different raw bytes");
  const auto ChangedEvidence = analyzePayload(ChangedPayload);
  require(!instructionPosition(ChangedEvidence, ChangedPayload, Profile),
      "redecoded opposite opcode accepted for selected profile");
  const auto Opposite = Profile == InstructionProfile::Default
      ? InstructionProfile::Edited : InstructionProfile::Default;
  require(instructionPosition(ChangedEvidence, ChangedPayload, Opposite).has_value(),
      "payload negative must be the actual opposite closed opcode, not decoder failure");
  require(SHA256::hash(Payload) == OriginalDigest &&
      instructionPosition(Built.Evidence, Payload, Profile).has_value(),
      "mutation controls altered original linked payload");
  return json::Object{{"decoded_field_refusals", Decoded},
      {"stale_identity_refusals", 1}, {"gapped_sequence_refusals", 1},
      {"raw_byte_mismatch_refusals", 1}, {"redecoded_opposite_opcode_refusals", 1},
      {"opposite_profile_mutated_payload_observed", true},
      {"original_payload_unchanged", true}, {"captured_llvm_mutated", false},
      {"mutated_payload_executed_on_hardware", false}};
}
