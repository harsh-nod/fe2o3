// Independent ELF metadata/descriptor consistency. No physical-entry zero-scratch assumptions.
#include "CompositionResourceRelation.inc"
struct CompositionResources {
  uint64_t Private=0,Sgprs=0,Vgprs=0,Agprs=0,SgprSpills=0,VgprSpills=0;
  uint64_t ArchVgprs=0,AccumStart=0,HasDynSizedStack=0,HasRecursion=0;
  bool UsesDynamicStack=false;
  uint64_t DynamicMetadataOffset=0;
  std::array<uint64_t,2> StackSymbolOffsets{};
  uint32_t Rsrc1=0,Rsrc2=0,Rsrc3=0,SgprCapacity=0,VgprCapacity=0;
  uint16_t Properties=0;
  uint64_t DescriptorOffset=0;
  std::array<uint8_t,32> DescriptorDigest{};
  json::Array Arguments;
};
std::optional<uint64_t> compositionUnsigned(msgpack::MapDocNode &M,StringRef K) {
  auto I=M.find(K);if(I==M.end())return std::nullopt;
  if(I->second.getKind()==msgpack::Type::UInt)return I->second.getUInt();
  if(I->second.getKind()==msgpack::Type::Int && I->second.getInt()>=0)return uint64_t(I->second.getInt());
  return std::nullopt;
}
std::optional<StringRef> compositionString(msgpack::MapDocNode &M,StringRef K) {
  auto I=M.find(K);if(I==M.end() || !I->second.isString())return std::nullopt;return I->second.getString();
}
std::optional<CompositionResources> compositionMetadata(ArrayRef<uint8_t> Bytes,const CompositionRecord &R) {
  if(Bytes.empty() || Bytes.size()>PayloadByteLimit)return std::nullopt;
  auto O=object::ObjectFile::createObjectFile(MemoryBufferRef(
    StringRef(reinterpret_cast<const char*>(Bytes.data()),Bytes.size()),"<ordinary-composition>"));
  if(!O){consumeError(O.takeError());return std::nullopt;}
  const auto *Elf=dyn_cast<object::ELFObjectFile<object::ELF64LE>>(O->get());
  if(!Elf)return std::nullopt;
  const auto &File=Elf->getELFFile();auto Sections=File.sections();
  if(!Sections){consumeError(Sections.takeError());return std::nullopt;}
  if(Sections->size()>128)return std::nullopt;
  std::optional<StringRef> Blob;
  for(const auto &S:*Sections) {
    if(S.sh_type!=ELF::SHT_NOTE)continue;
    Error E=Error::success();
    for(const auto Note:File.notes(S,E)) {
      if(Note.getName()!="AMDGPU" || Note.getType()!=ELF::NT_AMDGPU_METADATA)continue;
      if(Blob || S.sh_addralign!=4){consumeError(std::move(E));return std::nullopt;}
      Blob=Note.getDescAsStringRef(4);
    }
    if(E){consumeError(std::move(E));return std::nullopt;}
  }
  if(!Blob || Blob->empty() || Blob->size()>16384)return std::nullopt;
  msgpack::Document Doc;
  if(!Doc.readFromBlob(*Blob,true) || !Doc.getRoot().isArray() ||
      Doc.getRoot().getArray().size()!=1)return std::nullopt;
  auto &N=Doc.getRoot().getArray()[0];AMDGPU::HSAMD::V3::MetadataVerifier Verifier(true);
  if(!Verifier.verify(N) || !N.isMap())return std::nullopt;
  auto &Root=N.getMap();
  if(compositionString(Root,"amdhsa.target")!=std::optional<StringRef>("amdgcn-amd-amdhsa--gfx942:xnack-"))return std::nullopt;
  auto K=Root.find("amdhsa.kernels");
  if(K==Root.end() || !K->second.isArray() || K->second.getArray().size()!=1 ||
      !K->second.getArray()[0].isMap())return std::nullopt;
  auto &Kernel=K->second.getArray()[0].getMap();
  if(compositionString(Kernel,".name")!=std::optional<StringRef>(R.Input.Entry) ||
      compositionString(Kernel,".symbol")!=std::optional<StringRef>(R.Input.Descriptor))return std::nullopt;
  for(const auto &[Key,Value]:std::array<std::pair<StringRef,uint64_t>,5>{{
      {".kernarg_segment_size",288},{".kernarg_segment_align",8},{".group_segment_fixed_size",0},
      {".wavefront_size",64},{".max_flat_workgroup_size",64}}})
    if(compositionUnsigned(Kernel,Key)!=std::optional<uint64_t>(Value))return std::nullopt;
  auto WG=Kernel.find(".reqd_workgroup_size");
  if(WG==Kernel.end() || !WG->second.isArray() || WG->second.getArray().size()!=3)return std::nullopt;
  for(size_t I=0;I<3;++I)if(WG->second.getArray()[I].getKind()!=msgpack::Type::UInt ||
      WG->second.getArray()[I].getUInt()!=(I==0?64:1))return std::nullopt;
  auto Dynamic=Kernel.find(".uses_dynamic_stack");
  if(Dynamic==Kernel.end() || Dynamic->second.getKind()!=msgpack::Type::Boolean)
    return std::nullopt;
  CompositionResources Result;
  Result.UsesDynamicStack=Dynamic->second.getBool();
  // Exact original MessagePack byte retained only for independent mutation controls.
  // The decoded metadata verifier above remains the acceptance parser.
  constexpr StringLiteral DynamicKey=".uses_dynamic_stack";
  const size_t KeyAt=Blob->find(DynamicKey);
  if(KeyAt==StringRef::npos || KeyAt==0 ||
      uint8_t((*Blob)[KeyAt-1])!=uint8_t(0xa0+DynamicKey.size()) ||
      Blob->find(DynamicKey,KeyAt+1)!=StringRef::npos ||
      KeyAt+DynamicKey.size()>=Blob->size() ||
      uint8_t((*Blob)[KeyAt+DynamicKey.size()])!=(Result.UsesDynamicStack?0xc3:0xc2))
    return std::nullopt;
  const auto Base=reinterpret_cast<uintptr_t>(Bytes.data());
  const auto DynamicByte=reinterpret_cast<uintptr_t>(Blob->data()+KeyAt+DynamicKey.size());
  if(DynamicByte<Base || DynamicByte-Base>=Bytes.size())return std::nullopt;
  Result.DynamicMetadataOffset=DynamicByte-Base;
  for(const auto &[Key,Ptr]:std::array<std::pair<StringRef,uint64_t*>,6>{{
      {".private_segment_fixed_size",&Result.Private},{".sgpr_count",&Result.Sgprs},
      {".vgpr_count",&Result.Vgprs},{".agpr_count",&Result.Agprs},
      {".sgpr_spill_count",&Result.SgprSpills},{".vgpr_spill_count",&Result.VgprSpills}}}) {
    const auto V=compositionUnsigned(Kernel,Key);if(!V || *V>1024*1024)return std::nullopt;*Ptr=*V;
  }
  if(Result.Sgprs==0 || Result.Sgprs>112 || Result.Vgprs<37 || Result.Vgprs>256 ||
      Result.Agprs>256 || Result.SgprSpills>1024 || Result.VgprSpills>1024)return std::nullopt;
  auto Args=Kernel.find(".args");
  if(Args==Kernel.end() || !Args->second.isArray() || Args->second.getArray().size()<18 ||
      Args->second.getArray().size()>24)return std::nullopt;
  constexpr std::array<uint64_t,5> Offsets{0,8,16,20,24},Sizes{8,8,4,4,4};
  struct Hidden {uint64_t Offset,Size;StringRef Kind;bool Required;};
  // Exact V5/V6 streamer layout, base alignTo(explicit28,8)=32. Optional
  // services may be suppressed by backend-inferred attributes. They are NOT
  // silently asserted absent, and any present row must match its fixed slot.
  constexpr std::array<Hidden,19> HiddenRows{{
    {32,4,"hidden_block_count_x",true},{36,4,"hidden_block_count_y",true},{40,4,"hidden_block_count_z",true},
    {44,2,"hidden_group_size_x",true},{46,2,"hidden_group_size_y",true},{48,2,"hidden_group_size_z",true},
    {50,2,"hidden_remainder_x",true},{52,2,"hidden_remainder_y",true},{54,2,"hidden_remainder_z",true},
    {72,8,"hidden_global_offset_x",true},{80,8,"hidden_global_offset_y",true},{88,8,"hidden_global_offset_z",true},
    {96,2,"hidden_grid_dims",true},{112,8,"hidden_hostcall_buffer",false},{120,8,"hidden_multigrid_sync_arg",false},
    {128,8,"hidden_heap_v1",false},{136,8,"hidden_default_queue",false},{144,8,"hidden_completion_action",false},
    {CompositionQueuePointerOffset,8,"hidden_queue_ptr",false}}};
  std::array<bool,19> Seen{};size_t Ordinal=0;uint64_t End=0;
  for(auto &AN:Args->second.getArray()) {
    if(!AN.isMap())return std::nullopt;
    auto &A=AN.getMap();const auto Offset=compositionUnsigned(A,".offset"),Size=compositionUnsigned(A,".size");
    const auto Kind=compositionString(A,".value_kind");
    if(!Offset || !Size || !Kind || *Size==0 || *Offset<End || *Offset>288 || *Size>288-*Offset)return std::nullopt;
    End=*Offset+*Size;std::string Name;
    if(Ordinal<5) {
      const auto ActualName=compositionString(A,".name");
      if(!ActualName || *ActualName!=R.ArgumentNames[Ordinal] || *Offset!=Offsets[Ordinal] ||
          *Size!=Sizes[Ordinal] || *Kind!=(Ordinal==0?"global_buffer":"by_value"))return std::nullopt;
      Name=ActualName->str();
      if(Ordinal==0) {if(compositionString(A,".address_space")!=std::optional<StringRef>("global"))return std::nullopt;}
      else if(A.find(".address_space")!=A.end())return std::nullopt;
    } else {
      size_t H=0;for(;H<HiddenRows.size() && HiddenRows[H].Kind!=*Kind;++H){}
      if(H==HiddenRows.size() || Seen[H] || *Offset!=HiddenRows[H].Offset || *Size!=HiddenRows[H].Size ||
          A.find(".name")!=A.end() || A.find(".address_space")!=A.end())return std::nullopt;
      if(H==18 && !compositionGfx942HiddenQueuePointer(*Offset,*Size))return std::nullopt;
      Seen[H]=true;
    }
    Result.Arguments.emplace_back(json::Object{{"ordinal",Ordinal},{"name",Name},
      {"kind",Kind->str()},{"offset",*Offset},{"bytes",*Size}});++Ordinal;
  }
  for(size_t I=0;I<HiddenRows.size();++I)if(HiddenRows[I].Required && !Seen[I])return std::nullopt;
  size_t Symbols=0,Found=0;
  std::optional<uint64_t> ArchVgprs,AccVgprs;
  std::array<std::optional<uint64_t>,2> StackSymbols;
  for(const auto &S:(*O)->symbols()) {
    if(++Symbols>4096)return std::nullopt;
    auto Name=S.getName();if(!Name){consumeError(Name.takeError());return std::nullopt;}
    if(*Name==R.Input.Entry+".num_vgpr" || *Name==R.Input.Entry+".num_agpr") {
      auto Flags=S.getFlags();if(!Flags){consumeError(Flags.takeError());return std::nullopt;}
      auto Value=S.getAddress();if(!Value){consumeError(Value.takeError());return std::nullopt;}
      auto &Slot=*Name==R.Input.Entry+".num_vgpr"?ArchVgprs:AccVgprs;
      if(!(*Flags & object::BasicSymbolRef::SF_Absolute) || Slot)return std::nullopt;
      Slot=*Value;
    }
    for(size_t I=0;I<StackSymbols.size();++I) {
      if(*Name!=R.Input.Entry+(I==0?".has_dyn_sized_stack":".has_recursion"))continue;
      auto Flags=S.getFlags();if(!Flags){consumeError(Flags.takeError());return std::nullopt;}
      auto Value=S.getAddress();if(!Value){consumeError(Value.takeError());return std::nullopt;}
      auto Raw=Elf->getSymbol(S.getRawDataRefImpl());
      if(!Raw){consumeError(Raw.takeError());return std::nullopt;}
      if(!(*Flags & object::BasicSymbolRef::SF_Absolute) || StackSymbols[I] || *Value>1)
        return std::nullopt;
      const auto Base=reinterpret_cast<uintptr_t>(Bytes.data()),At=reinterpret_cast<uintptr_t>(*Raw);
      if(At<Base || At-Base>Bytes.size() ||
          Bytes.size()-(At-Base)<sizeof(object::ELF64LE::Sym))return std::nullopt;
      StackSymbols[I]=*Value;Result.StackSymbolOffsets[I]=At-Base;
    }
    if(*Name!=R.Input.Descriptor)continue;
    if(++Found!=1)return std::nullopt;
    auto Section=S.getSection();if(!Section){consumeError(Section.takeError());return std::nullopt;}
    if(*Section==(*O)->section_end())return std::nullopt;
    auto Content=(*Section)->getContents();if(!Content){consumeError(Content.takeError());return std::nullopt;}
    auto Address=S.getAddress();if(!Address){consumeError(Address.takeError());return std::nullopt;}
    if(*Address<(*Section)->getAddress())return std::nullopt;
    const auto At=*Address-(*Section)->getAddress();
    if(At>Content->size() || Content->size()-At<64)return std::nullopt;
    const auto *B=reinterpret_cast<const uint8_t*>(Content->data())+At;
    const auto Base=reinterpret_cast<uintptr_t>(Bytes.data()),Start=reinterpret_cast<uintptr_t>(B);
    if(Start<Base || Start-Base>Bytes.size() || Bytes.size()-(Start-Base)<64)return std::nullopt;
    Result.DescriptorOffset=Start-Base;Result.DescriptorDigest=SHA256::hash(ArrayRef<uint8_t>(B,64));
    using namespace llvm::amdhsa;
    if(support::endian::read32le(B+GROUP_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
       support::endian::read32le(B+PRIVATE_SEGMENT_FIXED_SIZE_OFFSET)!=Result.Private ||
       support::endian::read32le(B+KERNARG_SIZE_OFFSET)!=288)return std::nullopt;
    Result.Rsrc1=support::endian::read32le(B+COMPUTE_PGM_RSRC1_OFFSET);
    Result.Rsrc2=support::endian::read32le(B+COMPUTE_PGM_RSRC2_OFFSET);
    Result.Rsrc3=support::endian::read32le(B+COMPUTE_PGM_RSRC3_OFFSET);
    Result.Properties=support::endian::read16le(B+KERNEL_CODE_PROPERTIES_OFFSET);
    Result.VgprCapacity=((Result.Rsrc1&63)+1)*8;
    Result.SgprCapacity=(((Result.Rsrc1>>6)&15)+1)*8;
    if((Result.Properties & KERNEL_CODE_PROPERTY_ENABLE_WAVEFRONT_SIZE32)!=0 ||
       bool(Result.Properties & KERNEL_CODE_PROPERTY_ENABLE_SGPR_QUEUE_PTR)!=Seen[18] ||
       Result.VgprCapacity<Result.Vgprs || Result.VgprCapacity>256 ||
       Result.SgprCapacity<Result.Sgprs || Result.SgprCapacity>112 ||
       support::endian::read16le(B+KERNARG_PRELOAD_OFFSET)!=0)return std::nullopt;
  }
  if(!StackSymbols[0] || !StackSymbols[1] ||
      !compositionGfx942DynamicStack(*StackSymbols[0],*StackSymbols[1],
        Result.UsesDynamicStack,Result.Properties))return std::nullopt;
  Result.HasDynSizedStack=*StackSymbols[0];Result.HasRecursion=*StackSymbols[1];
  if(Found!=1 || !ArchVgprs || !AccVgprs || *AccVgprs!=Result.Agprs ||
      !compositionGfx942Allocation(*ArchVgprs,*AccVgprs,Result.Vgprs,Result.Rsrc1,Result.Rsrc3))
    return std::nullopt;
  Result.ArchVgprs=*ArchVgprs;Result.AccumStart=((Result.Rsrc3&63)+1)*4;
  return Result;
}
json::Object compositionResourceJson(const CompositionResources &R) {
  return json::Object{{"private_segment_bytes",R.Private},{"scratch_free_claim",false},
    {"sgpr_count",R.Sgprs},{"vgpr_count",R.Vgprs},{"agpr_count",R.Agprs},
    {"arch_vgpr_count",R.ArchVgprs},{"accum_vgpr_start",R.AccumStart},
    {"vgpr_count_semantics","gfx942_combined_arch_aligned_plus_accumulator"},
    {"compiler_register_allocation_observed",true},{"extra_machine_effects_admitted",false},
    {"uses_dynamic_stack",R.UsesDynamicStack},
    {"backend_has_dyn_sized_stack_symbol",R.HasDynSizedStack},
    {"backend_has_recursion_symbol",R.HasRecursion},
    {"source_recursion_proved",false},{"dynamic_stack_execution_proved",false},
    {"sgpr_spills",R.SgprSpills},{"vgpr_spills",R.VgprSpills},
    {"sgpr_capacity",R.SgprCapacity},{"vgpr_capacity",R.VgprCapacity},
    {"compute_pgm_rsrc1",R.Rsrc1},{"compute_pgm_rsrc2",R.Rsrc2},{"compute_pgm_rsrc3",R.Rsrc3},
    {"kernel_code_properties",R.Properties},{"descriptor_sha256",hex(R.DescriptorDigest)},
    {"descriptor_file_offset",R.DescriptorOffset},{"arguments",json::Array(R.Arguments)}};
}
