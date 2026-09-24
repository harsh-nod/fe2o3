// Independent expected typed native rows; not parsed from the emitter text.
// Exact opcode names/effect conventions match the pinned worker's MC decoder.
struct PhysicalRow {
  const char *Opcode; std::vector<std::string> Registers;
  std::vector<uint64_t> Immediates;
  unsigned Definitions, Bytes; std::vector<std::string> Uses, Writes;
  PhysicalMachineMemoryAccess Memory = PhysicalMachineMemoryAccess::None;
  unsigned Width = 0;
  PhysicalMachineBranchKind Branch = PhysicalMachineBranchKind::None;
  int Target = -1;
};
std::vector<PhysicalRow> physicalRows(bool Select) {
  using M = PhysicalMachineMemoryAccess; using B = PhysicalMachineBranchKind;
  std::vector<PhysicalRow> R{
    {"S_LOAD_DWORDX2_IMM_vi",{"SGPR8_SGPR9","SGPR0_SGPR1"},{0,0},1,8,{},{},M::Read,8},
    {"S_LOAD_DWORDX2_IMM_vi",{"SGPR10_SGPR11","SGPR0_SGPR1"},{8,0},1,8,{},{},M::Read,8},
    {"S_LOAD_DWORD_IMM_vi",{"SGPR12","SGPR0_SGPR1"},{16,0},1,8,{},{},M::Read,4},
    {"S_LOAD_DWORD_IMM_vi",{"SGPR13","SGPR0_SGPR1"},{20,0},1,8,{},{},M::Read,4},
    {"S_LOAD_DWORD_IMM_vi",{"SGPR14","SGPR0_SGPR1"},{24,0},1,8,{},{},M::Read,4},
    {"S_LOAD_DWORD_IMM_vi",{"SGPR15","SGPR0_SGPR1"},{28,0},1,8,{},{},M::Read,4},
    {"S_WAITCNT_vi",{},{0xc07f},0,4,{},{}},
    {"S_LSHL_B32_vi",{"SGPR16","SGPR2"},{6},1,4,{},{"SCC"}},
    {"V_ADD_U32_e32_gfx9",{"VGPR2","SGPR16","VGPR0"},{},1,4,{"EXEC"},{}},
    {"V_MOV_B32_e32_vi",{"VGPR3"},{0},1,4,{"EXEC"},{}},
    {"V_MOV_B32_e32_vi",{"VGPR4","SGPR9"},{},1,4,{"EXEC"},{}}
  };
  if (Select) {
    R.push_back({"S_CMP_EQ_U32_vi",{"SGPR15"},{0},0,4,{},{"SCC"}});
    R.push_back({"S_CBRANCH_SCC1_vi",{},{},0,4,{"SCC"},{},M::None,0,B::ConditionalDirect,15});
    R.push_back({"V_MOV_B32_e32_vi",{"VGPR8","SGPR13"},{},1,4,{"EXEC"},{}});
    R.push_back({"S_BRANCH_vi",{},{},0,4,{},{},M::None,0,B::UnconditionalDirect,16});
  }
  R.push_back({"V_MOV_B32_e32_vi",{"VGPR8","SGPR12"},{},1,4,{"EXEC"},{}});
  const std::vector<PhysicalRow> Tail{
    {"V_LSHLREV_B64_vi",{"VGPR6_VGPR7","VGPR2_VGPR3"},{2},1,8,{"EXEC"},{}},
    {"V_ADD_CO_U32_e32_gfx9",{"VGPR6","SGPR8","VGPR6"},{},1,4,{"EXEC"},{"VCC"}},
    {"V_ADDC_CO_U32_e32_gfx9",{"VGPR7","VGPR4","VGPR7"},{},1,4,{"EXEC","VCC"},{"VCC"}},
    {"V_CMP_GT_U64_e32_vi",{"SGPR10_SGPR11","VGPR2_VGPR3"},{},0,4,{"EXEC"},{"VCC"}},
    {"S_AND_SAVEEXEC_B64_vi",{"SGPR18_SGPR19","VCC"},{},1,4,{"EXEC"},{"EXEC","SCC"}},
    {"GLOBAL_STORE_DWORD_vi",{"VGPR6_VGPR7","VGPR8"},{0,0},0,8,{"EXEC"},{},M::Write,4},
    {"S_WAITCNT_vi",{},{0x0f70},0,4,{},{}},
    {"S_MOV_B64_vi",{"EXEC","SGPR18_SGPR19"},{},1,4,{},{}},
    {"S_ENDPGM_vi",{},{0},0,4,{},{},M::None,0,B::Return}
  };
  R.insert(R.end(),Tail.begin(),Tail.end()); return R;
}
#define PE_CHECK(C,M) do { if (!(C)) return std::string(M); } while(false)
std::optional<std::string> physicalRelation(bool Select,
    const PhysicalMachineEffectEvidence &E, ArrayRef<uint8_t> Payload) {
  const auto Rows = physicalRows(Select);
  PE_CHECK(E.PayloadDigest == SHA256::hash(Payload) && E.PayloadBytes == Payload.size(), "physical payload identity");
  PE_CHECK(E.Entries.size() == 1 && E.Functions.size() == 1 &&
      E.Entries[0].Symbol == PhysicalName && E.Functions[0].Symbol == PhysicalName &&
      E.Functions[0].DirectCallees.empty() &&
      E.Functions[0].CodeOffset == E.Entries[0].CodeOffset &&
      E.Functions[0].CodeSize == E.Entries[0].CodeSize, "physical exact entry/no helper closure");
  PE_CHECK(E.Instructions.size() == Rows.size(), "physical complete entry census: no prologue or hidden tail");
  const auto &Entry = E.Entries[0];
  PE_CHECK(Entry.CodeOffset <= Payload.size() && Entry.CodeSize <= Payload.size()-Entry.CodeOffset, "physical entry extent");
  uint64_t Cursor = Entry.CodeOffset;
  for (size_t At=0; At<Rows.size(); ++At) {
    const auto &I=E.Instructions[At]; const auto &R=Rows[At];
    PE_CHECK(I.FunctionSymbol == PhysicalName && I.InstructionOffset == Cursor && I.Encoding.size() == R.Bytes &&
        Cursor <= Payload.size() && I.Encoding.size() <= Payload.size()-Cursor &&
        std::equal(I.Encoding.begin(),I.Encoding.end(),Payload.begin()+Cursor), "physical full-entry encoding extent");
    PE_CHECK(I.Opcode == R.Opcode && I.ExplicitDefinitionCount == R.Definitions &&
        I.ImplicitUses == R.Uses && I.ImplicitDefinitions == R.Writes &&
        I.MemoryAccess == R.Memory && I.MemoryWidth == R.Width && I.BranchKind == R.Branch,
        "physical exact opcode/definitions/implicit state/effect");
    std::vector<std::string> Registers; std::vector<uint64_t> Immediates; std::string OperandOrder;
    for (const auto &O:I.Operands) {
      PE_CHECK(O.TiedTo == -1, "unexpected physical tied operand");
      if (O.Kind == PhysicalMachineOperandKind::Register) { Registers.push_back(O.Register); OperandOrder += 'r'; }
      else if (O.Kind == PhysicalMachineOperandKind::SignedImmediate) { Immediates.push_back(O.Value); OperandOrder += 'i'; }
      else return std::string("unexpected physical operand kind");
    }
    std::string ExpectedOrder(R.Registers.size(), 'r');
    ExpectedOrder += std::string(R.Immediates.size(), 'i');
    if (R.Target >= 0) ExpectedOrder = "i";
    if (StringRef(R.Opcode) == "V_LSHLREV_B64_vi") ExpectedOrder = "rir";
    PE_CHECK(Registers == R.Registers && OperandOrder == ExpectedOrder, "physical register/tuple/operand order");
    if (R.Target >= 0) {
      PE_CHECK(Immediates.size() == 1 && static_cast<size_t>(R.Target)<Rows.size() &&
          I.BranchTarget == E.Instructions[R.Target].InstructionOffset, "physical authored branch destination");
      const uint32_t Prefix=R.Branch==PhysicalMachineBranchKind::ConditionalDirect?0xbf850000U:0xbf820000U;
      PE_CHECK((support::endian::read32le(I.Encoding.data())&0xffff0000U)==Prefix, "physical branch encoding family");
    } else PE_CHECK(Immediates == R.Immediates && I.BranchTarget == 0, "physical immediate operand/stray edge");
    const uint16_t Flags=R.Memory==PhysicalMachineMemoryAccess::Read?1:
        R.Memory==PhysicalMachineMemoryAccess::Write?2:
        R.Branch==PhysicalMachineBranchKind::ConditionalDirect?4:
        R.Branch!=PhysicalMachineBranchKind::None?12:0;
    PE_CHECK(I.Flags == Flags, "physical exact MC effect/control flags");
    Cursor += I.Encoding.size();
  }
  PE_CHECK(Cursor == Entry.CodeOffset+Entry.CodeSize, "physical unaccounted entry suffix");
  const size_t Tail = Select ? 16 : 12;
  PE_CHECK(support::endian::read32le(E.Instructions[6].Encoding.data())==0xbf8cc07fU &&
      support::endian::read32le(E.Instructions[Tail+6].Encoding.data())==0xbf8c0f70U &&
      support::endian::read32le(E.Instructions.back().Encoding.data())==0xbf810000U,
      "physical exact readiness/completion/termination encodings");
  PE_CHECK(support::endian::read64le(E.Instructions[Tail+5].Encoding.data())==0x007f0806dc708000ULL,
      "physical exact global store encoding");
  const std::vector<size_t> Starts=Select?std::vector<size_t>{0,13,15,16}:std::vector<size_t>{0};
  const std::vector<size_t> Counts=Select?std::vector<size_t>{13,2,1,9}:std::vector<size_t>{21};
  const std::vector<std::vector<uint32_t>> Edges=Select?
      std::vector<std::vector<uint32_t>>{{1,2},{3},{3},{}}:std::vector<std::vector<uint32_t>>{{}};
  PE_CHECK(E.Blocks.size()==Starts.size(), "physical CFG block census");
  for (size_t B=0;B<Starts.size();++B) {
    const auto &Block=E.Blocks[B];
    PE_CHECK(Block.FunctionSymbol==PhysicalName && Block.Ordinal==B && Block.InstructionCount==Counts[B] &&
        Block.FirstInstructionOffset==E.Instructions[Starts[B]].InstructionOffset &&
        Block.Successors==Edges[B], "physical exact CFG membership/successors");
    for (size_t I=0;I<Counts[B];++I)
      PE_CHECK(E.Instructions[Starts[B]+I].BlockOrdinal==B, "physical CFG instruction membership");
  }
  return std::nullopt;
}
#undef PE_CHECK
