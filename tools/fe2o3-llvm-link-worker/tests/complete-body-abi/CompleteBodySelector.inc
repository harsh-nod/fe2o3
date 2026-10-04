// Test-only provenance over already decoded operands, not another decoder or
// arithmetic/CFG interpreter. Unknown definitions erase provenance.
enum class SelectorOriginKind { Unknown, KernargLow, KernargHigh, Word };
struct SelectorOrigin {
  SelectorOriginKind Kind=SelectorOriginKind::Unknown;
  uint64_t Offset=0;
  size_t Load=0;
  bool Ready=false;
  std::vector<size_t> Copies;
};
struct CompleteSelectorFlow {
  unsigned KernargSgpr=0;
  size_t Load=0;
  std::vector<size_t> Copies;
};
bool selectorSgprs(StringRef Name,std::vector<unsigned> &Registers) {
  if(!Name.starts_with("SGPR")) return true; // no ordinary SGPR component
  SmallVector<StringRef,16> Parts; Name.split(Parts,'_');
  if(Parts.empty() || Parts.size()>16) return false;
  for(StringRef Part:Parts) {
    unsigned Index=0;
    if(!Part.consume_front("SGPR") || Part.getAsInteger(10,Index) || Index>=112) return false;
    Registers.push_back(Index);
  }
  for(size_t I=1;I<Registers.size();++I) if(Registers[I]!=Registers[I-1]+1) return false;
  return true;
}
std::optional<std::string> completeSelectorFlow(const PhysicalMachineEffectEvidence &E,
    const CompleteDescriptor &D,size_t Start,CompleteSelectorFlow *Found=nullptr) {
  using namespace llvm::amdhsa;
  const uint16_t Properties=D.CodeProperties;
  if(!(Properties&KERNEL_CODE_PROPERTY_ENABLE_SGPR_KERNARG_SEGMENT_PTR) ||
      (Properties&KERNEL_CODE_PROPERTY_ENABLE_WAVEFRONT_SIZE32))
    return std::string("descriptor lacks wave64 kernarg SGPR");
  unsigned Base=0;
  if(Properties&KERNEL_CODE_PROPERTY_ENABLE_SGPR_PRIVATE_SEGMENT_BUFFER) Base+=4;
  if(Properties&KERNEL_CODE_PROPERTY_ENABLE_SGPR_DISPATCH_PTR) Base+=2;
  if(Properties&KERNEL_CODE_PROPERTY_ENABLE_SGPR_QUEUE_PTR) Base+=2;
  if(Base+2>112 || ((D.Rsrc2>>1)&31)<Base+2 || Start>E.Instructions.size())
    return std::string("descriptor kernarg register range");
  std::array<SelectorOrigin,112> State{};
  std::array<bool,112> Pending{}; // destination hazards survive provenance kills
  State[Base].Kind=SelectorOriginKind::KernargLow;
  State[Base+1].Kind=SelectorOriginKind::KernargHigh;
  for(size_t At=0;At<Start;++At) {
    const auto &I=E.Instructions[At];
    if(I.ExplicitDefinitionCount>I.Operands.size()) return std::string("definition operand census");
    // Read the pre-instruction sources before any overlapping destination kills.
    const auto Before=State;
    bool WritesOrdinarySgpr=false;
    for(size_t Def=0;Def<I.ExplicitDefinitionCount;++Def) {
      const auto &Operand=I.Operands[Def];
      if(Operand.Kind!=PhysicalMachineOperandKind::Register) return std::string("nonregister definition");
      std::vector<unsigned> Written;
      if(!selectorSgprs(Operand.Register,Written)) return std::string("unknown SGPR definition alias");
      for(unsigned Register:Written) {
        if(Pending[Register]) return std::string("write overlaps unfinished scalar load");
        WritesOrdinarySgpr=true; State[Register]=SelectorOrigin{};
      }
    }
    for(const auto &Name:I.ImplicitDefinitions) {
      std::vector<unsigned> Written;
      if(!selectorSgprs(Name,Written)) return std::string("unknown implicit SGPR alias");
      for(unsigned Register:Written) {
        if(Pending[Register]) return std::string("implicit write overlaps unfinished scalar load");
        WritesOrdinarySgpr=true; State[Register]=SelectorOrigin{};
      }
    }
    if(I.Opcode=="S_WAITCNT_vi") {
      if(I.Operands.size()!=1 || I.Operands[0].Kind!=PhysicalMachineOperandKind::SignedImmediate)
        return std::string("wait operand profile");
      if((I.Operands[0].Value&0xf00)==0) {
        for(auto &Origin:State) if(Origin.Kind==SelectorOriginKind::Word) Origin.Ready=true;
        Pending.fill(false);
      }
      continue;
    }
    const bool Move32=I.Opcode=="S_MOV_B32_vi",Move64=I.Opcode=="S_MOV_B64_vi";
    if(Move32 || Move64) {
      if(I.ExplicitDefinitionCount!=1 || I.Operands.size()!=2 ||
          I.Operands[0].Kind!=PhysicalMachineOperandKind::Register)
        return std::string("scalar move shape");
      if(I.Operands[1].Kind!=PhysicalMachineOperandKind::Register) continue;
      std::vector<unsigned> To,From;
      if(!selectorSgprs(I.Operands[0].Register,To) ||
          !selectorSgprs(I.Operands[1].Register,From)) return std::string("scalar move aliases");
      if(To.empty()) continue;
      if(To.size()!=(Move64?2U:1U) || From.size()!=To.size()) continue; // unknown/non-SGPR source
      for(size_t J=0;J<To.size();++J) {
        auto Origin=Before[From[J]];
        if(Origin.Kind==SelectorOriginKind::Word) {
          if(!Origin.Ready) continue; // cannot copy an unfinished asynchronous load
          if(Origin.Copies.size()>=InstructionLimit) return std::string("copy chain cap");
          Origin.Copies.push_back(At);
        }
        State[To[J]]=std::move(Origin);
      }
      continue;
    }
    size_t Words=0;
    if(I.Opcode=="S_LOAD_DWORD_IMM_vi") Words=1;
    else if(I.Opcode=="S_LOAD_DWORDX2_IMM_vi") Words=2;
    else if(I.Opcode=="S_LOAD_DWORDX4_IMM_vi") Words=4;
    else if(I.Opcode=="S_LOAD_DWORDX8_IMM_vi") Words=8;
    else if(I.Opcode=="S_LOAD_DWORDX16_IMM_vi") Words=16;
    if(!Words) {
      if(WritesOrdinarySgpr && I.MemoryAccess!=PhysicalMachineMemoryAccess::None)
        return std::string("unknown scalar load provenance");
      continue;
    }
    if(I.ExplicitDefinitionCount!=1 || I.Operands.size()!=4 ||
        I.Operands[0].Kind!=PhysicalMachineOperandKind::Register ||
        I.Operands[1].Kind!=PhysicalMachineOperandKind::Register ||
        I.Operands[2].Kind!=PhysicalMachineOperandKind::SignedImmediate ||
        I.Operands[3].Kind!=PhysicalMachineOperandKind::SignedImmediate || I.Operands[3].Value!=0 ||
        I.MemoryAccess!=PhysicalMachineMemoryAccess::Read || I.MemoryWidth!=4*Words)
      return std::string("scalar immediate load shape");
    std::vector<unsigned> To,Pointer;
    if(!selectorSgprs(I.Operands[0].Register,To) || !selectorSgprs(I.Operands[1].Register,Pointer) ||
        To.size()!=Words || Pointer.size()!=2) return std::string("scalar load register tuple");
    for(unsigned Register:To) Pending[Register]=true;
    if(Before[Pointer[0]].Kind!=SelectorOriginKind::KernargLow ||
        Before[Pointer[1]].Kind!=SelectorOriginKind::KernargHigh) continue;
    const uint64_t Offset=I.Operands[2].Value; // pinned gfx9 IMM MC operand uses bytes
    if(Offset%4 || Offset>288 || 4*Words>288-Offset) return std::string("scalar kernarg load range");
    for(size_t J=0;J<Words;++J)
      State[To[J]]=SelectorOrigin{SelectorOriginKind::Word,Offset+4*J,At,false,{}};
  }
  const auto &Selector=State[22];
  if(Selector.Kind!=SelectorOriginKind::Word || Selector.Offset!=28 || !Selector.Ready)
    return std::string("current s22 does not derive from ready kernarg byte28");
  if(Found) *Found={Base,Selector.Load,Selector.Copies};
  return std::nullopt;
}
json::Array completeSelectorNegatives(const BuiltFixture &Built,StringRef Symbol,
    const CompleteDescriptor &D,size_t Start,const CompleteSelectorFlow &Flow) {
  const auto &Original=Built.ResponseValue.LinkedOutput->Bytes;
  const auto &Trace=Built.Evidence.Instructions;
  const auto &Load=Trace[Flow.Load];
  require(Load.Encoding.size()==8 && Flow.Load<Start,"actual selector load encoding bound");
  struct Mutation { StringRef Name; uint64_t Offset; uint32_t Word; };
  std::vector<Mutation> Changes{
    {"selector_kernarg_load_offset",Load.InstructionOffset+4,
     support::endian::read32le(Load.Encoding.data()+4)^4U}};
  // Replace the latest four-byte prologue instruction with an explicit s22
  // overwrite. This is a genuine native mutation, never an altered trace row.
  std::optional<size_t> Last;
  for(size_t I=0;I<Start;++I) if(Trace[I].Encoding.size()==4) Last=I;
  require(Last.has_value(),"selector overwrite mutation needs actual 4-byte prologue site");
  require(*Last>=Flow.Load && (Flow.Copies.empty() || *Last>=Flow.Copies.back()),
      "selector overwrite must follow its actual load and last provenance copy");
  Changes.push_back({"selector_s22_overwrite",Trace[*Last].InstructionOffset,0xbe960080U});
  json::Array Rows;
  for(const auto &M:Changes) {
    auto Changed=Original;
    require(M.Offset+4<=Changed.size(),"selector mutation extent");
    support::endian::write32le(Changed.data()+M.Offset,M.Word);
    auto E=completeAnalyze(Changed,Symbol);
    std::string Phase,Reason;
    if(!E) {Phase="existing_machine_decoder";Reason=toString(E.takeError());}
    else {
      Phase="selector_native_provenance";
      auto Error=completeSelectorFlow(*E,D,Start);
      require(Error.has_value(),"changed native selector dataflow accepted");
      Reason=*Error;
    }
    require(Reason.size()<=4096,"selector diagnostic cap");
    Rows.emplace_back(json::Object{{"mutation",M.Name},{"phase",Phase},{"reason",Reason},
      {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"fresh_payload_hash",true},{"accepted",false}});
  }
  return Rows;
}
