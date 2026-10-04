// Reuses the existing strict production analyzer unchanged. In particular,
// SCRATCH/BUFFER families remain closed. Unsupported effects refuse. Such refusal
// is recorded as incomplete observation, never reclassified as qualification.
constexpr std::array<Step,2> CompositionPair{{{Opcode::Xor,32,34,35},{Opcode::And,33,32,36}}};
constexpr std::array<Step,1> CompositionMove0{{{Opcode::Mov,33,34,0}}};
constexpr std::array<Step,1> CompositionMove1{{{Opcode::Mov,33,35,0}}};
// Independently transcribed promoted program, not decoded from the report.
constexpr std::array<Step,2> PromotedPair{{{Opcode::Xor,8,10,11},{Opcode::And,9,8,12}}};
constexpr std::array<Step,1> PromotedMove2{{{Opcode::Mov,9,12,0}}};
static_assert(expectedWord(PromotedPair[0])==0x2a10170a);
static_assert(expectedWord(PromotedPair[1])==0x26121908);
static_assert(expectedWord(PromotedMove2[0])==0x7e12030c);
ArrayRef<Step> compositionSteps(size_t Kind,CompositionProfile P=CompositionProfile::LegacyV32) {
  if(P==CompositionProfile::PromotedV8) {
    if(Kind==0)return PromotedPair;
    if(Kind==1)return PromotedMove2;
    fail("native closed promoted program");
  }
  if (Kind==0) return CompositionPair;
  if (Kind==1) return CompositionMove0;
  if (Kind==2) return CompositionMove1;
  fail("native closed literal program");
}
struct CompositionInterval {std::string Function;size_t Kind,First,Count;};
bool compositionNativeRelation(const PhysicalMachineEffectEvidence &E,ArrayRef<uint8_t> Bytes,
    const CompositionRecord &R,std::vector<CompositionInterval> *Output=nullptr) {
  if(Bytes.empty() || Bytes.size()>PayloadByteLimit || E.PayloadDigest!=SHA256::hash(Bytes) ||
      E.PayloadBytes!=Bytes.size() || E.Entries.size()!=1 || E.Entries[0].Symbol!=R.Input.Entry ||
      E.Functions.empty() || E.Functions.size()>R.Input.Helpers.size()+1 ||
      E.Instructions.empty() || E.Instructions.size()>1024 || E.Blocks.size()>128)return false;
  std::vector<std::string> Names;std::array<size_t,3> Expanded{};
  std::vector<CompositionInterval> Found;
  for(const auto &F:E.Functions) {
    if(F.Symbol!=R.Input.Entry && !std::binary_search(R.Input.Helpers.begin(),R.Input.Helpers.end(),F.Symbol))return false;
    if(F.CodeOffset>Bytes.size() || F.CodeSize==0 || F.CodeSize>Bytes.size()-F.CodeOffset)return false;
    Names.push_back(F.Symbol);
  }
  std::sort(Names.begin(),Names.end());
  if(std::adjacent_find(Names.begin(),Names.end())!=Names.end() ||
      !std::binary_search(Names.begin(),Names.end(),R.Input.Entry))return false;
  for(size_t N=0;N<E.Instructions.size();++N) {
    const auto &I=E.Instructions[N];
    for(size_t P=0;P<N;++P)if(E.Instructions[P].InstructionOffset==I.InstructionOffset)return false;
    if(!std::binary_search(Names.begin(),Names.end(),I.FunctionSymbol) ||
        I.InstructionOffset>Bytes.size() || I.Encoding.empty() || I.Encoding.size()>16 ||
        I.Encoding.size()>Bytes.size()-I.InstructionOffset ||
        !std::equal(I.Encoding.begin(),I.Encoding.end(),Bytes.begin()+I.InstructionOffset))return false;
    if(I.BranchKind==PhysicalMachineBranchKind::DirectCall && I.FunctionSymbol!=R.Input.Entry)return false;
  }
  for(const auto &F:E.Functions) {
    size_t Weight=F.Symbol==R.Input.Entry?1:0;
    if(F.Symbol!=R.Input.Entry)
      for(const auto &I:E.Instructions)if(I.BranchKind==PhysicalMachineBranchKind::DirectCall &&
          I.FunctionSymbol==R.Input.Entry && I.BranchTarget==F.CodeOffset)++Weight;
    if(Weight==0 || Weight>8)return false; // No unreferenced/recursive native clone.
    for(size_t Kind=0;Kind<compositionPatternCount(R.Profile);++Kind) {
      const auto Steps=compositionSteps(Kind,R.Profile);
      for(size_t I=0;I+Steps.size()<=E.Instructions.size();++I) {
        const auto &First=E.Instructions[I];bool Match=true;
        for(size_t J=0;J<Steps.size();++J) {
          const auto &Row=E.Instructions[I+J];
          Match &= matchesInstruction(Row,Steps[J],Bytes,F.Symbol) &&
            Row.BlockOrdinal==First.BlockOrdinal && Row.InstructionOffset==First.InstructionOffset+4*J &&
            Row.InstructionOffset>=F.CodeOffset && Row.InstructionOffset-F.CodeOffset<=F.CodeSize &&
            F.CodeSize-(Row.InstructionOffset-F.CodeOffset)>=4;
        }
        if(!Match)continue;
        if(Found.size()>=8)return false;
        for(const auto &Old:Found)if(I<Old.First+Old.Count && Old.First<I+Steps.size())return false;
        Found.push_back({F.Symbol,Kind,I,Steps.size()});Expanded[Kind]+=Weight;
      }
    }
  }
  if(Expanded!=R.Expanded)return false;
  if(Output)*Output=std::move(Found);
  return true;
}
Expected<PhysicalMachineEffectEvidence> compositionAnalyze(const CompositionRecord &R,ArrayRef<uint8_t> Bytes) {
  const auto Id=physicalMachineEffectIdentities();PhysicalMachineEffectRequest Q;
  Q.AnalyzerIdentity=Id.Analyzer;Q.ToolchainIdentity=Id.Toolchain;Q.ExecutionChallenge.fill(0x70);
  Q.RequestIdentity.fill(0x71);Q.RequestBytes=4096; // Explicit synthetic diagnostic domain.
  Q.Payload.assign(Bytes.begin(),Bytes.end());Q.PayloadDigest=SHA256::hash(Bytes);Q.PayloadBytes=Bytes.size();
  Q.Entries={{R.Input.Entry,{32,16,8,8,8}}};
  auto E=analyzeGfx942PhysicalMachineEffects(Q);if(!E)return E.takeError();
  require(E->AnalyzerIdentity==Id.Analyzer && E->ToolchainIdentity==Id.Toolchain &&
    E->ExecutionChallenge==Q.ExecutionChallenge && E->RequestIdentity==Q.RequestIdentity &&
    E->RequestBytes==Q.RequestBytes,"exact diagnostic analyzer request/output identity");
  return std::move(*E);
}
json::Object compositionMachine(const CompositionRecord &R,ArrayRef<uint8_t> Bytes,
    const CompositionResources &Resources,bool &Qualified) {
  auto E=compositionAnalyze(R,Bytes);
  if(!E) {
    auto Reason=toString(E.takeError());require(Reason.size()<=8192,"bounded analyzer refusal");
    Qualified=false;
    return json::Object{{"status","incomplete_analyzer_refusal"},{"reason",Reason},
      {"full_instruction_preservation_qualified",false},{"production_analyzer_relaxed",false}};
  }
  require(E->Entries.size()==1 && E->Entries[0].DescriptorIdentity==Resources.DescriptorDigest,
    "machine/metadata exact descriptor identity");
  std::vector<CompositionInterval> Intervals;
  require(compositionNativeRelation(*E,Bytes,R,&Intervals),"closed source program/native interval census mismatch");
  size_t Negatives=0;
  const auto Reject=[&](PhysicalMachineEffectEvidence Changed) {
    require(!compositionNativeRelation(Changed,Bytes,R),"corrupted native observation admitted");++Negatives;
  };
  const size_t At=Intervals.front().First;
  auto Changed=*E;Changed.PayloadDigest[0]^=1;Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].Opcode="V_MOV_B32_e64";Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].Operands[0].Register="VGPR0";Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].Operands[1].TiedTo=0;Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].ImplicitUses.clear();Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].ImplicitDefinitions={"EXEC"};Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].MemoryAccess=PhysicalMachineMemoryAccess::Read;Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].ExplicitDefinitionCount=2;Reject(std::move(Changed));
  Changed=*E;Changed.Instructions[At].Encoding[0]^=1;Reject(std::move(Changed));
  Changed=*E;Changed.Instructions.erase(Changed.Instructions.begin()+At);Reject(std::move(Changed));
  Changed=*E;Changed.Instructions.push_back(E->Instructions[At]);Reject(std::move(Changed));
  require(Negatives==11,"decoded mutation control census");
  json::Array Rows;
  for(const auto &I:Intervals) {
    json::Array Instructions;
    for(size_t J=0;J<I.Count;++J)Instructions.emplace_back(instructionJson(E->Instructions[I.First+J]));
    Rows.emplace_back(json::Object{{"function",I.Function},{"literal_kind",I.Kind},
      {"instructions",std::move(Instructions)}});
  }
  Qualified=true;
  return json::Object{{"status","exact_authored_intervals_observed"},
    {"authored_intervals",std::move(Rows)},{"decoded_observation_mutation_refusals",Negatives},
    {"native",helperNativeObservation(*E)},{"physical_helper_argument_result_flow_proved",false},
    {"root_value_guard_store_functional_equivalence_proved",false},
    {"flattened_native_sequence_claim",false},{"complete_dynamic_execution_order_proved",false}};
}
