// Mutate actual bytes, then decode/check the changed bytes with a fresh hash.
std::vector<uint8_t> changedMetadata(ArrayRef<uint8_t> Original,const Observation &A,
    StringRef Key,uint64_t Value,std::optional<size_t> Argument=std::nullopt) {
  require(within(Original.size(),A.NoteOffset,A.NoteBytes),"metadata mutation extent");
  msgpack::Document D;
  require(D.readFromBlob(StringRef(reinterpret_cast<const char*>(Original.data()+A.NoteOffset),A.NoteBytes),true) &&
      D.getRoot().isArray() && D.getRoot().getArray().size()==1,"metadata mutation parse");
  auto Root=D.getRoot().getArray()[0];
  auto &Kernel=Root.getMap()["amdhsa.kernels"].getArray()[0].getMap();
  if (Argument) {
    auto &Args=Kernel[".args"].getArray();
    require(*Argument<Args.size(),"metadata argument mutation bound");
    Args[*Argument].getMap()[Key]=D.getNode(Value);
  } else Kernel[Key]=D.getNode(Value);
  // readFromBlob(Multi=true) wraps the one actual map; do not serialize wrapper.
  D.getRoot()=Root; std::string Blob; D.writeToBlob(Blob);
  require(Blob.size()==A.NoteBytes,"metadata mutation must preserve note byte length");
  std::vector<uint8_t> Changed(Original.begin(),Original.end());
  std::copy(Blob.begin(),Blob.end(),Changed.begin()+A.NoteOffset); return Changed;
}
json::Array controls(const Output &Original,const Observation &A,const Request &R,Mc &M) {
  json::Array Results; std::set<std::string> Names;
  auto Observe=[&](std::string Label,std::vector<uint8_t> Changed) {
    require(Results.size()<128 && Names.insert(Label).second && Changed!=Original.Bytes,
        "nonempty unique bounded mutation");
    auto Result=inspect(Changed,R,M);
    require(!Result,"changed fixture unexpectedly accepted");
    std::string Reason=toString(Result.takeError());
    require(!Reason.empty() && Reason.size()<=4096,"mutation reason bound");
    Results.emplace_back(json::Object{{"mutation",std::move(Label)},{"accepted",false},
        {"reason",std::move(Reason)},{"bytes",Changed.size()},
        {"sha256",hex(SHA256::hash(Changed))}});
  };
  auto Word=[&](std::string Label,uint64_t At,uint32_t Value) {
    require(within(Original.Bytes.size(),At,4),"word mutation range");
    auto B=Original.Bytes; support::endian::write32le(B.data()+At,Value); Observe(std::move(Label),std::move(B));
  };
  auto Flip=[&](std::string Label,uint64_t At,uint32_t Mask) {
    require(within(Original.Bytes.size(),At,4),"flip mutation range");
    Word(std::move(Label),At,support::endian::read32le(Original.Bytes.data()+At)^Mask);
  };
  for (size_t I=0;I<A.Trace.size();++I) {
    const auto At=A.EntryOffset+A.Trace[I].Offset;
    Word("row_"+std::to_string(I)+"_removed",At,0xbf800000);
    Flip("row_"+std::to_string(I)+"_operand",At,1);
  }
  Word("load_wait_lgkm_pending",A.EntryOffset+8,0xbf8c0100);
  Word("store_wait_vm_pending",A.EntryOffset+72,0xbf8c0001);
  Word("trap_abort_id2",A.EntryOffset+76,0xbf920002);
  Word("trap_breakpoint_id1",A.EntryOffset+76,0xbf920001);
  Word("extra_early_trap",A.EntryOffset+72,0xbf920003);
  Word("second_trap_at_end",A.EntryOffset+80,0xbf920003);
  Flip("pointer_kernarg_offset",A.EntryOffset+4,8);
  Flip("sentinel_s12_literal",A.EntryOffset+16,1);
  Flip("sentinel_s13_literal",A.EntryOffset+24,1);
  Flip("scale_64bit_immediate",A.EntryOffset+40,1);
  Flip("store_data_register",A.EntryOffset+68,1U<<8);
  Flip("store_offset_nonzero",A.EntryOffset+64,4);
  Word("prefetch_padding_changed",A.EntryOffset+84,0xbf810000);
  const std::array<std::tuple<const char*,unsigned,uint32_t>,20> DescriptorChanges{{
    {"group_segment",0,512},{"private_segment",4,4},{"kernarg_total",8,256},
    {"reserved12",12,1},{"entry_offset",16,0},{"reserved24",24,1},
    {"reserved40",40,1},{"rsrc3_extra",44,A.Rsrc3|(1U<<16)},
    {"vgpr_short",48,A.Rsrc1&~63U},{"sgpr_short",48,A.Rsrc1&~(15U<<6)},
    {"privilege",48,A.Rsrc1|(1U<<20)},{"debug_mode",48,A.Rsrc1|(1U<<22)},
    {"workgroup_x_missing",52,4},{"user_sgpr_extra",52,134},
    {"workitem_y",52,132|(1U<<11)},{"scratch",52,133},
    {"trap_enable_descriptor",52,196},{"kernarg_pointer_missing",56,0},
    {"kernarg_preload",56,static_cast<uint32_t>(llvm::amdhsa::KERNEL_CODE_PROPERTY_ENABLE_SGPR_KERNARG_SEGMENT_PTR)|(1U<<16)},
    {"reserved60",60,1}}};
  for (const auto &[Label,Offset,Value]:DescriptorChanges)
    Word(std::string("descriptor_")+Label,A.DescriptorOffset+Offset,Value);
  for (const auto &[Label,Key,Value]:std::array<std::tuple<const char*,const char*,uint64_t>,10>{{
      {"wave",".wavefront_size",32},{"sgpr",".sgpr_count",0},{"vgpr",".vgpr_count",0},
      {"agpr",".agpr_count",1},{"sgpr_spill",".sgpr_spill_count",1},{"vgpr_spill",".vgpr_spill_count",1},
      {"kernarg_total",".kernarg_segment_size",265},{"kernarg_align",".kernarg_segment_align",4},
      {"workgroup_cap",".max_flat_workgroup_size",32},{"grid_cap",".max_num_workgroups_x",2}}})
    Observe(std::string("metadata_")+Label,changedMetadata(Original.Bytes,A,Key,Value));
  Observe("metadata_output_offset",changedMetadata(Original.Bytes,A,".offset",1,0));
  Observe("metadata_hidden_offset",changedMetadata(Original.Bytes,A,".offset",9,1));
  Observe("metadata_hidden_width",changedMetadata(Original.Bytes,A,".size",2,1));
  Flip("elf_target",48,1); Flip("elf_machine",16,1U<<16);
  auto Abi=Original.Bytes; Abi[8]^=1; Observe("elf_cov",std::move(Abi));
  auto Truncated=Original.Bytes; Truncated.resize(63); Observe("elf_truncated",std::move(Truncated));
  auto Over=Original.Bytes; Over.resize(ArtifactCap+1); Observe("artifact_one_over",std::move(Over));
  require(Results.size()==83,"exact 83 native/metadata mutation roster");
  return Results;
}
json::Array inputControls(OptimizationLevel Level) {
  json::Array Rows;
  for (bool Missing:{true,false}) {
    auto R=requestFor(fixedInput(Missing,!Missing),Level); auto Reply=execute(R);
    require(!Reply.LinkedOutput && !Reply.Derivation && Reply.FailureStage==Stage::InputValidation,
        "exact ordinary input refusal");
    const StringRef Needle=Missing?"has no exact required-workgroup metadata":"target CPU does not match request";
    require(llvm::any_of(Reply.Diagnostics,[&](const std::string &D){return StringRef(D).contains(Needle);}),
        "ordinary refusal cause");
    Rows.emplace_back(json::Object{{"mutation",Missing?"missing_launch":"wrong_cpu"},{"accepted",false}});
  }
  return Rows;
}
