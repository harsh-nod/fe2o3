// Private test-only readback while the unchanged parser still owns its actual
// metadata document. This is a closed ABI prefix join, not a second ELF parser.
bool transportMetadataShape(msgpack::MapDocNode &M) {
  const auto number=[&](msgpack::MapDocNode &A,StringRef K,uint64_t Expected) {
    const auto I=A.find(K);if(I==A.end())return false;
    if(I->second.getKind()==msgpack::Type::UInt)
      return I->second.getUInt()==Expected;
    return I->second.getKind()==msgpack::Type::Int &&
           I->second.getInt()>=0 && uint64_t(I->second.getInt())==Expected;
  };
  const auto text=[&](msgpack::MapDocNode &A,StringRef K,StringRef Expected) {
    const auto I=A.find(K);
    return I!=A.end() && I->second.isString() && I->second.getString()==Expected;
  };
  if(M.size()>64 || !number(M,".kernarg_segment_size",288) ||
     !number(M,".kernarg_segment_align",8) ||
     !number(M,".wavefront_size",64) ||
     !number(M,".max_flat_workgroup_size",64))return false;
  auto WG=M.find(".reqd_workgroup_size");
  if(WG==M.end() || !WG->second.isArray() || WG->second.getArray().size()!=3)
    return false;
  for(size_t I=0;I<3;++I) {
    const auto &N=WG->second.getArray()[I];
    if(N.getKind()!=msgpack::Type::UInt || N.getUInt()!=(I?1:64))return false;
  }
  auto Args=M.find(".args");
  if(Args==M.end() || !Args->second.isArray() ||
     Args->second.getArray().size()<5 || Args->second.getArray().size()>24)
    return false;
  constexpr std::array<uint64_t,5> Offsets{0,8,16,20,24},Sizes{8,8,4,4,4};
  constexpr std::array<StringLiteral,5> Names{
    "arg0.data","arg0.len","arg1","arg2","arg3"};
  for(size_t I=0;I<5;++I) {
    auto &N=Args->second.getArray()[I];if(!N.isMap())return false;
    auto &A=N.getMap();
    if(A.size()>16)return false;
    if(!number(A,".offset",Offsets[I]) || !number(A,".size",Sizes[I]) ||
       !text(A,".name",Names[I]) ||
       !text(A,".value_kind",I==0?"global_buffer":"by_value"))return false;
    if(I==0) {if(!text(A,".address_space","global"))return false;}
    else if(A.find(".address_space")!=A.end())return false;
  }
  // Later ABI rows cannot alias the explicit prefix. Their complete hidden
  // service/resource grammar remains the unchanged independent observer's job.
  uint64_t End=28;
  for(size_t I=5;I<Args->second.getArray().size();++I) {
    auto &N=Args->second.getArray()[I];if(!N.isMap())return false;
    auto &A=N.getMap();
    if(A.size()>16)return false;
    auto O=A.find(".offset"),S=A.find(".size");
    if(O==A.end() || S==A.end() ||
       O->second.getKind()!=msgpack::Type::UInt ||
       S->second.getKind()!=msgpack::Type::UInt)return false;
    const auto Offset=O->second.getUInt(),Size=S->second.getUInt();
    if(Offset<End || Offset>288 || Size==0 || Size>288-Offset)return false;
    End=Offset+Size;
  }
  return true;
}
