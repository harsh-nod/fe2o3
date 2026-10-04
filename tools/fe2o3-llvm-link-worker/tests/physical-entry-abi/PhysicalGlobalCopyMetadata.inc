// Distinct four-slot ABI. Reuses bounded ELF/metadata vocabulary, not V20 admission.
std::optional<PhysicalMetadata> globalCopyMetadata(ArrayRef<uint8_t> Payload,StringRef Symbol) {
  if(Payload.empty() || Payload.size()>64*1024) return std::nullopt;
  auto Object=object::ObjectFile::createObjectFile(MemoryBufferRef(
    StringRef(reinterpret_cast<const char*>(Payload.data()),Payload.size()),"<physical-body>"));
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
  if(physicalString(Root,"amdhsa.target")!=std::optional<StringRef>("amdgcn-amd-amdhsa--gfx942:xnack-"))
    return std::nullopt;
  auto K=Root.find("amdhsa.kernels");
  if(K==Root.end() || !K->second.isArray() || K->second.getArray().size()!=1 ||
      !K->second.getArray()[0].isMap()) return std::nullopt;
  auto &Kernel=K->second.getArray()[0].getMap();
  if(physicalString(Kernel,".name")!=std::optional<StringRef>(Symbol) ||
      physicalString(Kernel,".symbol")!=std::optional<StringRef>(Symbol.str()+".kd"))
    return std::nullopt;
  for(const auto &[Key,Value]:std::array<std::pair<StringRef,uint64_t>,6>{{
      {".kernarg_segment_size",288},{".kernarg_segment_align",8},
      {".group_segment_fixed_size",0},{".private_segment_fixed_size",0},
      {".wavefront_size",64},{".max_flat_workgroup_size",64}}})
    if(physicalUnsigned(Kernel,Key)!=std::optional<uint64_t>(Value)) return std::nullopt;
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
  PhysicalMetadata Result;
  for(const auto &[Key,Place]:std::array<std::pair<StringRef,uint64_t*>,5>{{
      {".sgpr_count",&Result.Sgprs},{".vgpr_count",&Result.Vgprs},{".agpr_count",&Result.Agprs},
      {".sgpr_spill_count",&Result.SgprSpills},{".vgpr_spill_count",&Result.VgprSpills}}}) {
    auto V=physicalUnsigned(Kernel,Key); if(!V || *V>1024) return std::nullopt; *Place=*V;
  }
  auto Args=Kernel.find(".args");
  // Expected exact absence-attribute roster; fresh O0/O3 qualification remains required.
  const size_t HiddenCount=13;
  if(Args==Kernel.end() || !Args->second.isArray() || Args->second.getArray().size()!=4+HiddenCount)
    return std::nullopt;
  constexpr std::array<uint64_t,4> Offsets{{0,8,16,24}},Sizes{{8,8,8,8}};
  constexpr std::array<StringRef,4> Names{{"input_data","input_length","output_data","output_length"}};
  struct Hidden { uint64_t Offset,Size; StringRef Kind; };
  constexpr std::array<Hidden,13> H{{
    {32,4,"hidden_block_count_x"},{36,4,"hidden_block_count_y"},{40,4,"hidden_block_count_z"},
    {44,2,"hidden_group_size_x"},{46,2,"hidden_group_size_y"},{48,2,"hidden_group_size_z"},
    {50,2,"hidden_remainder_x"},{52,2,"hidden_remainder_y"},{54,2,"hidden_remainder_z"},
    {72,8,"hidden_global_offset_x"},{80,8,"hidden_global_offset_y"},{88,8,"hidden_global_offset_z"},
    {96,2,"hidden_grid_dims"}}};
  size_t Ordinal=0; uint64_t PreviousEnd=0;
  for(auto &Node:Args->second.getArray()) {
    if(!Node.isMap()) return std::nullopt;
    auto &A=Node.getMap(); auto Offset=physicalUnsigned(A,".offset"),Size=physicalUnsigned(A,".size");
    auto Kind=physicalString(A,".value_kind");
    if(!Offset || !Size || !Kind || *Offset<PreviousEnd || *Size==0 ||
        *Offset>288 || *Size>288-*Offset) return std::nullopt;
    PreviousEnd=*Offset+*Size;
    PhysicalArgument Row{*Offset,*Size,"",Kind->str(),""};
    if(Ordinal<4) {
      auto Name=physicalString(A,".name");
      if(!Name || *Name!=Names[Ordinal] || *Offset!=Offsets[Ordinal] || *Size!=Sizes[Ordinal] ||
          *Kind!=((Ordinal==0||Ordinal==2)?"global_buffer":"by_value")) return std::nullopt;
      Row.Name=Name->str();
      if(Ordinal==0||Ordinal==2) {
        if(physicalString(A,".address_space")!=std::optional<StringRef>("global")) return std::nullopt;
        Row.Address="global";
      } else if(A.find(".address_space")!=A.end()) return std::nullopt;
    } else {
      const auto &Expected=H[Ordinal-4];
      if(*Offset!=Expected.Offset || *Size!=Expected.Size || *Kind!=Expected.Kind ||
          A.find(".name")!=A.end() || A.find(".address_space")!=A.end()) return std::nullopt;
    }
    Result.Arguments.push_back(std::move(Row)); ++Ordinal;
  }
  return Result;
}
