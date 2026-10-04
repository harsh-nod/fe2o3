// This reader is a closed diagnostic profile, not arbitrary LLVM/source admission.
PromotedInput promotedRecord(StringRef Report,StringRef ReportHash,StringRef Repository,
                             StringRef Variant) {
  require(Variant=="copy" || Variant=="preserve" || Variant=="edit","closed promoted variant");
  PromotedInput P;auto &R=P.Record;auto &In=R.Input;
  R.Profile=CompositionProfile::PromotedV8;In.Label=Variant.str();
  const auto Bytes=helperPinned(In,Report,ReportHash,4*1024*1024);
  auto Parsed=json::parse(StringRef(reinterpret_cast<const char*>(Bytes.data()),Bytes.size()));
  require(bool(Parsed),"promoted report JSON");const auto &Root=compositionRow(*Parsed);
  require(helperString(Root,"schema")=="fe2o3-test-composition-promoted-normal-ladder-v1",
    "only actual promoted source report accepted");
  for(const auto &[Key,N]:std::array<std::pair<StringRef,uint64_t>,9>{{
      {"workload_children",23},{"actual_extractor_invocations",13},{"source_publications",3},
      {"feature_specific_capture_diagnostics",3},{"exact_source_refusals",6},
      {"actual_normal_sessions",10},{"fresh_candidate_normal_triples",3},
      {"cpu_cases",128},{"descriptor_extension_refusals",15}}})
    require(helperInteger(Root,Key)==N,"completed promoted source census");
  helperBool(Root,"fresh_source_custody_constructed_in_callback",true);
  promotedFalse(Root,{"source_custody_from_files","published_candidates_modified",
    "historical_evidence_rewritten","runtime_conditions_discharged","native_worker_qualified",
    "hardware_observed","protected_authority"});
  const auto &Sessions=compositionArray(Root,"normal_sessions",10);
  const auto &Owner=promotedFind(Sessions,"variant",Variant,"observe");
  const auto &Llvm=promotedFind(Sessions,"variant",Variant,"llvm");
  const auto &Handoff=promotedFind(Sessions,"variant",Variant,"handoff");
  const auto &O=helperObject(Owner,"observation");
  const auto &Invocation=helperObject(Owner,"invocation");
  const std::string Package="package-promoted-"+Variant.str();
  require(helperString(Invocation,"schema")=="fe2o3-test-ordered-composition-publisher-invocation-v1" &&
    helperString(Invocation,"case")==Variant && helperString(Invocation,"package")==Package &&
    helperString(Invocation,"feature")=="ordered-composition-publish-direct",
    "fresh promoted package/feature identity");
  for(const auto *S:{&Owner,&Llvm,&Handoff}) {
    require(helperString(*S,"schema")=="fe2o3-test-composition-promoted-normal-session-v1",
      "actual promoted session schema");
    helperBool(*S,"actual_fresh_frontend",true);
    promotedFalse(*S,{"runtime_conditions_discharged","source_custody_exported",
      "hardware_observed","protected_authority"});
    promotedEqual(*S,Owner,"invocation");promotedEqual(*S,Owner,"source");
  }
  const auto &Snapshot=helperObject(Owner,"source");
  require(helperString(Invocation,"leaf_sha256")==helperString(Snapshot,"sha256"),
    "fresh frontend source digest");
  const auto &Action=promotedFind(compositionArray(Root,"actions",13),"case",Variant);
  const auto &Publication=helperObject(Action,"result");
  require(helperString(Publication,"stage")=="actual_cli_published","actual CLI publication");
  helperBool(Action,"actual_extractor",true);require(helperInteger(Action,"exit")==0,"publication exit");
  require(Publication.get("candidate") && *Publication.get("candidate")==*Owner.get("source"),
    "publication/current source snapshot");
  const auto &Promotion=helperObject(helperObject(Publication,"report"),"source_promotion");
  for(const auto &[Field,SourceField]:std::array<std::pair<StringRef,StringRef>,4>{{
      {"candidate_sha256","sha256"},{"candidate_bytes","bytes"},
      {"candidate_device","device"},{"candidate_inode","inode"}}})
    require(Promotion.get(Field) && Snapshot.get(SourceField) &&
      *Promotion.get(Field)==*Snapshot.get(SourceField),"published bytes/inode join");
  helperBool(Promotion,"program_edited",Variant=="edit");
  helperBool(Promotion,"created_new",true);helperBool(Promotion,"fresh_compilation_required",true);
  promotedFalse(Promotion,{"original_overwritten","fresh_compilation_observed",
    "source_custody_exported","grants_artifact_or_launch_authority"});
  const auto &Joined=promotedFind(compositionArray(helperObject(Root,"candidate_joins"),"joins",3),"variant",Variant);
  require(Joined.get("fresh_checked") && *Joined.get("fresh_checked")==*Owner.get("observation") &&
    Joined.get("published_source") && *Joined.get("published_source")==*Owner.get("source"),
    "actual new candidate normal join");
  helperBool(Joined,"actual_normal_llvm_handoff_join",true);
  helperBool(Joined,"cross_candidate_prefix_refused",true);
  require(helperString(O,"stage")=="actual_checked_composition_inert_handoff" &&
    helperString(O,"source_profile")==Variant && helperInteger(O,"logical_arguments")==4 &&
    helperInteger(O,"actual_global_extent")==128 && helperInteger(O,"extra_parameter")==0 &&
    helperInteger(O,"extra_minimum_bytes")==512,"actual checked source/launch/ABI");
  helperBool(O,"fresh_checked_owner",true);helperBool(O,"extra_initialized_read",false);
  helperBool(O,"extra_write_permission",true);helperBool(O,"exact_descriptor_extension",true);
  promotedFalse(O,{"runtime_conditions_discharged","source_custody_exported","functional_equivalence_claim",
    "native_llvm_executed","hardware_observed","protected_authority"});
  for(StringRef K:{"definition_count","helper_count","call_count","occurrence_count"})
    require(helperInteger(O,K)==1,"single generated helper roster");
  const auto &Cpu=helperObject(O,"cpu");
  require(helperInteger(Cpu,"cases")==32 && helperInteger(Cpu,"view_offset")==8,"32-case CPU observation");
  helperBool(Cpu,"edited_intent",Variant=="edit");helperBool(Cpu,"canaries_and_initialization",true);
  helperBool(Cpu,"native_or_physical_execution",false);
  const auto &Baseline=helperObject(promotedFind(Sessions,"variant","original","observe"),"observation");
  const auto BaselineCpu=helperString(helperObject(Baseline,"cpu"),"output_sha256");
  require((helperString(Cpu,"output_sha256")==BaselineCpu)==(Variant!="edit") &&
    helperString(O,"canonical_sha256")!=helperString(Baseline,"canonical_sha256"),
    "fresh helper graph and independent CPU relation");
  const auto &L=helperObject(Llvm,"observation"),&H=helperObject(Handoff,"observation");
  require(helperString(L,"stage")=="public_normal_llvm" &&
    helperString(H,"stage")=="public_normal_handoff" &&
    helperString(O,"llvm_sha256")==helperString(L,"llvm_sha256") &&
    helperString(O,"handoff_sha256")==helperString(H,"handoff_sha256"),"actual normal output joins");
  In.Entry=helperString(O,"entry");In.Descriptor=In.Entry+".kd";
  require(helperSymbol(In.Entry),"kernel symbol");
  const auto &Rows=helperObject(O,"native_rows");
  require(helperString(Rows,"schema")=="fe2o3-test-composition-promoted-native-rows-v1" &&
    helperString(Rows,"source_profile")==Variant &&
    helperString(Rows,"canonical_identity")==helperString(O,"canonical_identity"),
    "distinct actual promoted canonical row identity");
  promotedFalse(Rows,{"source_custody_exported","native_correspondence_proved"});
  const auto &Registers=helperObject(Rows,"register_profile");
  require(helperInteger(Registers,"scratch")==8 && helperInteger(Registers,"output")==9,
    "promoted declared register profile");
  const auto &DeclaredInputs=compositionArray(Registers,"inputs",3);
  for(size_t I=0;I<3;++I)require(DeclaredInputs[I].getAsInteger()==std::optional<int64_t>(10+I),
    "promoted declared inputs");
  const auto &Helper=compositionRow(compositionArray(Rows,"helpers",1)[0]);
  const auto HelperName=helperString(Helper,"symbol");
  const auto HelperOrdinal=helperInteger(Helper,"function_ordinal");
  const auto RootOrdinal=helperInteger(Rows,"root_function_ordinal");
  require(helperInteger(Helper,"key")==0 && HelperOrdinal<2 && RootOrdinal<2 &&
    HelperOrdinal!=RootOrdinal && helperSymbol(HelperName) && HelperName!=In.Entry,
    "actual helper key/function");
  In.Helpers.push_back(HelperName);
  const auto &Definition=compositionRow(compositionArray(Rows,"definitions",1)[0]);
  require(helperInteger(Definition,"key")==0 && compositionSite(Definition)==HelperName &&
    helperInteger(helperObject(Definition,"site"),"function_ordinal")==HelperOrdinal &&
    helperInteger(Definition,"scratch")==8 && helperInteger(Definition,"output")==9,
    "actual helper program owner/roles");
  const auto &Inputs=compositionArray(Definition,"inputs",3);
  for(size_t I=0;I<3;++I)require(Inputs[I].getAsInteger()==std::optional<int64_t>(10+I),
    "actual program physical inputs");
  const std::vector<uint16_t> Literal=Variant=="edit"?std::vector<uint16_t>{40}:std::vector<uint16_t>{133,315};
  const auto &Descriptors=compositionArray(Definition,"descriptors",Literal.size());
  const auto &Exact=compositionArray(Rows,"exact_descriptors",Literal.size());
  for(size_t I=0;I<Literal.size();++I)
    require(Descriptors[I].getAsInteger()==std::optional<int64_t>(Literal[I]) && Exact[I]==Descriptors[I],
      "independent promoted literal descriptors");
  R.Definitions.push_back({HelperName,Literal});
  const auto &Call=compositionRow(compositionArray(Rows,"calls",1)[0]);
  require(helperInteger(Call,"key")==0 && helperInteger(Call,"callee")==0 &&
    compositionSite(Call)==In.Entry && helperInteger(helperObject(Call,"site"),"function_ordinal")==RootOrdinal,
    "actual generated-helper root call");
  R.Calls.push_back(HelperName);
  const auto &Occurrence=compositionRow(compositionArray(Rows,"occurrences",1)[0]);
  require(helperInteger(Occurrence,"key")==0 && helperInteger(Occurrence,"definition")==0 &&
    helperInteger(Occurrence,"root")==RootOrdinal && helperInteger(Occurrence,"incoming_call")==0,
    "actual root-qualified helper occurrence");
  R.Expanded[compositionKind(Literal,R.Profile)]=1;
  const std::string Base=Report.substr(0,Report.rfind('/')).str();
  const std::string Prefix=Base+"/"+Variant.str()+"-observe.output/";
  (void)promotedPin(In,Prefix+"canonical-v17.bin",O,"canonical_sha256");
  In.Llvm.Kind=InputKind::LlvmTextIr;
  In.Llvm.Bytes=promotedPin(In,Base+"/"+Variant.str()+"-llvm.output",O,"llvm_sha256",1024*1024);
  require(In.Llvm.Bytes==promotedPin(In,Prefix+"canonical.ll",O,"llvm_sha256",1024*1024),
    "unchanged exact public/canonical LLVM");
  In.Llvm.Digest=SHA256::hash(In.Llvm.Bytes);
  const auto D=promotedPin(In,Prefix+"descriptor-v1.bin",O,"descriptor_sha256",65536);
  const auto Worker=helperRead(Prefix+"worker.ll",4*1024*1024);
  promotedDescriptorText(In.Llvm.Bytes,Worker,D);
  (void)helperPinned(In,Prefix+"worker.ll",hex(SHA256::hash(Worker)),4*1024*1024,Worker.size());
  require(promotedPin(In,Base+"/"+Variant.str()+"-handoff.output",O,"handoff_sha256")==
    promotedPin(In,Prefix+"handoff-v2.bin",O,"handoff_sha256"),"exact ordinary handoff bytes");
  promotedSource(P,Base+"/"+Package+"/src/ordered_composition_publish_v1.rs",Snapshot);
  promotedSource(P,Base+"/package-original/src/ordered_composition_publish_v1.rs",helperObject(Root,"original_source"));
  (void)promotedPin(In,Base+"/"+Variant.str()+".request.json",Promotion,"request_sha256",8192);
  for(const auto &[Suffix,Field,Cap]:std::array<std::tuple<StringRef,StringRef,size_t>,3>{{
      {"Cargo.toml","manifest_sha256",65536},{"Cargo.lock","lock_sha256",1024*1024},
      {"src/lib.rs","root_sha256",4096}}}) {
    (void)promotedPin(In,Base+"/"+Package+"/"+Suffix.str(),Invocation,Field,Cap);
  }
  (void)promotedPin(In,Base+"/"+Package+".preparation/metadata.stdout",Invocation,"metadata_sha256",16*1024*1024);
  (void)promotedPin(In,Base+"/"+Package+".preparation/sysroot.stdout",Invocation,"sysroot_sha256",4096);
  (void)promotedPin(In,Repository.str()+"/Cargo.lock",Invocation,"workspace_lock_sha256",1024*1024);
  require(Invocation.get("provider_sources") && Root.get("provider_sources") &&
    *Invocation.get("provider_sources")==*Root.get("provider_sources"),"actual provider census");
  for(const auto &V:compositionArray(Root,"provider_sources",6)) {
    const auto &S=compositionRow(V);const auto Path=helperString(S,"path");
    require(StringRef(Path).starts_with("crates/") && Path.find("..")==std::string::npos &&
      Path.find("//")==std::string::npos && Path.size()<512,"provider relative path");
    (void)helperPinned(In,Repository.str()+"/"+Path,helperString(S,"sha256"),1024*1024,helperInteger(S,"bytes"));
  }
  promotedRecheck(P);return P;
}
