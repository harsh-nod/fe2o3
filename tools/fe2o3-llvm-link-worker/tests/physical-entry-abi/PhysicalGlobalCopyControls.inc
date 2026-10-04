// Fresh byte mutations only. No evidence editing or stale-payload identity reuse.
Expected<PhysicalMachineEffectEvidence> globalCopyPhysicalAnalyze(ArrayRef<uint8_t> Bytes,StringRef Symbol) {
  require(!Bytes.empty()&&Bytes.size()<=64*1024,"canonical changed payload bound");
  const auto Identities=physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest R;R.AnalyzerIdentity=Identities.Analyzer;R.ToolchainIdentity=Identities.Toolchain;
  R.ExecutionChallenge.fill(0x70);R.RequestIdentity.fill(0x71);R.RequestBytes=4096;
  R.Payload.assign(Bytes.begin(),Bytes.end());R.PayloadDigest=SHA256::hash(Bytes);R.PayloadBytes=Bytes.size();
  R.Entries={{Symbol.str(),{16,8,4,2,0}}};return analyzeGfx942PhysicalMachineEffects(R);
}
bool globalCopyInputJoin(const GlobalCopyInput &C,const Input &Llvm) {
  return C.LlvmBytes==Llvm.Bytes.size()&&C.LlvmDigest==hex(Llvm.Digest);
}
json::Array globalCopyPhysicalSidecarNegatives(ArrayRef<uint8_t> Original,const Input &Llvm) {
  const std::string Base(reinterpret_cast<const char*>(Original.data()),Original.size());
  std::vector<std::pair<std::string,std::string>> Changes;
  Changes.emplace_back("missing_final_lf",Base.substr(0,Base.size()-1));
  Changes.emplace_back("extra_trailing_record",Base+"junk\n");
  auto Header=Base;Header[0]='X';Changes.emplace_back("unknown_header",std::move(Header));
  auto Old=Base;const auto FirstEnd=Old.find('\n');
  require(FirstEnd!=std::string::npos,"global-copy header extent");
  Old.replace(0,FirstEnd,"FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1");
  Changes.emplace_back("old_v20_header",std::move(Old));
  auto Nul=Base;Nul[3]='\0';Changes.emplace_back("non_ascii_control",std::move(Nul));
  auto Canonical=Base;auto At=Canonical.find("canonical ");require(At!=std::string::npos,"canonical anchor");
  Canonical[At+10]='G';Changes.emplace_back("noncanonical_digest",std::move(Canonical));
  auto Identity=Base;At=Identity.find("llvm ");require(At!=std::string::npos,"LLVM identity anchor");
  Identity[At+5]=Identity[At+5]=='0'?'1':'0';Changes.emplace_back("wrong_llvm_digest",std::move(Identity));
  auto Duplicate=Base;At=Duplicate.find("blocks ");require(At!=std::string::npos,"block anchor");
  Duplicate.insert(At,"blocks 1\n");Changes.emplace_back("duplicate_record",std::move(Duplicate));
  auto Opcode=Base;At=Opcode.find("step ");require(At!=std::string::npos,"step anchor");
  for(unsigned I=0;I<3;++I)At=Opcode.find(' ',At)+1;
  auto End=Opcode.find(' ',At);Opcode.replace(At,End-At,"255");
  Changes.emplace_back("unknown_step_descriptor",std::move(Opcode));
  auto Destination=Base;At=Destination.find("step ");
  for(unsigned I=0;I<4;++I)At=Destination.find(' ',At)+1;
  End=Destination.find(' ',At);Destination.replace(At,End-At,"64");
  Changes.emplace_back("physical_destination_out_of_range",std::move(Destination));
  auto Control=Base;At=Control.find("block 0 ");
  require(At!=std::string::npos,"canonical block control anchor");
  Control[At+8]='3';Changes.emplace_back("entry_terminator_encoding_drift",std::move(Control));
  auto Length=Base;At=Length.find("canonical ")+10+64+1;
  Length.insert(At,"0");Changes.emplace_back("noncanonical_decimal",std::move(Length));
  json::Array Rows;
  for(const auto &[Name,Changed]:Changes) {
    auto Parsed=parseGlobalCopyInput(ArrayRef<uint8_t>(
        reinterpret_cast<const uint8_t*>(Changed.data()),Changed.size()));
    const bool Accepted=Parsed&&globalCopyInputJoin(*Parsed,Llvm)&&globalCopyPhysicalRows(*Parsed).has_value();
    require(!Accepted,"canonical sidecar mutation accepted");
    Rows.emplace_back(json::Object{{"mutation",Name},{"accepted",false},
        {"phase","bounded_inert_expectation_parser_or_content_join"}});
  }
  return Rows;
}
json::Array globalCopyPhysicalInputNegatives(const Input &Original,StringRef Symbol,OptimizationLevel Level) {
  json::Array Rows;
  for(bool Missing:{true,false}) {
    std::string Text(reinterpret_cast<const char*>(Original.Bytes.data()),Original.Bytes.size());
    const StringRef From=Missing?"!reqd_work_group_size !0":"\"target-cpu\"=\"gfx942\"";
    const StringRef To=Missing?"":"\"target-cpu\"=\"gfx950\"";
    const auto At=Text.find(From.str());
    require(At!=std::string::npos&&Text.find(From.str(),At+From.size())==std::string::npos,
        "canonical input negative exact emitter anchor");
    Text.replace(At,From.size(),To.str());Input Changed;Changed.Kind=InputKind::LlvmTextIr;
    Changed.Bytes.assign(Text.begin(),Text.end());Changed.Digest=SHA256::hash(Changed.Bytes);
    const auto Result=execute(makeInputRequest(std::move(Changed),Level,Symbol));
    require(!Result.LinkedOutput&&!Result.Derivation&&Result.FailureStage==Stage::InputValidation,
        "canonical target/launch mutation passed worker input");
    checkDiagnosticBounds(Result.Diagnostics);
    const StringRef Needle=Missing?"has no exact required-workgroup metadata":"target CPU does not match request";
    require(llvm::any_of(Result.Diagnostics,[Needle](const std::string &D){return StringRef(D).contains(Needle);}),
        "canonical input refusal cause");
    Rows.emplace_back(json::Object{{"mutation",Missing?"missing_launch":"wrong_cpu"},
        {"phase","ordinary_worker_input_validation"},{"accepted",false}});
  }
  return Rows;
}
json::Array globalCopyPhysicalMutations(const GlobalCopyInput &Input,
    const GlobalCopyRelation &Expected,const BuiltFixture &Built,const PhysicalDescriptor &D) {
  const auto &Original=Built.ResponseValue.LinkedOutput->Bytes;const auto &Trace=Built.Evidence.Instructions;
  struct Change {std::string Name;uint64_t Offset;uint32_t Value;unsigned Width;bool Descriptor;};
  std::vector<Change> Changes;
  for(size_t I=0;I<Trace.size();++I)
    Changes.push_back({"instruction_"+std::to_string(I)+"_removed",Trace[I].InstructionOffset,0xbf800000U,4,false});
  auto Flip=[&](std::string Name,const auto &I,unsigned Delta,uint32_t Mask) {
    require(Delta+4<=I.Encoding.size(),"global-copy native mutation subword");
    Changes.push_back({std::move(Name),I.InstructionOffset+Delta,
        support::endian::read32le(I.Encoding.data()+Delta)^Mask,4,false});
  };
  for(const auto &Step:Input.Steps) {
    const auto &I=Trace[Step.Native];const auto O=Step.Descriptor[0];
    const auto Suffix="_"+std::to_string(Step.Native);
    if(O==0) {
      Flip("kernarg_offset"+Suffix,I,4,8);
      Flip("kernarg_origin_base"+Suffix,I,0,1);
    }
    if(O==2) {
      Flip("index_workgroup_origin"+Suffix,I,0,1);
      Flip("index_shift"+Suffix,I,0,1U<<8);
    }
    if(O==3)Flip("index_lane_origin"+Suffix,I,0,1U<<9);
    if(O==4&&Step.Descriptor[2]!=255)Flip("pointer_high_origin"+Suffix,I,0,1);
    if(O==5)Flip("index_64bit_scale"+Suffix,I,4,1);
    if(O==6)Flip("pointer_low_carry_source"+Suffix,I,0,1);
    if(O==7)Flip("pointer_high_carry_source"+Suffix,I,0,1);
    if(O==8) {
      Flip("load_destination"+Suffix,I,4,1U<<24);
      Flip("load_address_pair"+Suffix,I,4,2);
      Flip("load_sgpr_off_origin"+Suffix,I,4,1U<<16);
      Flip("load_offset"+Suffix,I,0,4);
      Flip("load_cache_policy"+Suffix,I,0,1U<<16);
    }
    if(O==9)Changes.push_back({"vm_wait_one_pending"+Suffix,I.InstructionOffset,0xbf8c0f71U,4,false});
    if(O==10)Flip("bounds_length_origin"+Suffix,I,0,2);
    if(O==11)Changes.push_back({"exec_mask_removed"+Suffix,I.InstructionOffset,0xbf800000U,4,false});
    if(O==12) {
      Flip("store_actual_load_register"+Suffix,I,4,1U<<8);
      Flip("store_output_address"+Suffix,I,4,2);
    }
  }
  const std::array<Change,10> Fixed{{
    {"missing_kernarg_enable",D.Offset+56,0,2,true},
    {"extra_dispatch_pointer",D.Offset+56,10,2,true},
    {"zero_user_sgpr_count",D.Offset+52,128,4,true},
    {"extra_user_sgpr_count",D.Offset+52,134,4,true},
    {"missing_workgroup_x",D.Offset+52,4,4,true},
    {"workitem_y_enabled",D.Offset+52,132|(1U<<11),4,true},
    {"kernarg_preload_enabled",D.Offset+58,1,2,true},
    {"scratch_enabled",D.Offset+52,133,4,true},
    {"kernarg_extent_changed",D.Offset+8,280,4,true},
    {"reserved_descriptor_field",D.Offset+12,1,4,true}
  }};
  Changes.insert(Changes.end(),Fixed.begin(),Fixed.end());
  if(Expected.Vgprs>8)Changes.push_back({"vgpr_capacity_too_small",D.Offset+48,D.Rsrc1&~0x3fU,4,true});
  if(Expected.Sgprs>8)Changes.push_back({"sgpr_capacity_too_small",D.Offset+48,D.Rsrc1&~0x3c0U,4,true});
  require(Changes.size()<=96,"canonical native mutation bound");
  json::Array Rows;
  for(const auto &C:Changes) {
    require(C.Offset<=Original.size()&&C.Width<=Original.size()-C.Offset&&(C.Width==2||C.Width==4),
        "canonical mutation payload extent");
    auto Changed=Original;
    if(C.Width==2)support::endian::write16le(Changed.data()+C.Offset,static_cast<uint16_t>(C.Value));
    else support::endian::write32le(Changed.data()+C.Offset,C.Value);
    require(Changed!=Original,"canonical mutation must change payload");
    auto E=globalCopyPhysicalAnalyze(Changed,Input.Entry);std::string Phase,Reason;
    if(!E){Phase="existing_machine_decoder";Reason=toString(E.takeError());}
    else if(C.Descriptor) {
      auto M=globalCopyMetadata(Changed,Input.Entry);require(M.has_value(),"descriptor mutation changed metadata");
      require(!physicalDescriptor(*E,Changed,*M,Input.Entry,Expected.Sgprs,Expected.Vgprs),
          "canonical descriptor mutation accepted");
      Phase="physical_entry_descriptor_relation";Reason="exact entry inputs/resources/reserved fields differ";
    } else {
      auto Failure=globalCopyPhysicalRelation(Input,Expected,*E,Changed);
      require(Failure.has_value(),"canonical authored entry mutation accepted");
      Phase="physical_complete_entry_relation";Reason=*Failure;
    }
    require(Reason.size()<=4096,"canonical mutation diagnostic bound");
    Rows.emplace_back(json::Object{{"mutation",C.Name},{"phase",Phase},{"reason",Reason},
        {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"fresh_payload_hash",true},{"accepted",false}});
  }
  for(const auto &[From,To]:std::array<std::pair<StringRef,StringRef>,5>{{
      {"input_data","input_datx"},{"input_length","input_lengtx"},{"output_data","output_datx"},
      {"output_length","output_lengtx"},{"hidden_grid_dims","hidden_grid_dimx"}}}) {
    auto Changed=Original;auto At=std::search(Changed.begin(),Changed.end(),From.begin(),From.end());
    require(At!=Changed.end()&&std::search(At+From.size(),Changed.end(),From.begin(),From.end())==Changed.end(),
        "canonical metadata anchor absent/ambiguous");
    require(From.size()==To.size(),"global-copy metadata fixed extent");
    std::copy(To.begin(),To.end(),At);require(!globalCopyMetadata(Changed,Input.Entry),"canonical metadata mutation accepted");
    Rows.emplace_back(json::Object{{"mutation",From},{"phase","physical_argument_metadata_relation"},
        {"changed_payload_sha256",hex(SHA256::hash(Changed))},{"accepted",false}});
  }
  return Rows;
}
