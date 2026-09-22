// Independent low-register literals. GFX9 VOP2 fields: opcode base | dst<<17 |
// src1<<9 | (256+src0). These are not generated from source descriptors or MC.
struct ExpectedStep {
  const char *Opcode; std::array<const char *, 3> Registers; uint32_t Word;
};
constexpr std::array<ExpectedStep, 3> DefaultSteps{{
    {"V_XOR_B32_e32_vi", {"VGPR4", "VGPR0", "VGPR1"}, 0x2a080300},
    {"V_AND_B32_e32_vi", {"VGPR4", "VGPR4", "VGPR2"}, 0x26080504},
    {"V_XOR_B32_e32_vi", {"VGPR5", "VGPR1", "VGPR4"}, 0x2a0a0901},
}};
static_assert((0x2a000000 | (4 << 17) | (1 << 9) | 256) == 0x2a080300);
static_assert((0x26000000 | (4 << 17) | (2 << 9) | 260) == 0x26080504);
static_assert((0x2a000000 | (5 << 17) | (4 << 9) | 257) == 0x2a0a0901);

bool matchesDefault(const PhysicalMachineInstructionTrace &Site,
                    const ExpectedStep &Expected, ArrayRef<uint8_t> Payload) {
  if (Site.FunctionSymbol != KernelName || Site.Opcode != Expected.Opcode ||
      Site.Encoding.size() != 4 || Site.ExplicitDefinitionCount != 1 ||
      Site.Operands.size() != 3 || !Site.ImplicitDefinitions.empty() ||
      Site.ImplicitUses != std::vector<std::string>{"EXEC"} ||
      Site.MemoryAccess != PhysicalMachineMemoryAccess::None ||
      Site.MemoryWidth != 0 || (Site.Flags & ~uint16_t(1 << 4)) != 0 ||
      Site.BranchKind != PhysicalMachineBranchKind::None || Site.BranchTarget != 0 ||
      Site.InstructionOffset > Payload.size() ||
      Payload.size() - Site.InstructionOffset < 4) return false;
  for (size_t Index = 0; Index < 3; ++Index) {
    const auto &Operand = Site.Operands[Index];
    if (Operand.Kind != PhysicalMachineOperandKind::Register ||
        Operand.TiedTo != -1 || Operand.Register != Expected.Registers[Index])
      return false;
  }
  const auto Raw = Payload.slice(Site.InstructionOffset, 4);
  return std::equal(Raw.begin(), Raw.end(), Site.Encoding.begin()) &&
         support::endian::read32le(Raw.data()) == Expected.Word;
}
size_t locateDefault(const BuiltFixture &Built) {
  const auto &Evidence = Built.Evidence;
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  require(Evidence.Instructions.size() >= 3 &&
              Evidence.Instructions.size() <= InstructionLimit &&
              Evidence.Entries.size() == 1 && Evidence.Entries[0].Symbol == KernelName &&
              Evidence.PayloadBytes == Payload.size() &&
              Evidence.PayloadDigest == SHA256::hash(Payload), "exact decoded payload join");
  const auto &Entry = Evidence.Entries[0];
  require(Entry.CodeOffset <= Payload.size() &&
              Entry.CodeSize <= Payload.size() - Entry.CodeOffset, "entry bounds");
  std::optional<size_t> Found;
  for (size_t Index = 0; Index + 2 < Evidence.Instructions.size(); ++Index) {
    const auto &First = Evidence.Instructions[Index]; bool Match = true;
    for (size_t Offset = 0; Offset < 3; ++Offset) {
      const auto &Site = Evidence.Instructions[Index + Offset];
      Match &= matchesDefault(Site, DefaultSteps[Offset], Payload) &&
          Site.BlockOrdinal == First.BlockOrdinal &&
          Site.InstructionOffset == First.InstructionOffset + 4 * Offset &&
          Site.InstructionOffset >= Entry.CodeOffset &&
          Site.InstructionOffset - Entry.CodeOffset <= Entry.CodeSize &&
          Entry.CodeSize - (Site.InstructionOffset - Entry.CodeOffset) >= 4;
    }
    if (!Match) continue;
    require(!Found, "ambiguous default sequence"); Found = Index;
  }
  require(Found.has_value(), "default e32 sequence absent"); return *Found;
}
json::Object siteJson(const PhysicalMachineInstructionTrace &Site) {
  json::Array Registers, Uses, Definitions;
  for (const auto &Operand : Site.Operands)
    if (Operand.Kind == PhysicalMachineOperandKind::Register)
      Registers.emplace_back(Operand.Register);
  for (const auto &Use : Site.ImplicitUses) Uses.emplace_back(Use);
  for (const auto &Definition : Site.ImplicitDefinitions) Definitions.emplace_back(Definition);
  return json::Object{{"file_offset", Site.InstructionOffset}, {"opcode", Site.Opcode},
      {"bytes_hex", hex(Site.Encoding)}, {"mc_flags", Site.Flags},
      {"register_operands", std::move(Registers)}, {"implicit_reads", std::move(Uses)},
      {"implicit_writes", std::move(Definitions)}};
}
json::Object defaultDescriptor(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  StringRef Data(reinterpret_cast<const char *>(Payload.data()), Payload.size());
  auto Object = unwrap(object::ObjectFile::createObjectFile(
      MemoryBufferRef(Data, "<default-final-descriptor>")));
  std::optional<json::Object> Result; size_t Count = 0;
  for (const auto &Candidate : Object->symbols()) {
    require(++Count <= 4096, "descriptor symbol scan bound");
    if (unwrap(Candidate.getName()) != KernelName.str() + ".kd") continue;
    require(!Result, "duplicate default descriptor");
    auto Section = unwrap(Candidate.getSection());
    require(Section != Object->section_end(), "descriptor section absent");
    auto Contents = unwrap(Section->getContents());
    const uint64_t Address = unwrap(Candidate.getAddress());
    require(Address >= Section->getAddress(), "descriptor precedes section");
    const uint64_t Relative = Address - Section->getAddress();
    require(Relative <= Contents.size() && Contents.size() - Relative >= 64,
            "descriptor section extent");
    const auto *Bytes = reinterpret_cast<const uint8_t *>(Contents.data()) + Relative;
    const uintptr_t Base = reinterpret_cast<uintptr_t>(Payload.data());
    const uintptr_t Begin = reinterpret_cast<uintptr_t>(Bytes);
    require(Begin >= Base && Begin - Base <= Payload.size() &&
                Payload.size() - (Begin - Base) >= 64, "descriptor exact payload storage");
    const auto Digest = SHA256::hash(ArrayRef<uint8_t>(Bytes, 64));
    require(Digest == Built.Evidence.Entries[0].DescriptorIdentity,
            "MC/independent descriptor digest mismatch");
    const uint32_t Rsrc1 = support::endian::read32le(Bytes + 48);
    const uint32_t Rsrc3 = support::endian::read32le(Bytes + 44);
    const uint32_t Capacity = ((Rsrc1 & 63) + 1) * 8;
    const uint32_t Boundary = ((Rsrc3 & 63) + 1) * 4;
    require(Capacity >= Boundary && Boundary >= 6,
            "descriptor cannot cover authored v0..v5 high-water");
    Result = json::Object{{"file_offset", Begin - Base}, {"bytes", 64},
        {"sha256", hex(Digest)}, {"compute_pgm_rsrc1", Rsrc1},
        {"compute_pgm_rsrc3", Rsrc3}, {"vgpr_capacity", Capacity},
        {"architected_vgpr_boundary", Boundary}, {"required_footprint_high_water", 6},
        {"interpretation", "encoded-capacity-not-metadata-usage-or-lifetime"}};
  }
  require(Result.has_value(), "default descriptor absent"); return std::move(*Result);
}
json::Object describeDefault(const BuiltFixture &Built, OptimizationLevel Level) {
  const size_t Position = locateDefault(Built);
  json::Array Program, Diagnostics;
  for (size_t Offset = 0; Offset < 3; ++Offset)
    Program.emplace_back(siteJson(Built.Evidence.Instructions[Position + Offset]));
  for (const auto &Diagnostic : Built.InspectionDiagnostics) Diagnostics.emplace_back(Diagnostic);
  // Unlike the high-register observer, no lexical boundary-use scan is
  // promoted into a low-register value/lifetime assertion. v0 is also a real
  // ABI live-in, and pairs/tuples/reuse require the machine owner's analysis.
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
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
