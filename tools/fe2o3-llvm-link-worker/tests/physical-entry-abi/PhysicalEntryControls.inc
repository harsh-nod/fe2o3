// All native controls change actual bytes and obtain a fresh decoder observation.
Expected<PhysicalMachineEffectEvidence> physicalAnalyze(ArrayRef<uint8_t> Bytes) {
  require(!Bytes.empty() && Bytes.size()<=64*1024,"physical changed payload bound");
  const auto Identities=physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest R;
  R.AnalyzerIdentity=Identities.Analyzer; R.ToolchainIdentity=Identities.Toolchain;
  R.ExecutionChallenge.fill(0x70); R.RequestIdentity.fill(0x71); R.RequestBytes=4096;
  R.Payload.assign(Bytes.begin(),Bytes.end()); R.PayloadDigest=SHA256::hash(Bytes);
  R.PayloadBytes=Bytes.size(); R.Entries={{PhysicalName.str(),{16,8,4,2,0}}};
  return analyzeGfx942PhysicalMachineEffects(R);
}
json::Array physicalInputNegatives(bool Select,OptimizationLevel Level) {
  json::Array Rows;
  for(bool Missing:{true,false}) {
    auto Result=execute(makeInputRequest(physicalInput(Select,Missing,!Missing),Level,PhysicalName));
    require(!Result.LinkedOutput && !Result.Derivation && Result.FailureStage==Stage::InputValidation,
        "physical wrong target/missing launch not refused at input");
    checkDiagnosticBounds(Result.Diagnostics);
    const StringRef Needle=Missing?"has no exact required-workgroup metadata":"target CPU does not match request";
    require(llvm::any_of(Result.Diagnostics,[Needle](const std::string &D){return StringRef(D).contains(Needle);}),
        "physical input refusal cause mismatch");
    Rows.emplace_back(json::Object{{"mutation",Missing?"missing_launch":"wrong_cpu"},
        {"phase","ordinary_worker_input_validation"},{"accepted",false}});
  }
  return Rows;
}
json::Array physicalMutations(bool Select,const BuiltFixture &Built,const PhysicalDescriptor &D) {
  const auto &Original=Built.ResponseValue.LinkedOutput->Bytes;
  const auto &Trace=Built.Evidence.Instructions;
  const size_t Tail=Select?16:12;
  struct Change { StringRef Name; uint64_t Offset; uint32_t Value; unsigned Width; bool Descriptor; };
  std::vector<Change> Changes;
  auto Replace=[&](StringRef Name,size_t I,uint32_t Word) {
    require(I<Trace.size() && Trace[I].Encoding.size()>=4,"physical native mutation row");
    Changes.push_back({Name,Trace[I].InstructionOffset,Word,4,false});
  };
  auto Flip=[&](StringRef Name,size_t I,unsigned Delta,uint32_t Mask) {
    require(I<Trace.size() && Delta+4<=Trace[I].Encoding.size(),"physical native mutation subword");
    Changes.push_back({Name,Trace[I].InstructionOffset+Delta,
        support::endian::read32le(Trace[I].Encoding.data()+Delta)^Mask,4,false});
  };
  Flip("pointer_slot_offset",0,4,4);
  Flip("length_slot_offset",1,4,4);
  Flip("selector_slot_offset",5,4,4);
  Replace("load_wait_removed",6,0xbf800000U);
  Flip("index_wrong_workgroup_register",7,0,1);
  Flip("index_wrong_shift",7,0,1U<<8);
  Flip("index_wrong_local_register",8,0,1U<<9);
  Flip("pointer_high_wrong_register",10,0,1);
  Flip("pointer_low_carry_source",Tail+1,0,1);
  Flip("index_64bit_scale_wrong",Tail,4,1);
  Replace("bounds_compare_removed",Tail+3,0xbf800000U);
  Flip("bounds_compare_wrong_length_pair",Tail+3,0,2);
  Replace("exec_mask_removed",Tail+4,0xbf800000U);
  Flip("store_wrong_data_register",Tail+5,4,1U<<8);
  Replace("store_wait_removed",Tail+6,0xbf800000U);
  Replace("store_wait_one_pending",Tail+6,0xbf8c0f71U);
  Replace("mask_restore_removed",Tail+7,0xbf800000U);
  Replace("termination_removed",Tail+8,0xbf800000U);
  if(Select) {
    const uint32_t Word=support::endian::read32le(Trace[12].Encoding.data());
    Replace("selector_branch_self",12,(Word&0xffff0000U)|0xffffU);
    const auto Difference=Trace[16].InstructionOffset-(Trace[12].InstructionOffset+4);
    require(Difference%4==0 && Difference/4<32768,"physical branch mutation bound");
    Replace("selector_branch_skips_result",12,(Word&0xffff0000U)|static_cast<uint32_t>(Difference/4));
  }
  const std::array<Change,12> DescriptorChanges{{
    {"missing_kernarg_enable",D.Offset+56,0,2,true},
    {"extra_dispatch_pointer",D.Offset+56,10,2,true},
    {"zero_user_sgpr_count",D.Offset+52,128,4,true},
    {"extra_user_sgpr_count",D.Offset+52,134,4,true},
    {"missing_workgroup_x",D.Offset+52,4,4,true},
    {"workitem_y_enabled",D.Offset+52,132|(1U<<11),4,true},
    {"kernarg_preload_enabled",D.Offset+58,1,2,true},
    {"scratch_enabled",D.Offset+52,133,4,true},
    {"vgpr_capacity_too_small",D.Offset+48,D.Rsrc1&~0x3fU,4,true},
    {"sgpr_capacity_too_small",D.Offset+48,D.Rsrc1&~0x3c0U,4,true},
    {"kernarg_extent_changed",D.Offset+8,280,4,true},
    {"reserved_descriptor_field",D.Offset+12,1,4,true}
  }};
  Changes.insert(Changes.end(),DescriptorChanges.begin(),DescriptorChanges.end());
  require(Changes.size()<=32,"physical mutation count bound");
  json::Array Rows;
  for(const auto &C:Changes) {
    require(C.Offset<=Original.size() && C.Width<=Original.size()-C.Offset &&
        (C.Width==2 || C.Width==4),"physical mutation payload extent");
    auto Changed=Original;
    if(C.Width==2) support::endian::write16le(Changed.data()+C.Offset,static_cast<uint16_t>(C.Value));
    else support::endian::write32le(Changed.data()+C.Offset,C.Value);
    require(Changed!=Original,"physical mutation must change bytes");
    auto E=physicalAnalyze(Changed);
    std::string Phase,Reason;
    if(!E) { Phase="existing_machine_decoder"; Reason=toString(E.takeError()); }
    else if(C.Descriptor) {
      auto M=physicalMetadata(Changed,PhysicalName);
      require(M.has_value(),"descriptor-only mutation changed ELF metadata");
      require(!physicalDescriptor(*E,Changed,*M),"physical entry descriptor mutation accepted");
      Phase="physical_entry_descriptor_relation"; Reason="exact entry inputs, extent, resources or reserved bits differ";
    } else {
      auto Failure=physicalRelation(Select,*E,Changed);
      require(Failure.has_value(),"physical authored-entry mutation accepted");
      Phase="physical_complete_entry_relation"; Reason=*Failure;
    }
    require(Reason.size()<=4096,"physical mutation diagnostic bound");
    Rows.emplace_back(json::Object{{"mutation",C.Name},{"phase",Phase},{"reason",Reason},
        {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"fresh_payload_hash",true},{"accepted",false}});
  }
  // Independent metadata refusals, not altered observations or stale identities.
  for(const auto &[From,To]:std::array<std::pair<StringRef,StringRef>,2>{{
          {"selector","selectox"},{"hidden_grid_dims","hidden_grid_dimx"}}}) {
    auto Changed=Original;
    auto At=std::search(Changed.begin(),Changed.end(),From.begin(),From.end());
    require(At!=Changed.end() && std::search(At+From.size(),Changed.end(),From.begin(),From.end())==Changed.end(),
        "physical metadata anchor absent or ambiguous");
    require(From.size()==To.size(),"physical metadata fixed extent");
    std::copy(To.begin(),To.end(),At);
    require(!physicalMetadata(Changed,PhysicalName),"physical metadata mutation accepted");
    Rows.emplace_back(json::Object{{"mutation",From},{"phase","physical_argument_metadata_relation"},
        {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"accepted",false}});
  }
  return Rows;
}
