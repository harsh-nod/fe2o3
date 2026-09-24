// Adapted from the pinned existing physical-entry metadata fixture; no shared edit.
// Bounded independent view of the actual selected ELF metadata and descriptor.
// The existing worker validates the same payload first. No worker wire change.
struct FixtureArgument { uint64_t Offset=0,Size=0; std::string Name,Kind,Address; };
struct FixtureMetadata {
  std::vector<FixtureArgument> Arguments;
  uint64_t Sgprs=0,Vgprs=0,Agprs=0,SgprSpills=0,VgprSpills=0;
};
std::optional<uint64_t> fixtureUnsigned(msgpack::MapDocNode &Map,StringRef Key) {
  auto I=Map.find(Key);
  if(I==Map.end()) return std::nullopt;
  if(I->second.getKind()==msgpack::Type::UInt) return I->second.getUInt();
  if(I->second.getKind()==msgpack::Type::Int && I->second.getInt()>=0)
    return static_cast<uint64_t>(I->second.getInt());
  return std::nullopt;
}
std::optional<StringRef> fixtureString(msgpack::MapDocNode &Map,StringRef Key) {
  auto I=Map.find(Key); if(I==Map.end() || !I->second.isString()) return std::nullopt;
  return I->second.getString();
}
std::optional<FixtureMetadata> fixtureMetadata(ArrayRef<uint8_t> Payload,StringRef Symbol) {
  if(Payload.empty() || Payload.size()>ArtifactCap) return std::nullopt;
  auto Object=object::ObjectFile::createObjectFile(MemoryBufferRef(
    StringRef(reinterpret_cast<const char*>(Payload.data()),Payload.size()),"<fixture-body>"));
  if(!Object) { consumeError(Object.takeError()); return std::nullopt; }
  const auto *Elf=dyn_cast<object::ELFObjectFile<object::ELF64LE>>(Object->get());
  if(!Elf) return std::nullopt;
  const auto &File=Elf->getELFFile();
  auto Sections=File.sections();
  if(!Sections) { consumeError(Sections.takeError()); return std::nullopt; }
  if(Sections->size()>128) return std::nullopt;
  std::optional<StringRef> Blob;
  for(const auto &Section:*Sections) {
    if(Section.sh_type!=ELF::SHT_NOTE) continue;
    Error ErrorValue=Error::success();
    for(const auto Note:File.notes(Section,ErrorValue)) {
      if(Note.getName()!="AMDGPU" || Note.getType()!=ELF::NT_AMDGPU_METADATA) continue;
      if(Section.sh_addralign!=4 || Blob) { consumeError(std::move(ErrorValue)); return std::nullopt; }
      Blob=Note.getDescAsStringRef(4);
    }
    if(ErrorValue) { consumeError(std::move(ErrorValue)); return std::nullopt; }
  }
  if(!Blob || Blob->empty() || Blob->size()>16*1024) return std::nullopt;
  msgpack::Document Doc;
  if(!Doc.readFromBlob(*Blob,true) || !Doc.getRoot().isArray() ||
      Doc.getRoot().getArray().size()!=1) return std::nullopt;
  auto &RootNode=Doc.getRoot().getArray()[0];
  AMDGPU::HSAMD::V3::MetadataVerifier Verifier(true);
  if(!Verifier.verify(RootNode) || !RootNode.isMap()) return std::nullopt;
  auto &Root=RootNode.getMap();
  if(fixtureString(Root,"amdhsa.target")!=std::optional<StringRef>("amdgcn-amd-amdhsa--gfx950:xnack-"))
    return std::nullopt;
  auto K=Root.find("amdhsa.kernels");
  if(K==Root.end() || !K->second.isArray() || K->second.getArray().size()!=1 ||
      !K->second.getArray()[0].isMap()) return std::nullopt;
  auto &Kernel=K->second.getArray()[0].getMap();
  if(fixtureString(Kernel,".name")!=std::optional<StringRef>(Symbol) ||
      fixtureString(Kernel,".symbol")!=std::optional<StringRef>(Symbol.str()+".kd"))
    return std::nullopt;
  for(const auto &[Key,Value]:std::array<std::pair<StringRef,uint64_t>,6>{{
      {".kernarg_segment_size",264},{".kernarg_segment_align",8},
      {".group_segment_fixed_size",0},{".private_segment_fixed_size",0},
      {".wavefront_size",64},{".max_flat_workgroup_size",64}}})
    if(fixtureUnsigned(Kernel,Key)!=std::optional<uint64_t>(Value)) return std::nullopt;
  auto Dynamic=Kernel.find(".uses_dynamic_stack");
  if(Dynamic==Kernel.end() || Dynamic->second.getKind()!=msgpack::Type::Boolean ||
      Dynamic->second.getBool()) return std::nullopt;
  auto WG=Kernel.find(".reqd_workgroup_size");
  if(WG==Kernel.end() || !WG->second.isArray() || WG->second.getArray().size()!=3)
    return std::nullopt;
  for(size_t I=0;I<3;++I) {
    auto &N=WG->second.getArray()[I];
    if(N.getKind()!=msgpack::Type::UInt || N.getUInt()!=(I==0?64:1)) return std::nullopt;
  }
  for (StringRef Key : {".max_num_workgroups_x", ".max_num_workgroups_y", ".max_num_workgroups_z"})
    if(fixtureUnsigned(Kernel,Key)!=std::optional<uint64_t>(1)) return std::nullopt;
  if(Kernel.find(".cluster_dims")!=Kernel.end()) return std::nullopt;
  FixtureMetadata Result;
  for(const auto &[Key,Place]:std::array<std::pair<StringRef,uint64_t*>,5>{{
      {".sgpr_count",&Result.Sgprs},{".vgpr_count",&Result.Vgprs},{".agpr_count",&Result.Agprs},
      {".sgpr_spill_count",&Result.SgprSpills},{".vgpr_spill_count",&Result.VgprSpills}}}) {
    auto V=fixtureUnsigned(Kernel,Key); if(!V || *V>1024) return std::nullopt; *Place=*V;
  }
  auto Args=Kernel.find(".args");
  // Exact supported absence attributes suppress all six unused runtime services at both O0/O3.
  const size_t HiddenCount=13;
  if(Args==Kernel.end() || !Args->second.isArray() || Args->second.getArray().size()!=1+HiddenCount)
    return std::nullopt;
  constexpr std::array<uint64_t,1> Offsets{{0}},Sizes{{8}};
  constexpr std::array<StringRef,1> Names{{"out"}};
  struct Hidden { uint64_t Offset,Size; StringRef Kind; };
  constexpr std::array<Hidden,13> H{{
    {8,4,"hidden_block_count_x"},{12,4,"hidden_block_count_y"},{16,4,"hidden_block_count_z"},
    {20,2,"hidden_group_size_x"},{22,2,"hidden_group_size_y"},{24,2,"hidden_group_size_z"},
    {26,2,"hidden_remainder_x"},{28,2,"hidden_remainder_y"},{30,2,"hidden_remainder_z"},
    {48,8,"hidden_global_offset_x"},{56,8,"hidden_global_offset_y"},{64,8,"hidden_global_offset_z"},
    {72,2,"hidden_grid_dims"}}};
  size_t Ordinal=0; uint64_t PreviousEnd=0;
  for(auto &Node:Args->second.getArray()) {
    if(!Node.isMap()) return std::nullopt;
    auto &A=Node.getMap(); auto Offset=fixtureUnsigned(A,".offset"),Size=fixtureUnsigned(A,".size");
    auto Kind=fixtureString(A,".value_kind");
    if(!Offset || !Size || !Kind || *Offset<PreviousEnd || *Size==0 ||
        *Offset>264 || *Size>264-*Offset) return std::nullopt;
    PreviousEnd=*Offset+*Size;
    FixtureArgument Row{*Offset,*Size,"",Kind->str(),""};
    if(Ordinal<1) {
      auto Name=fixtureString(A,".name");
      if(!Name || *Name!=Names[Ordinal] || *Offset!=Offsets[Ordinal] || *Size!=Sizes[Ordinal] ||
          *Kind!=(Ordinal==0?"global_buffer":"by_value")) return std::nullopt;
      Row.Name=Name->str();
      if(Ordinal==0) {
        if(fixtureString(A,".address_space")!=std::optional<StringRef>("global")) return std::nullopt;
        Row.Address="global";
      } else if(A.find(".address_space")!=A.end()) return std::nullopt;
    } else {
      const auto &Expected=H[Ordinal-1];
      if(*Offset!=Expected.Offset || *Size!=Expected.Size || *Kind!=Expected.Kind ||
          A.find(".name")!=A.end() || A.find(".address_space")!=A.end()) return std::nullopt;
    }
    Result.Arguments.push_back(std::move(Row)); ++Ordinal;
  }
  return Result;
}
json::Array fixtureArgumentsJson(const FixtureMetadata &Metadata) {
  json::Array Rows;
  for(const auto &A:Metadata.Arguments)
    Rows.emplace_back(json::Object{{"offset",A.Offset},{"size",A.Size},
      {"name",A.Name},{"kind",A.Kind},{"address_space",A.Address}});
  return Rows;
}
