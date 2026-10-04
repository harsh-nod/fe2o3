// Independent descriptor -> existing MC expectation rows, not an assembler or evaluator.
std::string globalCopyS(unsigned N) {return "SGPR"+std::to_string(N);}
std::string globalCopyV(unsigned N) {return "VGPR"+std::to_string(N);}
std::string globalCopySP(unsigned N) {return globalCopyS(N)+"_"+globalCopyS(N+1);}
std::string globalCopyVP(unsigned N) {return globalCopyV(N)+"_"+globalCopyV(N+1);}
std::optional<PhysicalRow> globalCopyPhysicalRow(const GlobalCopyStep &Step) {
  using M=PhysicalMachineMemoryAccess;
  const auto &X=Step.Descriptor;const unsigned O=X[0],D=X[1],A=X[2],B=X[3];
  const uint32_t I=support::endian::read32le(X.data()+4);
  const auto S=[](unsigned R){return R<=63;};
  const auto SD=[](unsigned R){return R>=3&&R<=63;};
  const auto V=[](unsigned R){return R<=63;};
  const auto VD=[](unsigned R){return R>=1&&R<=63;};
  const auto Pair=[](unsigned R){return R<=62&&R%2==0;};
  switch(O) {
  case 0:
    if(!SD(D)||!Pair(D)||A||B||(I!=0&&I!=8&&I!=16&&I!=24))return std::nullopt;
    return PhysicalRow{"S_LOAD_DWORDX2_IMM_vi",{globalCopySP(D),"SGPR0_SGPR1"},{I,0},1,8,{},{},M::Read,8};
  case 1:
    if(D||A||B||I)return std::nullopt;
    return PhysicalRow{"S_WAITCNT_vi",{},{0xc07f},0,4,{},{}};
  case 2:
    if(!SD(D)||!S(A)||B||I!=6)return std::nullopt;
    return PhysicalRow{"S_LSHL_B32_vi",{globalCopyS(D),globalCopyS(A)},{6},1,4,{},{"SCC"}};
  case 3:
    if(!VD(D)||!S(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADD_U32_e32_gfx9",{globalCopyV(D),globalCopyS(A),globalCopyV(B)},{},1,4,{"EXEC"},{}};
  case 4:
    if(!VD(D)||(A!=255&&!S(A))||B||I)return std::nullopt;
    if(A==255)return PhysicalRow{"V_MOV_B32_e32_vi",{globalCopyV(D)},{0},1,4,{"EXEC"},{}};
    return PhysicalRow{"V_MOV_B32_e32_vi",{globalCopyV(D),globalCopyS(A)},{},1,4,{"EXEC"},{}};
  case 5:
    if(!VD(D)||!Pair(D)||!Pair(A)||B||I!=2)return std::nullopt;
    return PhysicalRow{"V_LSHLREV_B64_vi",{globalCopyVP(D),globalCopyVP(A)},{2},1,8,{"EXEC"},{}};
  case 6:
    if(!VD(D)||!S(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADD_CO_U32_e32_gfx9",{globalCopyV(D),globalCopyS(A),globalCopyV(B)},{},1,4,{"EXEC"},{"VCC"}};
  case 7:
    if(!VD(D)||!V(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADDC_CO_U32_e32_gfx9",{globalCopyV(D),globalCopyV(A),globalCopyV(B)},{},1,4,{"EXEC","VCC"},{"VCC"}};
  case 10:
    if(D||!Pair(A)||!Pair(B)||I)return std::nullopt;
    return PhysicalRow{"V_CMP_GT_U64_e32_vi",{globalCopySP(A),globalCopyVP(B)},{},0,4,{"EXEC"},{"VCC"}};
  case 11:
    if(!SD(D)||!Pair(D)||A||B||I)return std::nullopt;
    return PhysicalRow{"S_AND_SAVEEXEC_B64_vi",{globalCopySP(D),"VCC"},{},1,4,{"EXEC"},{"EXEC","SCC"}};
  case 12:
    if(D||!Pair(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"GLOBAL_STORE_DWORD_vi",{globalCopyVP(A),globalCopyV(B)},{0,0},0,8,{"EXEC"},{},M::Write,4};
  case 9:
    if(D||A||B||I)return std::nullopt;
    return PhysicalRow{"S_WAITCNT_vi",{},{0x0f70},0,4,{},{}};
  case 13:
    if(D||!Pair(A)||B||I)return std::nullopt;
    return PhysicalRow{"S_MOV_B64_vi",{"EXEC",globalCopySP(A)},{},1,4,{},{}};
  case 8:
    if(!VD(D)||!Pair(A)||B||I)return std::nullopt;
    return PhysicalRow{"GLOBAL_LOAD_DWORD_vi",{globalCopyV(D),globalCopyVP(A)},{0,0},1,8,{"EXEC"},{},M::Read,4};
  default:return std::nullopt;
  }
}
struct GlobalCopyRelation {
  std::vector<PhysicalRow> Rows;
  uint32_t Sgprs=3,Vgprs=1;
};
std::optional<GlobalCopyRelation> globalCopyPhysicalRows(const GlobalCopyInput &Input) {
  using M=PhysicalMachineMemoryAccess;using B=PhysicalMachineBranchKind;
  // Closed one-block global-copy memory/wait census. The canonical/source
  // owner is joined externally; these diagnostic rows never authenticate it.
  if(Input.Starts.size()!=1 || Input.Starts[0]!=0 || Input.Counts.size()!=1 ||
      Input.Encodings.size()!=1 || Input.Encodings[0]!=4 || Input.Edges.size()!=1 ||
      !Input.Edges[0].empty() || Input.Steps.size()<23 || Input.Steps.size()>31 ||
      Input.Counts[0]!=Input.Steps.size()+1)
    return std::nullopt;
  std::array<unsigned,14> Census{};
  for(size_t N=0;N<Input.Steps.size();++N) {
    const auto &S=Input.Steps[N];
    if(S.Block || S.Native!=N || S.Descriptor[0]>=Census.size())return std::nullopt;
    ++Census[S.Descriptor[0]];
    if(N<4 && (S.Descriptor[0]!=0 || support::endian::read32le(S.Descriptor.data()+4)!=8*N))
      return std::nullopt;
    if(N==4 && S.Descriptor[0]!=1)return std::nullopt;
    if(S.Descriptor[0]==8 && (N+1>=Input.Steps.size() || Input.Steps[N+1].Descriptor[0]!=9))
      return std::nullopt;
  }
  // Do not hardcode the fixture's arithmetic/move census: each such real
  // instruction is independently decoded below, and the external typed owner
  // is responsible for its exact SSA/provenance relation.
  for(const auto &[O,N]:std::array<std::pair<size_t,unsigned>,8>{{
      {0,4},{1,1},{8,1},{9,2},{10,1},{11,1},{12,1},{13,1}}})
    if(Census[O]!=N)return std::nullopt;
  const std::array<uint8_t,5> Tail{{10,11,12,9,13}};
  for(size_t I=0;I<Tail.size();++I)
    if(Input.Steps[Input.Steps.size()-Tail.size()+I].Descriptor[0]!=Tail[I])return std::nullopt;

  const size_t Count=Input.Starts.back()+Input.Counts.back();
  if(!Count||Count>32)return std::nullopt;
  std::array<std::optional<PhysicalRow>,32> Slots;
  for(const auto &Step:Input.Steps) {
    if(Step.Native>=Count||Slots[Step.Native])return std::nullopt;
    auto Row=globalCopyPhysicalRow(Step);if(!Row)return std::nullopt;
    Slots[Step.Native]=std::move(*Row);
  }
  if(Slots[Count-1])return std::nullopt;
  Slots[Count-1]=PhysicalRow{"S_ENDPGM_vi",{},{0},0,4,{},{},M::None,0,B::Return};
  GlobalCopyRelation Result;
  for(size_t I=0;I<Count;++I) {
    if(!Slots[I])return std::nullopt;
    for(const auto &R:Slots[I]->Registers) {
      SmallVector<StringRef,2> Parts;StringRef(R).split(Parts,'_');
      for(StringRef Part:Parts) {
        if(Part=="EXEC"||Part=="VCC"||Part=="SCC")continue;
        bool IsV=Part.consume_front("VGPR");bool IsS=!IsV&&Part.consume_front("SGPR");
        unsigned N=0;if((!IsV&&!IsS)||Part.getAsInteger(10,N)||N>63)return std::nullopt;
        if(IsV)Result.Vgprs=std::max(Result.Vgprs,N+1);
        else Result.Sgprs=std::max(Result.Sgprs,N+1);
      }
    }
    Result.Rows.push_back(std::move(*Slots[I]));
  }
  return Result;
}
std::optional<std::string> globalCopyPhysicalRelation(const GlobalCopyInput &Input,
    const GlobalCopyRelation &Expected,const PhysicalMachineEffectEvidence &E,
    ArrayRef<uint8_t> Payload) {
  if(auto Failure=physicalRelationRows(Input.Entry,Expected.Rows,Input.Starts,Input.Counts,Input.Edges,E,Payload))
    return Failure;
  for(const auto &Step:Input.Steps) {
    const auto &Encoding=E.Instructions[Step.Native].Encoding;
    const auto &D=Step.Descriptor;
    if(D[0]==1||D[0]==9) {
      const uint32_t ExpectedWord=D[0]==1?0xbf8cc07fU:0xbf8c0f70U;
      if(Encoding.size()!=4||support::endian::read32le(Encoding.data())!=ExpectedWord)
        return std::string("canonical exact load/store wait encoding");
    }
    if(D[0]==8) {
      // Pinned f58b06d FLAT_Real: op0x14, global segment, no offset/cache/acc,
      // disabled SGPR address0x7f, vaddr low byte and vdst high byte.
      const uint32_t High=(static_cast<uint32_t>(D[1])<<24)|0x007f0000U|D[2];
      if(Encoding.size()!=8||support::endian::read32le(Encoding.data())!=0xdc508000U||
          support::endian::read32le(Encoding.data()+4)!=High)
        return std::string("global-copy exact global-load encoding");
    }
    if(D[0]==12) {
      // Pinned GLOBAL_STORE_DWORD_vi: off SGPR, zero offset/cache bits,
      // only the already-matched address/data VGPR fields vary.
      const uint32_t High=0x007f0000U|(static_cast<uint32_t>(D[3])<<8)|D[2];
      if(Encoding.size()!=8||support::endian::read32le(Encoding.data())!=0xdc708000U||
          support::endian::read32le(Encoding.data()+4)!=High)
        return std::string("canonical exact global-store encoding");
    }
  }
  if(support::endian::read32le(E.Instructions.back().Encoding.data())!=0xbf810000U)
    return std::string("canonical exact terminal encoding");
  return std::nullopt;
}
