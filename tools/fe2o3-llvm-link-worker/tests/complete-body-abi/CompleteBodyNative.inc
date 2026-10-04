// Uses only the unchanged worker's complete decoded instruction/CFG evidence.
struct CompleteLayout {
  size_t Start=0, Tail=0;
  std::vector<size_t> BlockStarts, Arithmetic, Branches;
};
std::vector<std::string> completeRegisters(const PhysicalMachineInstructionTrace &I) {
  std::vector<std::string> R;
  for(const auto &O:I.Operands) if(O.Kind==PhysicalMachineOperandKind::Register) R.push_back(O.Register);
  return R;
}
bool opcodeVariant(StringRef Actual,StringRef Base) {
  return Actual==Base || Actual==(Base.str()+"_vi");
}
#define CB_CHECK(C,M) do { if(!(C)) return std::string(M); } while(false)
std::optional<std::string> completeRelation(const CompletePlan &P,
    const PhysicalMachineEffectEvidence &E,ArrayRef<uint8_t> Payload,StringRef Symbol,
    CompleteLayout *Found=nullptr) {
  CB_CHECK(E.PayloadDigest==SHA256::hash(Payload) && E.PayloadBytes==Payload.size(),
      "payload identity");
  CB_CHECK(E.Entries.size()==1 && E.Functions.size()==1 && E.Entries[0].Symbol==Symbol &&
      E.Functions[0].Symbol==Symbol && E.Functions[0].DirectCallees.empty(),"entry closure");
  CB_CHECK(!E.Instructions.empty() && E.Instructions.size()<=InstructionLimit &&
      !E.Blocks.empty() && E.Blocks.size()<=128,"decoded bounds");
  const auto &Entry=E.Entries[0];
  CB_CHECK(Entry.CodeOffset<=Payload.size() && Entry.CodeSize<=Payload.size()-Entry.CodeOffset,
      "entry range");
  size_t Count=9;
  CompleteLayout L;
  for(const auto &B:P.Blocks) {
    L.BlockStarts.push_back(Count-9);
    Count += B.Steps.size()+(B.End==CompleteEnd::Zero?3:1);
  }
  CB_CHECK(E.Instructions.size()>Count,"missing compiler prologue");
  L.Start=E.Instructions.size()-Count;
  L.Tail=E.Instructions.size()-9;
  for(auto &Offset:L.BlockStarts) Offset+=L.Start;
  uint64_t Cursor=Entry.CodeOffset;
  for(size_t I=0;I<E.Instructions.size();++I) {
    const auto &S=E.Instructions[I];
    CB_CHECK(S.FunctionSymbol==Symbol && S.InstructionOffset==Cursor && !S.Encoding.empty() &&
        Cursor<=Payload.size() && S.Encoding.size()<=Payload.size()-Cursor &&
        std::equal(S.Encoding.begin(),S.Encoding.end(),Payload.begin()+Cursor),"complete entry bytes");
    Cursor+=S.Encoding.size();
    if(I<L.Start)
      CB_CHECK(S.BranchKind==PhysicalMachineBranchKind::None &&
          (S.MemoryAccess==PhysicalMachineMemoryAccess::None ||
           S.MemoryAccess==PhysicalMachineMemoryAccess::Read),"prologue effects");
  }
  CB_CHECK(Cursor==Entry.CodeOffset+Entry.CodeSize,"entry suffix");
  // Independently join the worker's CFG rows to every trace instruction and
  // direct target. No re-decoding or guessed branch-displacement interpreter.
  size_t Covered=0;
  for(size_t BI=0;BI<E.Blocks.size();++BI) {
    const auto &B=E.Blocks[BI];
    CB_CHECK(B.FunctionSymbol==Symbol && B.Ordinal==BI && B.InstructionCount>0 &&
        B.InstructionCount<=E.Instructions.size()-Covered &&
        B.FirstInstructionOffset==E.Instructions[Covered].InstructionOffset,"CFG census");
    for(size_t J=0;J<B.InstructionCount;++J)
      CB_CHECK(E.Instructions[Covered+J].BlockOrdinal==BI,"CFG instruction membership");
    const auto &Last=E.Instructions[Covered+B.InstructionCount-1];
    std::vector<uint32_t> Expected;
    if(Last.BranchKind==PhysicalMachineBranchKind::ConditionalDirect ||
       Last.BranchKind==PhysicalMachineBranchKind::UnconditionalDirect) {
      auto Target=std::find_if(E.Blocks.begin(),E.Blocks.end(),[&](const auto &X) {
        return X.FirstInstructionOffset==Last.BranchTarget;
      });
      CB_CHECK(Target!=E.Blocks.end(),"CFG target boundary");
      Expected.push_back(Target->Ordinal);
      if(Last.BranchKind==PhysicalMachineBranchKind::ConditionalDirect) {
        CB_CHECK(BI+1<E.Blocks.size(),"conditional fallthrough");
        if(Expected[0]!=BI+1) Expected.push_back(BI+1);
      }
    } else if(Last.BranchKind==PhysicalMachineBranchKind::None) {
      CB_CHECK(BI+1<E.Blocks.size(),"nonterminal entry suffix"); Expected.push_back(BI+1);
    } else CB_CHECK(Last.BranchKind==PhysicalMachineBranchKind::Return,"forbidden call");
    llvm::sort(Expected); // The existing worker exports sorted successor ordinals.
    CB_CHECK(B.Successors==Expected,"decoded CFG successor relation");
    Covered+=B.InstructionCount;
  }
  CB_CHECK(Covered==E.Instructions.size(),"CFG suffix");
  const auto CheckBranch=[&](size_t At,size_t To,bool Conditional)->bool {
    if(At>=E.Instructions.size() || To>=E.Instructions.size()) return false;
    const auto &S=E.Instructions[At];
    if(S.Encoding.size()!=4 || S.MemoryAccess!=PhysicalMachineMemoryAccess::None ||
       S.MemoryWidth!=0 || S.ExplicitDefinitionCount!=0 || !completeRegisters(S).empty() ||
       !S.ImplicitDefinitions.empty() ||
       S.BranchTarget!=E.Instructions[To].InstructionOffset ||
       S.BranchKind!=(Conditional?PhysicalMachineBranchKind::ConditionalDirect:
         PhysicalMachineBranchKind::UnconditionalDirect)) return false;
    const auto Word=support::endian::read32le(S.Encoding.data());
    return (Word&0xffff0000U)==(Conditional?0xbf850000U:0xbf820000U) &&
      opcodeVariant(S.Opcode,Conditional?"S_CBRANCH_SCC1":"S_BRANCH") &&
      S.ImplicitUses==(Conditional?std::vector<std::string>{"SCC"}:std::vector<std::string>{});
  };
  for(size_t BI=0;BI<P.Blocks.size();++BI) {
    const auto &B=P.Blocks[BI];
    size_t At=L.BlockStarts[BI];
    for(const auto &StepValue:B.Steps) {
      CB_CHECK(matchesInstruction(E.Instructions[At],StepValue,Payload,Symbol),"authored arithmetic");
      L.Arithmetic.push_back(At++);
    }
    if(B.End==CompleteEnd::Zero) {
      const auto &Cmp=E.Instructions[At];
      CB_CHECK(opcodeVariant(Cmp.Opcode,"S_CMP_EQ_U32") && Cmp.Encoding.size()==4 &&
          support::endian::read32le(Cmp.Encoding.data())==0xbf068016U &&
          completeRegisters(Cmp)==std::vector<std::string>{"SGPR22"} &&
          Cmp.ExplicitDefinitionCount==0 && Cmp.ImplicitUses.empty() &&
          Cmp.ImplicitDefinitions==std::vector<std::string>{"SCC"} &&
          Cmp.BranchKind==PhysicalMachineBranchKind::None &&
          Cmp.MemoryAccess==PhysicalMachineMemoryAccess::None,"selector s22 comparison");
      CB_CHECK(CheckBranch(At+1,L.BlockStarts[B.Zero],true) &&
          CheckBranch(At+2,L.BlockStarts[B.Nonzero],false),"selector successor orientation");
      L.Branches.push_back(At+1); L.Branches.push_back(At+2);
    } else {
      const size_t Target=B.End==CompleteEnd::Tail?L.Tail:L.BlockStarts[B.Zero];
      CB_CHECK(CheckBranch(At,Target,false),"authored direct edge");
      L.Branches.push_back(At);
    }
  }
  const std::array<StringRef,9> Opcodes{
    "V_LSHLREV_B64_vi","V_ADD_CO_U32_e32_gfx9","V_ADDC_CO_U32_e32_gfx9",
    "V_CMP_GT_U64_e32_vi","S_AND_SAVEEXEC_B64_vi","GLOBAL_STORE_DWORD_vi",
    "S_WAITCNT_vi","S_MOV_B64_vi","S_ENDPGM_vi"};
  const std::array<std::vector<std::string>,9> Registers{{
    {"VGPR2_VGPR3","VGPR0_VGPR1"},{"VGPR2","SGPR16","VGPR2"},{"VGPR3","VGPR4","VGPR3"},
    {"SGPR18_SGPR19","VGPR0_VGPR1"},{"SGPR20_SGPR21","VCC"},
    {"VGPR2_VGPR3","VGPR"+std::to_string(P.Output)},{},{"EXEC","SGPR20_SGPR21"},{}}};
  const std::array<uint16_t,9> Definitions{{1,1,1,0,1,0,0,1,0}};
  const std::array<std::vector<std::string>,9> Uses{{
    {"EXEC"},{"EXEC"},{"EXEC","VCC"},{"EXEC"},{"EXEC"},{"EXEC"},{},{},{}}};
  const std::array<std::vector<std::string>,9> Writes{{
    {},{"VCC"},{"VCC"},{"VCC"},{"EXEC","SCC"},{},{},{},{}}};
  for(size_t I=0;I<9;++I) {
    const auto &S=E.Instructions[L.Tail+I];
    CB_CHECK(S.Opcode==Opcodes[I] && completeRegisters(S)==Registers[I] &&
        S.ExplicitDefinitionCount==Definitions[I] && S.ImplicitUses==Uses[I] &&
        S.ImplicitDefinitions==Writes[I],"guarded tail registers");
    CB_CHECK(S.BranchKind==(I==8?PhysicalMachineBranchKind::Return:PhysicalMachineBranchKind::None) &&
        S.MemoryAccess==(I==5?PhysicalMachineMemoryAccess::Write:PhysicalMachineMemoryAccess::None),
        "guarded tail effects");
    if(I==5) CB_CHECK(S.MemoryWidth==4 && S.Encoding.size()==8 &&
        support::endian::read64le(S.Encoding.data())==
          ((uint64_t(0x007f0002U | (uint32_t(P.Output)<<8))<<32)|0xdc708000ULL),
        "guarded tail store encoding");
    if(I==0) CB_CHECK(llvm::any_of(S.Operands,[](const auto &O) {
      return O.Kind==PhysicalMachineOperandKind::SignedImmediate && O.Value==2;
    }),"guarded tail scale");
    if(I==6) CB_CHECK(S.Encoding.size()==4 &&
        support::endian::read32le(S.Encoding.data())==0xbf8c0f70U,"guarded tail wait");
    if(I==8) CB_CHECK(S.Encoding.size()==4 &&
        support::endian::read32le(S.Encoding.data())==0xbf810000U,"guarded tail end");
  }
  if(Found) *Found=std::move(L);
  return std::nullopt;
}
#undef CB_CHECK

Expected<PhysicalMachineEffectEvidence> completeAnalyze(ArrayRef<uint8_t> Bytes,StringRef Symbol) {
  require(!Bytes.empty() && Bytes.size()<=64*1024,"changed retained payload cap");
  const auto Identities=physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest R;
  R.AnalyzerIdentity=Identities.Analyzer; R.ToolchainIdentity=Identities.Toolchain;
  R.ExecutionChallenge.fill(0x70); R.RequestIdentity.fill(0x71); R.RequestBytes=4096;
  R.Payload.assign(Bytes.begin(),Bytes.end()); R.PayloadDigest=SHA256::hash(Bytes);
  R.PayloadBytes=Bytes.size(); R.Entries={{Symbol.str(),{16,8,4,2,0}}};
  return analyzeGfx942PhysicalMachineEffects(R);
}
json::Array completeNativeNegatives(const CompletePlan &P,const BuiltFixture &Built,
    StringRef Symbol,const CompleteLayout &L) {
  const auto &Original=Built.ResponseValue.LinkedOutput->Bytes;
  const auto &Trace=Built.Evidence.Instructions;
  struct Mutation { StringRef Name; size_t At; uint32_t Word; };
  std::vector<Mutation> Changes;
  const auto Branch=L.Branches.front();
  const auto BranchWord=support::endian::read32le(Trace[Branch].Encoding.data());
  Changes.push_back({"branch_self",Branch,(BranchWord&0xffff0000U)|0xffffU});
  Changes.push_back({"branch_external",Branch,(BranchWord&0xffff0000U)|0x7fffU});
  const int64_t Back=static_cast<int64_t>(Trace.front().InstructionOffset)-
      static_cast<int64_t>(Trace[Branch].InstructionOffset+4);
  require(Back%4==0 && Back/4>=-32768 && Back<0,"backward target bound");
  Changes.push_back({"branch_back_to_prologue",Branch,(BranchWord&0xffff0000U)|uint16_t(Back/4)});
  // Target the second word of the real 8-byte store, not a stale outer digest.
  const int64_t Middle=static_cast<int64_t>(Trace[L.Tail+5].InstructionOffset+4)-
      static_cast<int64_t>(Trace[Branch].InstructionOffset+4);
  require(Middle%4==0 && Middle/4>=-32768 && Middle/4<=32767,"middle target bound");
  Changes.push_back({"branch_mid_instruction",Branch,(BranchWord&0xffff0000U)|uint16_t(Middle/4)});
  const int64_t Wrong=static_cast<int64_t>(Trace[L.Tail].InstructionOffset)-
      static_cast<int64_t>(Trace[Branch].InstructionOffset+4);
  if(Trace[Branch].BranchTarget!=Trace[L.Tail].InstructionOffset)
    Changes.push_back({"branch_wrong_valid_boundary",Branch,(BranchWord&0xffff0000U)|uint16_t(Wrong/4)});
  const auto A=L.Arithmetic.front();
  Step Different{Opcode::Mov,P.Output,P.Inputs[2],0};
  if(support::endian::read32le(Trace[A].Encoding.data())==expectedWord(Different))
    Different.Source0=P.Inputs[1];
  Changes.push_back({"arithmetic_register",A,expectedWord(Different)});
  Changes.push_back({"wait_vm_one",L.Tail+6,0xbf8c0f71U});
  if(P.Blocks[0].End==CompleteEnd::Zero)
    Changes.push_back({"selector_s23",L.Start,0xbf068017U});
  json::Array Results;
  for(const auto &M:Changes) {
    std::vector<uint8_t> Changed=Original;
    const auto Offset=Trace[M.At].InstructionOffset;
    require(Offset+4<=Changed.size(),"native mutation extent");
    support::endian::write32le(Changed.data()+Offset,M.Word);
    auto Reanalyzed=completeAnalyze(Changed,Symbol);
    std::string Phase,Reason;
    if(!Reanalyzed) { Phase="existing_machine_decoder"; Reason=toString(Reanalyzed.takeError()); }
    else {
      Phase="complete_body_native_relation";
      auto Failure=completeRelation(P,*Reanalyzed,Changed,Symbol);
      require(Failure.has_value(),"changed native relation accepted");
      Reason=*Failure;
    }
    require(Reason.size()<=4096,"native refusal diagnostic cap");
    Results.emplace_back(json::Object{{"mutation",M.Name},{"phase",Phase},{"reason",Reason},
      {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"fresh_payload_hash",true},
      {"accepted",false}});
  }
  return Results;
}
