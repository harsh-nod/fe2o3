// Closed fresh source-role observation joins; files remain inert evidence.
namespace transport = fe2o3::worker::transport_test_v1;
struct TransportInputV1 {
  PromotedInput Normal;
  HelperInput Extra;
  transport::SourceProjection Projection;
  std::string ReportHash;
};
json::Value transportJson(ArrayRef<uint8_t> B) {
  auto P=json::parse(StringRef(reinterpret_cast<const char*>(B.data()),B.size()));
  require(bool(P),"transport JSON");return std::move(*P);
}
std::array<uint8_t,32> transportDigest(StringRef S) {
  require(helperDigest(S),"transport digest syntax");
  std::array<uint8_t,32> Out{};
  const auto N=[](char C){return C<='9'?C-'0':C-'a'+10;};
  for(size_t I=0;I<32;++I)Out[I]=uint8_t((N(S[2*I])<<4)|N(S[2*I+1]));
  return Out;
}
// Pure identity relation, tested independently without file reads or source owners.
bool transportRoleJoin(const json::Object &T,const json::Object &N) {
  for(StringRef K:{"canonical_identity","canonical_sha256","llvm_sha256",
      "descriptor_sha256","handoff_sha256","semantic_identity","source_profile"})
    if(!T.get(K)||!N.get(K)||*T.get(K)!=*N.get(K))return false;
  if(!T.getObject("cpu")||!N.getObject("cpu"))return false;
  for(StringRef K:{"cases","view_offset","edited_intent","canaries_and_initialization",
      "native_or_physical_execution","output_sha256"}) {
    const auto &A=*T.getObject("cpu"),&B=*N.getObject("cpu");
    if(!A.get(K)||!B.get(K)||*A.get(K)!=*B.get(K))return false;
  }
  return T.getString("entry_symbol")==N.getString("entry") &&
    T.getString("root_symbol")==N.getString("entry") &&
    T.getString("entry_symbol").has_value();
}
bool transportSessionJoin(const json::Object &T,const json::Object &N) {
  for(StringRef K:{"invocation","source"})
    if(!T.get(K)||!N.get(K)||*T.get(K)!=*N.get(K))return false;
  return true;
}
TransportInputV1 transportRecord(StringRef Report,StringRef Hash,StringRef Repository,
                                StringRef Variant,StringRef Level) {
  require(Variant=="copy"||Variant=="preserve"||Variant=="edit","transport variant");
  require(Level=="O0"||Level=="O3","transport optimization");
  TransportInputV1 T;T.ReportHash=Hash.str();
  const auto Bytes=helperPinned(T.Extra,Report,Hash,1024*1024);
  const auto Parsed=transportJson(Bytes);const auto &Root=compositionRow(Parsed);
  require(helperString(Root,"schema")=="fe2o3-test-composition-transport-ladder-v1",
    "distinct completed source transport report required");
  for(const auto &[K,N]:std::array<std::pair<StringRef,uint64_t>,9>{{
      {"workload_children",26},{"actual_extractor_invocations",13},{"actual_normal_sessions",10},
      {"actual_transport_sessions",3},{"source_publications",3},{"normal_cpu_cases",128},
      {"transport_cpu_cases",96},{"role_substitution_refusals",54},
      {"transport_descriptor_extension_refusals",9}}})
    require(helperInteger(Root,K)==N,"completed transport source census");
  helperBool(Root,"fresh_source_custody_constructed_in_callback",true);
  helperBool(Root,"cpu_counts_overlap_recompiled_source_graphs",true);
  promotedFalse(Root,{"source_custody_from_files","historical_evidence_rewritten",
    "machine_abi_transport_proved","native_functional_equivalence",
    "runtime_conditions_discharged","source_custody_exported","hardware_observed","protected_authority"});
  require(Report.ends_with("/transport-observation.json"),"fixed source report leaf");
  const auto Base=Report.drop_back(StringRef("/transport-observation.json").size()).str();
  const auto NormalPath=Base+"/observation.json";
  const auto NormalBytes=helperPinned(T.Extra,NormalPath,helperString(Root,"normal_report_sha256"),
    1024*1024,helperInteger(Root,"normal_report_bytes"));
  const auto NormalParsed=transportJson(NormalBytes);
  require(Root.get("normal_report") && *Root.get("normal_report")==NormalParsed,
    "exact embedded and retained fresh normal report");
  const auto &Normal=compositionRow(NormalParsed);
  T.Normal=promotedRecord(NormalPath,helperString(Root,"normal_report_sha256"),Repository,Variant);
  const auto &Session=promotedFind(compositionArray(Root,"transport_sessions",3),"variant",Variant);
  const auto &N=promotedFind(compositionArray(Normal,"normal_sessions",10),"variant",Variant,"observe");
  require(transportSessionJoin(Session,N),"same actual source/invocation normal and role sessions");
  require(helperString(Session,"schema")=="fe2o3-test-composition-transport-roles-v1" &&
    helperString(Session,"mode")=="transport-roles","role child route");
  helperBool(Session,"actual_fresh_frontend",true);
  promotedFalse(Session,{"runtime_conditions_discharged","source_custody_exported","hardware_observed","protected_authority"});
  const auto &O=helperObject(Session,"observation"),&NO=helperObject(N,"observation");
  require(transportRoleJoin(O,NO),"same fresh canonical/LLVM/descriptor/source role identity");
  require(helperString(O,"schema")=="fe2o3-test-composition-transport-roles-v1" &&
    helperString(O,"ancestry_language")=="direct intra-function SSA plus actual positional call/return" &&
    helperInteger(O,"call_key")==0 && helperInteger(O,"definition_key")==0 &&
    helperInteger(O,"occurrence_key")==0 && helperInteger(O,"role_substitution_refusals")==18 &&
    helperInteger(O,"descriptor_extension_refusals")==3,"bounded actual source role route");
  helperBool(O,"compiler_owner_replayed_by_normal_descriptor",true);
  helperBool(O,"pointer_and_predicate_recorded_not_proved",true);
  promotedFalse(O,{"machine_abi_transport_proved","native_functional_equivalence",
    "runtime_conditions_discharged","source_custody_exported","hardware_observed","protected_authority"});
  require(helperString(O,"helper_symbol")==T.Normal.Record.Input.Helpers.at(0),
    "same actual generated helper symbol");
  const auto &Abi=helperObject(O,"descriptor_abi"),&Condition=helperObject(O,"output_condition");
  require(helperInteger(Abi,"explicit_argument_size")==32 && helperInteger(Abi,"kernarg_segment_size")==288 &&
    helperInteger(Abi,"kernarg_segment_alignment")==8 && helperInteger(Condition,"parameter")==0 &&
    helperInteger(Condition,"minimum_bytes")==512,"exact source descriptor ABI/condition");
  helperBool(Condition,"initialized_read",false);helperBool(Condition,"write_permission",true);
  const auto &Pub=helperObject(O,"publication");
  const auto &Action=promotedFind(compositionArray(Normal,"actions",13),"case",Variant);
  require(Pub.get("candidate") && *Pub.get("candidate")==*Session.get("source"),"published source snapshot");
  helperBool(Pub,"inert_metadata_join_only",true);
  const auto Publication=helperPinned(T.Extra,Base+"/cli-"+Variant.str()+".accepted.json",
    helperString(Pub,"report_sha256"),65536,helperInteger(Pub,"report_bytes"));
  const auto PubParsed=transportJson(Publication);
  require(compositionRow(PubParsed)==Action,"actual publication report bytes");
  (void)helperPinned(T.Extra,Base+"/"+Variant.str()+".request.json",helperString(Pub,"request_sha256"),
    8192,helperInteger(Pub,"request_bytes"));
  const auto Prefix=Base+"/"+Variant.str()+"-transport-roles.output/";
  const auto RoleBytes=helperRead(Prefix+"transport-roles.json",65536);
  require(transportJson(RoleBytes)==*Session.get("observation"),"exact standalone role observation");
  (void)helperPinned(T.Extra,Prefix+"transport-roles.json",hex(SHA256::hash(RoleBytes)),65536,RoleBytes.size());
  for(const auto &[Leaf,Key,Limit]:std::array<std::tuple<StringRef,StringRef,size_t>,5>{{
      {"canonical-v17.bin","canonical",65536},{"canonical.ll","llvm",1024*1024},
      {"descriptor-v1.bin","descriptor",65536},{"worker.ll","worker",1024*1024},
      {"handoff-v2.bin","handoff",1024*1024}}}) {
    const auto B=helperPinned(T.Extra,Prefix+Leaf.str(),helperString(O,Key.str()+"_sha256"),
      Limit,helperInteger(O,Key.str()+"_bytes"));
    const auto Other=helperRead(Base+"/"+Variant.str()+"-observe.output/"+Leaf.str(),Limit);
    require(B==Other,"actual role and ordinary owner exports must be byte identical");
  }
  auto &P=T.Projection;
  const auto Count=helperInteger(O,"row_count");
  require(Count==(Variant=="edit"?41:42),"finite exact source role rows");
  const auto &Rows=compositionArray(O,"rows",Count);P.RowCount=uint16_t(Count);
  for(size_t I=0;I<Rows.size();++I) {
    const auto *Row=Rows[I].getAsArray();require(Row&&Row->size()==4,"source role row width");
    for(size_t J=0;J<4;++J) {
      const auto V=(*Row)[J].getAsInteger();
      require(V&&*V>=0&&uint64_t(*V)<=UINT32_MAX,"source role u32");
      P.Rows[I][J]=uint32_t(*V);
    }
  }
  P.Canonical=transportDigest(helperString(O,"canonical_identity"));
  P.Llvm=transportDigest(helperString(O,"llvm_sha256"));
  P.Descriptor=transportDigest(helperString(O,"descriptor_sha256"));
  P.SourceReport=transportDigest(Hash);
  P.Selected=Level=="O0"?transport::Profile::ConditionalHelperO0:transport::Profile::InlineRootO3;
  P.SelectedProgram=Variant=="edit"?transport::Program::MoveInput2:transport::Program::XorAnd;
  // Root/Helper StringRefs are assigned only after the returned owner reaches
  // its final caller location; moving SSO strings must not leave dangling refs.
  promotedRecheck(T.Normal);helperRecheck(T.Extra);return T;
}
