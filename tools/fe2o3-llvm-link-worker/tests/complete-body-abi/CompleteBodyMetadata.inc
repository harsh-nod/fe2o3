// Bounded independent view of the actual selected ELF metadata and descriptor.
// The existing worker validates the same payload first. No worker wire change.
struct CompleteArgument { uint64_t Offset=0,Size=0; std::string Name,Kind,Address; };
struct CompleteMetadata {
  std::vector<CompleteArgument> Arguments;
  uint64_t Sgprs=0,Vgprs=0,Agprs=0,SgprSpills=0,VgprSpills=0;
};
std::optional<uint64_t> completeUnsigned(msgpack::MapDocNode &Map,StringRef Key) {
  auto I=Map.find(Key);
  if(I==Map.end()) return std::nullopt;
  if(I->second.getKind()==msgpack::Type::UInt) return I->second.getUInt();
  if(I->second.getKind()==msgpack::Type::Int && I->second.getInt()>=0)
    return static_cast<uint64_t>(I->second.getInt());
  return std::nullopt;
}
std::optional<StringRef> completeString(msgpack::MapDocNode &Map,StringRef Key) {
  auto I=Map.find(Key); if(I==Map.end() || !I->second.isString()) return std::nullopt;
  return I->second.getString();
}
std::optional<CompleteMetadata> completeMetadata(ArrayRef<uint8_t> Payload,StringRef Symbol,
    OptimizationLevel Level) {
  if(Payload.empty() || Payload.size()>64*1024) return std::nullopt;
  auto Object=object::ObjectFile::createObjectFile(MemoryBufferRef(
    StringRef(reinterpret_cast<const char*>(Payload.data()),Payload.size()),"<complete-body>"));
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
  if(completeString(Root,"amdhsa.target")!=std::optional<StringRef>("amdgcn-amd-amdhsa--gfx942:xnack-"))
    return std::nullopt;
  auto K=Root.find("amdhsa.kernels");
  if(K==Root.end() || !K->second.isArray() || K->second.getArray().size()!=1 ||
      !K->second.getArray()[0].isMap()) return std::nullopt;
  auto &Kernel=K->second.getArray()[0].getMap();
  if(completeString(Kernel,".name")!=std::optional<StringRef>(Symbol) ||
      completeString(Kernel,".symbol")!=std::optional<StringRef>(Symbol.str()+".kd"))
    return std::nullopt;
  for(const auto &[Key,Value]:std::array<std::pair<StringRef,uint64_t>,6>{{
      {".kernarg_segment_size",288},{".kernarg_segment_align",8},
      {".group_segment_fixed_size",0},{".private_segment_fixed_size",0},
      {".wavefront_size",64},{".max_flat_workgroup_size",64}}})
    if(completeUnsigned(Kernel,Key)!=std::optional<uint64_t>(Value)) return std::nullopt;
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
  CompleteMetadata Result;
  for(const auto &[Key,Place]:std::array<std::pair<StringRef,uint64_t*>,5>{{
      {".sgpr_count",&Result.Sgprs},{".vgpr_count",&Result.Vgprs},{".agpr_count",&Result.Agprs},
      {".sgpr_spill_count",&Result.SgprSpills},{".vgpr_spill_count",&Result.VgprSpills}}}) {
    auto V=completeUnsigned(Kernel,Key); if(!V || *V>1024) return std::nullopt; *Place=*V;
  }
  auto Args=Kernel.find(".args");
  // O3 omits six unused runtime-service descriptors; their reserved kernarg
  // extent remains 256 bytes. Require the exact roster for this optimization,
  // never an arbitrary subset, and keep all six explicit slots mandatory.
  const size_t HiddenCount=Level==OptimizationLevel::O0?19:13;
  if(Args==Kernel.end() || !Args->second.isArray() || Args->second.getArray().size()!=6+HiddenCount)
    return std::nullopt;
  constexpr std::array<uint64_t,6> Offsets{{0,8,16,20,24,28}},Sizes{{8,8,4,4,4,4}};
  constexpr std::array<StringRef,6> Names{{"data","length","a","b","c","selector"}};
  struct Hidden { uint64_t Offset,Size; StringRef Kind; };
  constexpr std::array<Hidden,19> H{{
    {32,4,"hidden_block_count_x"},{36,4,"hidden_block_count_y"},{40,4,"hidden_block_count_z"},
    {44,2,"hidden_group_size_x"},{46,2,"hidden_group_size_y"},{48,2,"hidden_group_size_z"},
    {50,2,"hidden_remainder_x"},{52,2,"hidden_remainder_y"},{54,2,"hidden_remainder_z"},
    {72,8,"hidden_global_offset_x"},{80,8,"hidden_global_offset_y"},{88,8,"hidden_global_offset_z"},
    {96,2,"hidden_grid_dims"},{112,8,"hidden_hostcall_buffer"},{120,8,"hidden_multigrid_sync_arg"},
    {128,8,"hidden_heap_v1"},{136,8,"hidden_default_queue"},{144,8,"hidden_completion_action"},
    {232,8,"hidden_queue_ptr"}}};
  size_t Ordinal=0; uint64_t PreviousEnd=0;
  for(auto &Node:Args->second.getArray()) {
    if(!Node.isMap()) return std::nullopt;
    auto &A=Node.getMap(); auto Offset=completeUnsigned(A,".offset"),Size=completeUnsigned(A,".size");
    auto Kind=completeString(A,".value_kind");
    if(!Offset || !Size || !Kind || *Offset<PreviousEnd || *Size==0 ||
        *Offset>288 || *Size>288-*Offset) return std::nullopt;
    PreviousEnd=*Offset+*Size;
    CompleteArgument Row{*Offset,*Size,"",Kind->str(),""};
    if(Ordinal<6) {
      auto Name=completeString(A,".name");
      if(!Name || *Name!=Names[Ordinal] || *Offset!=Offsets[Ordinal] || *Size!=Sizes[Ordinal] ||
          *Kind!=(Ordinal==0?"global_buffer":"by_value")) return std::nullopt;
      Row.Name=Name->str();
      if(Ordinal==0) {
        if(completeString(A,".address_space")!=std::optional<StringRef>("global")) return std::nullopt;
        Row.Address="global";
      } else if(A.find(".address_space")!=A.end()) return std::nullopt;
    } else {
      const auto &Expected=H[Ordinal-6];
      if(*Offset!=Expected.Offset || *Size!=Expected.Size || *Kind!=Expected.Kind ||
          A.find(".name")!=A.end() || A.find(".address_space")!=A.end()) return std::nullopt;
    }
    Result.Arguments.push_back(std::move(Row)); ++Ordinal;
  }
  return Result;
}
json::Array completeArgumentsJson(const CompleteMetadata &Metadata) {
  json::Array Rows;
  for(const auto &A:Metadata.Arguments)
    Rows.emplace_back(json::Object{{"offset",A.Offset},{"size",A.Size},
      {"name",A.Name},{"kind",A.Kind},{"address_space",A.Address}});
  return Rows;
}
struct CompleteDescriptor {
  uint64_t Offset=0; uint32_t Rsrc1=0,Rsrc2=0,Rsrc3=0,VgprCapacity=0,SgprCapacity=0,Boundary=0;
  uint16_t CodeProperties=0;
  std::array<uint8_t,32> Digest{};
};
std::optional<CompleteDescriptor> completeDescriptor(const CompletePlan &P,
    const PhysicalMachineEffectEvidence &E,ArrayRef<uint8_t> Payload,StringRef Symbol,
    const CompleteMetadata &M) {
  auto O=object::ObjectFile::createObjectFile(MemoryBufferRef(
    StringRef(reinterpret_cast<const char*>(Payload.data()),Payload.size()),"<complete-descriptor>"));
  if(!O) { consumeError(O.takeError()); return std::nullopt; }
  std::optional<CompleteDescriptor> Result; size_t Count=0;
  for(const auto &S:(*O)->symbols()) {
    if(++Count>4096) return std::nullopt;
    auto Name=S.getName(); if(!Name) { consumeError(Name.takeError()); return std::nullopt; }
    if(*Name!=Symbol.str()+".kd") continue;
    if(Result) return std::nullopt;
    auto Sec=S.getSection(); if(!Sec) { consumeError(Sec.takeError()); return std::nullopt; }
    if(*Sec==(*O)->section_end()) return std::nullopt;
    auto Content=(*Sec)->getContents();
    auto Address=S.getAddress();
    if(!Content) { consumeError(Content.takeError()); if(!Address)consumeError(Address.takeError()); return std::nullopt; }
    if(!Address) { consumeError(Address.takeError()); return std::nullopt; }
    if(*Address<(*Sec)->getAddress()) return std::nullopt;
    const auto Relative=*Address-(*Sec)->getAddress();
    if(Relative>Content->size() || Content->size()-Relative<64) return std::nullopt;
    const auto *B=reinterpret_cast<const uint8_t*>(Content->data())+Relative;
    const uintptr_t Base=reinterpret_cast<uintptr_t>(Payload.data()),Begin=reinterpret_cast<uintptr_t>(B);
    if(Begin<Base || Begin-Base>Payload.size() || Payload.size()-(Begin-Base)<64) return std::nullopt;
    CompleteDescriptor D; D.Offset=Begin-Base; D.Digest=SHA256::hash(ArrayRef<uint8_t>(B,64));
    if(E.Entries.size()!=1 || D.Digest!=E.Entries[0].DescriptorIdentity) return std::nullopt;
    using namespace llvm::amdhsa;
    if(support::endian::read32le(B+GROUP_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
        support::endian::read32le(B+PRIVATE_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
        support::endian::read32le(B+KERNARG_SIZE_OFFSET)!=288 ||
        (support::endian::read16le(B+KERNEL_CODE_PROPERTIES_OFFSET)&KERNEL_CODE_PROPERTY_USES_DYNAMIC_STACK))
      return std::nullopt;
    D.Rsrc3=support::endian::read32le(B+44); D.Rsrc1=support::endian::read32le(B+48);
    D.Rsrc2=support::endian::read32le(B+52);
    D.CodeProperties=support::endian::read16le(B+KERNEL_CODE_PROPERTIES_OFFSET);
    D.VgprCapacity=((D.Rsrc1&0x3f)+1)*8; D.Boundary=((D.Rsrc3&0x3f)+1)*4;
    const uint32_t SgprBlocks=(D.Rsrc1>>6)&15;
    if(SgprBlocks>13) return std::nullopt;
    D.SgprCapacity=(SgprBlocks+1)*8;
    const uint32_t Required=1U+std::max({P.Scratch,P.Output,P.Inputs[0],P.Inputs[1],P.Inputs[2]});
    if(D.VgprCapacity<Required || D.Boundary<Required || D.VgprCapacity<D.Boundary ||
        D.VgprCapacity<M.Vgprs || D.SgprCapacity<23 || D.SgprCapacity<M.Sgprs) return std::nullopt;
    // Every decoded ordinary VGPR/SGPR component must fit encoded capacity.
    for(const auto &I:E.Instructions) for(const auto &Operand:I.Operands) {
      if(Operand.Kind!=PhysicalMachineOperandKind::Register) continue;
      SmallVector<StringRef,8> Parts; StringRef(Operand.Register).split(Parts,'_');
      if(Parts.size()>16) return std::nullopt;
      for(StringRef Part:Parts) {
        const bool V=Part.consume_front("VGPR"); const bool S=!V && Part.consume_front("SGPR");
        if(!V && !S) continue;
        unsigned Index=0;
        if(Part.getAsInteger(10,Index) || Index>=(V?D.VgprCapacity:D.SgprCapacity)) return std::nullopt;
      }
    }
    Result=D;
  }
  return Result;
}
json::Array completeMetadataNegatives(ArrayRef<uint8_t> Original,StringRef Symbol,
    OptimizationLevel Level) {
  // Byte mutation locates exact MessagePack field sequence from this current
  // payload. It preserves ELF layout and all outer lengths; fresh parser reads
  // the changed payload, not a stale digest-only refusal.
  const std::array<std::pair<StringRef,StringRef>,2> TextChanges{{
    {"selector","selectox"},{Level==OptimizationLevel::O0?"hidden_queue_ptr":"hidden_grid_dims",
      Level==OptimizationLevel::O0?"hidden_queue_ptx":"hidden_grid_dimx"}}};
  json::Array Results;
  for(const auto &[From,To]:TextChanges) {
    std::vector<uint8_t> Changed(Original.begin(),Original.end());
    auto At=std::search(Changed.begin(),Changed.end(),From.begin(),From.end());
    require(At!=Changed.end() && std::search(At+From.size(),Changed.end(),From.begin(),From.end())==Changed.end(),
      "metadata mutation anchor absent or ambiguous");
    std::copy(To.begin(),To.end(),At);
    require(!completeMetadata(Changed,Symbol,Level),"changed explicit/hidden metadata accepted");
    Results.emplace_back(json::Object{{"mutation",From},{"phase","explicit_hidden_metadata_relation"},
      {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"accepted",false}});
  }
  // Exact selector map prefix: .name=selector, .offset=28. No blanket byte28 search.
  const std::array<uint8_t,25> Prefix{{0xa5,'.','n','a','m','e',0xa8,'s','e','l','e','c','t','o','r',
    0xa7,'.','o','f','f','s','e','t',0x1c,0xa5}};
  std::vector<uint8_t> Changed(Original.begin(),Original.end());
  auto At=std::search(Changed.begin(),Changed.end(),Prefix.begin(),Prefix.end());
  require(At!=Changed.end() && std::search(At+Prefix.size(),Changed.end(),Prefix.begin(),Prefix.end())==Changed.end(),
    "selector offset byte anchor absent or ambiguous");
  *(At+23)=0x18;
  require(!completeMetadata(Changed,Symbol,Level),"changed selector slot accepted");
  Results.emplace_back(json::Object{{"mutation","selector_offset_28_to_24"},
    {"phase","explicit_hidden_metadata_relation"},{"changed_payload_sha256",hex(SHA256::hash(Changed))},
    {"accepted",false}});
  return Results;
}
json::Array completeDescriptorNegatives(const CompletePlan &P,const BuiltFixture &Built,
    StringRef Symbol,const CompleteDescriptor &D,OptimizationLevel Level) {
  const auto &Original=Built.ResponseValue.LinkedOutput->Bytes;
  json::Array Rows;
  for(bool Vgpr:{true,false}) {
    auto Changed=Original;
    const uint32_t Mask=Vgpr?0x3fU:0x3c0U;
    support::endian::write32le(Changed.data()+D.Offset+48,D.Rsrc1&~Mask);
    auto E=completeAnalyze(Changed,Symbol);
    std::string Phase,Reason;
    if(!E) { Phase="existing_machine_decoder"; Reason=toString(E.takeError()); }
    else {
      auto M=completeMetadata(Changed,Symbol,Level);
      require(M.has_value(),"capacity mutation unexpectedly changed metadata");
      require(!completeDescriptor(P,*E,Changed,Symbol,*M),"changed descriptor capacity accepted");
      Phase="descriptor_capacity_relation"; Reason="encoded capacity cannot cover required/current uses";
    }
    require(Reason.size()<=4096,"descriptor refusal diagnostic cap");
    Rows.emplace_back(json::Object{{"mutation",Vgpr?"vgpr_capacity":"sgpr_capacity"},
      {"phase",Phase},{"reason",Reason},{"changed_payload_sha256",hex(SHA256::hash(Changed))},
      {"fresh_payload_hash",true},{"accepted",false}});
  }
  return Rows;
}
