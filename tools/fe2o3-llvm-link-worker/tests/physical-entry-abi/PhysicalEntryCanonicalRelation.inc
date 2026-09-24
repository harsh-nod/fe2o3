// Independent descriptor -> existing MC expectation rows, not an assembler or evaluator.
std::string canonicalS(unsigned N) {return "SGPR"+std::to_string(N);}
std::string canonicalV(unsigned N) {return "VGPR"+std::to_string(N);}
std::string canonicalSP(unsigned N) {return canonicalS(N)+"_"+canonicalS(N+1);}
std::string canonicalVP(unsigned N) {return canonicalV(N)+"_"+canonicalV(N+1);}
std::optional<PhysicalRow> canonicalPhysicalRow(const CanonicalPhysicalStep &Step) {
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
    if(!SD(D)||!Pair(D)||A||B||(I!=0&&I!=8))return std::nullopt;
    return PhysicalRow{"S_LOAD_DWORDX2_IMM_vi",{canonicalSP(D),"SGPR0_SGPR1"},{I,0},1,8,{},{},M::Read,8};
  case 1:
    if(!SD(D)||A||B||(I!=16&&I!=20&&I!=24&&I!=28))return std::nullopt;
    return PhysicalRow{"S_LOAD_DWORD_IMM_vi",{canonicalS(D),"SGPR0_SGPR1"},{I,0},1,8,{},{},M::Read,4};
  case 2:
    if(D||A||B||I)return std::nullopt;
    return PhysicalRow{"S_WAITCNT_vi",{},{0xc07f},0,4,{},{}};
  case 3:
    if(!SD(D)||!S(A)||B||I!=6)return std::nullopt;
    return PhysicalRow{"S_LSHL_B32_vi",{canonicalS(D),canonicalS(A)},{6},1,4,{},{"SCC"}};
  case 4:
    if(!VD(D)||!S(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADD_U32_e32_gfx9",{canonicalV(D),canonicalS(A),canonicalV(B)},{},1,4,{"EXEC"},{}};
  case 5:
    if(!VD(D)||(A!=255&&!S(A))||B||I)return std::nullopt;
    if(A==255)return PhysicalRow{"V_MOV_B32_e32_vi",{canonicalV(D)},{0},1,4,{"EXEC"},{}};
    return PhysicalRow{"V_MOV_B32_e32_vi",{canonicalV(D),canonicalS(A)},{},1,4,{"EXEC"},{}};
  case 6:
    if(D||!S(A)||B||I)return std::nullopt;
    return PhysicalRow{"S_CMP_EQ_U32_vi",{canonicalS(A)},{0},0,4,{},{"SCC"}};
  case 9:
    if(!VD(D)||!Pair(D)||!Pair(A)||B||I!=2)return std::nullopt;
    return PhysicalRow{"V_LSHLREV_B64_vi",{canonicalVP(D),canonicalVP(A)},{2},1,8,{"EXEC"},{}};
  case 10:
    if(!VD(D)||!S(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADD_CO_U32_e32_gfx9",{canonicalV(D),canonicalS(A),canonicalV(B)},{},1,4,{"EXEC"},{"VCC"}};
  case 11:
    if(!VD(D)||!V(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"V_ADDC_CO_U32_e32_gfx9",{canonicalV(D),canonicalV(A),canonicalV(B)},{},1,4,{"EXEC","VCC"},{"VCC"}};
  case 12:
    if(D||!Pair(A)||!Pair(B)||I)return std::nullopt;
    return PhysicalRow{"V_CMP_GT_U64_e32_vi",{canonicalSP(A),canonicalVP(B)},{},0,4,{"EXEC"},{"VCC"}};
  case 13:
    if(!SD(D)||!Pair(D)||A||B||I)return std::nullopt;
    return PhysicalRow{"S_AND_SAVEEXEC_B64_vi",{canonicalSP(D),"VCC"},{},1,4,{"EXEC"},{"EXEC","SCC"}};
  case 14:
    if(D||!Pair(A)||!V(B)||I)return std::nullopt;
    return PhysicalRow{"GLOBAL_STORE_DWORD_vi",{canonicalVP(A),canonicalV(B)},{0,0},0,8,{"EXEC"},{},M::Write,4};
  case 15:
    if(D||A||B||I)return std::nullopt;
    return PhysicalRow{"S_WAITCNT_vi",{},{0x0f70},0,4,{},{}};
  case 16:
    if(D||!Pair(A)||B||I)return std::nullopt;
    return PhysicalRow{"S_MOV_B64_vi",{"EXEC",canonicalSP(A)},{},1,4,{},{}};
  default:return std::nullopt;
  }
}
struct CanonicalPhysicalRelation {
  std::vector<PhysicalRow> Rows;
  uint32_t Sgprs=3,Vgprs=1;
};
std::optional<CanonicalPhysicalRelation> canonicalPhysicalRows(const CanonicalPhysicalInput &Input) {
  using M=PhysicalMachineMemoryAccess;using B=PhysicalMachineBranchKind;
  if(Input.Starts.empty()||Input.Starts.size()>4)return std::nullopt;
  const size_t Count=Input.Starts.back()+Input.Counts.back();
  if(!Count||Count>64)return std::nullopt;
  std::array<std::optional<PhysicalRow>,64> Slots;
  for(const auto &Step:Input.Steps) {
    if(Step.Native>=Count||Slots[Step.Native])return std::nullopt;
    auto Row=canonicalPhysicalRow(Step);if(!Row)return std::nullopt;
    Slots[Step.Native]=std::move(*Row);
  }
  for(size_t Block=0;Block<Input.Starts.size();++Block) {
    const auto Encoding=Input.Encodings[Block];
    if(Encoding==3)continue;
    const auto Last=Input.Starts[Block]+Input.Counts[Block]-1;
    if(Last>=Count||Slots[Last])return std::nullopt;
    if((Encoding==1||Encoding==2)&&Input.Starts.size()!=4)return std::nullopt;
    if(Encoding==1)
      Slots[Last]=PhysicalRow{"S_CBRANCH_SCC1_vi",{},{},0,4,{"SCC"},{},M::None,0,
          B::ConditionalDirect,static_cast<int>(Input.Starts[2])};
    else if(Encoding==2)
      Slots[Last]=PhysicalRow{"S_BRANCH_vi",{},{},0,4,{},{},M::None,0,
          B::UnconditionalDirect,static_cast<int>(Input.Starts[3])};
    else if(Encoding==4)
      Slots[Last]=PhysicalRow{"S_ENDPGM_vi",{},{0},0,4,{},{},M::None,0,B::Return};
    else return std::nullopt;
  }
  CanonicalPhysicalRelation Result;
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
std::optional<std::string> canonicalPhysicalRelation(const CanonicalPhysicalInput &Input,
    const CanonicalPhysicalRelation &Expected,const PhysicalMachineEffectEvidence &E,
    ArrayRef<uint8_t> Payload) {
  if(auto Failure=physicalRelationRows(Input.Entry,Expected.Rows,Input.Starts,Input.Counts,Input.Edges,E,Payload))
    return Failure;
  for(const auto &Step:Input.Steps) {
    const auto &Encoding=E.Instructions[Step.Native].Encoding;
    const auto &D=Step.Descriptor;
    if(D[0]==2||D[0]==15) {
      const uint32_t ExpectedWord=D[0]==2?0xbf8cc07fU:0xbf8c0f70U;
      if(Encoding.size()!=4||support::endian::read32le(Encoding.data())!=ExpectedWord)
        return std::string("canonical exact load/store wait encoding");
    }
    if(D[0]==14) {
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
