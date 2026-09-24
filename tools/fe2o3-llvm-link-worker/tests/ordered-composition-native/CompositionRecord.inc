// Exact report/file joins remain observations, never typed source authority.
enum class CompositionProfile { LegacyV32, PromotedV8 };
struct CompositionDefinition { std::string Symbol; std::vector<uint16_t> Descriptors; };
struct CompositionRecord {
  HelperInput Input;
  CompositionProfile Profile = CompositionProfile::LegacyV32;
  std::vector<CompositionDefinition> Definitions;
  std::vector<std::string> Calls;
  std::array<size_t,3> Expanded{}; // pair, mov input0, mov input1
  std::array<std::string,5> ArgumentNames;
};
size_t compositionPatternCount(CompositionProfile P) {
  return P==CompositionProfile::LegacyV32?3:2;
}
size_t compositionKind(ArrayRef<uint16_t> D,CompositionProfile P=CompositionProfile::LegacyV32) {
  if(D.size()==2 && D[0]==133 && D[1]==315) return 0;
  if(P==CompositionProfile::PromotedV8) {
    if(D.size()==1 && D[0]==40)return 1;
  } else {
    if(D.size()==1 && D[0]==8)return 1;
    if(D.size()==1 && D[0]==24)return 2;
  }
  fail("canonical rows outside independent literal composition source programs");
}
std::array<size_t,4> compositionRoster(StringRef Feature) {
  if(Feature=="ordered-composition-root") return {2,0,0,2};
  if(Feature=="ordered-composition-helper" || Feature=="ordered-composition-wrapping" ||
     Feature=="ordered-composition-scalar-helper") return {1,1,1,1};
  if(Feature=="ordered-composition-two-calls") return {1,1,2,2};
  if(Feature=="ordered-composition-root-helper") return {2,1,1,2};
  if(Feature=="ordered-composition-const-monos") return {2,2,2,2};
  fail("unknown finite source feature");
}
const json::Array &compositionArray(const json::Object &O,StringRef Key,size_t Count) {
  const auto *A=O.getArray(Key);require(A && A->size()==Count,"composition exact array census");return *A;
}
const json::Object &compositionRow(const json::Value &V) {
  const auto *O=V.getAsObject();require(O,"composition row is not object");return *O;
}
std::string compositionSite(const json::Object &O) {
  const auto &S=helperObject(O,"site");
  require(helperInteger(S,"function_ordinal")<3 && helperInteger(S,"block_ordinal")<128 &&
    helperInteger(S,"block")<=UINT32_MAX && helperInteger(S,"operation_ordinal")<256,
    "bounded canonical definition/call coordinate");
  const auto Name=helperString(S,"symbol");require(helperSymbol(Name),"canonical row symbol");return Name;
}
CompositionRecord compositionRecord(StringRef Report,StringRef ReportHash,
    StringRef Repository,StringRef Feature) {
  CompositionRecord R;auto &In=R.Input;In.Label=Feature.str();
  const auto Expected=compositionRoster(Feature);
  auto Bytes=helperPinned(In,Report,ReportHash,4*1024*1024);
  auto Parsed=json::parse(StringRef(reinterpret_cast<const char*>(Bytes.data()),Bytes.size()));
  require(bool(Parsed),"composition report JSON");const auto &Root=compositionRow(*Parsed);
  require(helperString(Root,"schema")=="fe2o3-test-composition-normal-ladder-v1" &&
    helperInteger(Root,"actual_source_sessions")==36 &&
    helperInteger(Root,"same_source_normal_llvm_handoff_joins")==7,"completed 36-source report");
  for(StringRef Key:{"native_llvm_executed","hardware_observed","source_custody_exported",
      "runtime_conditions_discharged","protected_authority"}) helperBool(Root,Key,false);
  const auto &Observations=compositionArray(Root,"observations",36);
  const json::Object *Owner=nullptr,*Llvm=nullptr,*Handoff=nullptr;
  for(const auto &V:Observations) {
    const auto &S=compositionRow(V);
    if(helperString(S,"feature")!=Feature || S.getBoolean("finite")!=std::optional<bool>(true))continue;
    require(helperString(S,"schema")=="fe2o3-test-composition-normal-session-v1","session schema");
    const auto Mode=helperString(S,"mode");
    const json::Object **Slot=Mode=="observe"?&Owner:Mode=="llvm"?&Llvm:Mode=="handoff"?&Handoff:nullptr;
    require(Slot && !*Slot,"selected session mode/duplicate");*Slot=&S;
  }
  require(Owner && Llvm && Handoff,"selected finite source session triple");
  const auto &O=helperObject(*Owner,"observation");
  const auto &L=helperObject(*Llvm,"observation"),&H=helperObject(*Handoff,"observation");
  for(const auto *Session:{Owner,Llvm,Handoff}) {
    const auto &I=helperObject(*Session,"invocation");
    require(I.getBoolean("finite")==std::optional<bool>(true),"finite actual invocation");
    const auto &Original=helperObject(I,"original");
    require(helperString(Original,"feature")==Feature && Original.get("sources") &&
      Root.get("sources") && *Original.get("sources")==*Root.get("sources"),"actual source pin relation");
    const auto &First=helperObject(*Owner,"invocation");
    for(StringRef Key:{"args","crate_binding","cargo_observation","original"})
      require(I.get(Key) && First.get(Key) && *I.get(Key)==*First.get(Key),"same actual selected source invocation");
  }
  require(helperString(O,"stage")=="actual_checked_composition_inert_handoff" &&
    helperString(L,"stage")=="public_normal_llvm" && helperString(H,"stage")=="public_normal_handoff" &&
    helperString(O,"llvm_sha256")==helperString(L,"llvm_sha256") &&
    helperString(O,"handoff_sha256")==helperString(H,"handoff_sha256"),"normal output hash relation");
  for(StringRef Key:{"runtime_conditions_discharged","source_custody_exported","functional_equivalence_claim",
      "native_llvm_executed","hardware_observed","protected_authority"})helperBool(O,Key,false);
  helperBool(O,"exact_descriptor_extension",true);
  require(helperInteger(O,"logical_arguments")==4 && helperInteger(O,"actual_global_extent")==128 &&
    helperInteger(O,"extra_parameter")==0 && helperInteger(O,"extra_minimum_bytes")==512 &&
    O.getBoolean("extra_initialized_read")==std::optional<bool>(false) &&
    O.getBoolean("extra_write_permission")==std::optional<bool>(true),"unresolved launch envelope not discharged");
  In.Entry=helperString(O,"entry");In.Descriptor=In.Entry+".kd";require(helperSymbol(In.Entry),"actual kernel symbol");
  const auto &Rows=helperObject(O,"native_rows");
  require(helperString(Rows,"schema")=="fe2o3-test-composition-native-rows-v1" &&
    helperString(Rows,"canonical_identity")==helperString(O,"canonical_identity"),"canonical observation identity");
  helperBool(Rows,"source_custody_exported",false);helperBool(Rows,"native_correspondence_proved",false);
  const auto &Helpers=compositionArray(Rows,"helpers",Expected[1]);
  for(size_t I=0;I<Helpers.size();++I) {
    const auto &Row=compositionRow(Helpers[I]);const auto Name=helperString(Row,"symbol");
    require(helperInteger(Row,"key")==I && helperInteger(Row,"function_ordinal")<3 &&
      helperSymbol(Name) && Name!=In.Entry,"canonical helper key/symbol");In.Helpers.push_back(Name);
  }
  const auto OriginalHelpers=In.Helpers;
  std::sort(In.Helpers.begin(),In.Helpers.end());
  require(std::adjacent_find(In.Helpers.begin(),In.Helpers.end())==In.Helpers.end(),"duplicate canonical helper");
  const auto &Definitions=compositionArray(Rows,"definitions",Expected[0]);
  for(size_t I=0;I<Definitions.size();++I) {
    const auto &Row=compositionRow(Definitions[I]);CompositionDefinition Def;
    Def.Symbol=compositionSite(Row);
    require(helperInteger(Row,"key")==I && helperInteger(Row,"scratch")==32 &&
      helperInteger(Row,"output")==33 &&
      (Def.Symbol==In.Entry || std::binary_search(In.Helpers.begin(),In.Helpers.end(),Def.Symbol)),
      "canonical program ownership/roles");
    const auto &Inputs=compositionArray(Row,"inputs",3);
    for(size_t J=0;J<3;++J)require(Inputs[J].getAsInteger()==std::optional<int64_t>(34+J),"exact physical live-ins");
    const auto *Descriptors=Row.getArray("descriptors");require(Descriptors && Descriptors->size()<=2,"source program size");
    for(const auto &D:*Descriptors) {auto N=D.getAsInteger();require(N && *N>=0 && *N<=UINT16_MAX,"instruction descriptor");Def.Descriptors.push_back(*N);}
    (void)compositionKind(Def.Descriptors);R.Definitions.push_back(std::move(Def));
  }
  const auto &Calls=compositionArray(Rows,"calls",Expected[2]);
  for(size_t I=0;I<Calls.size();++I) {
    const auto &Row=compositionRow(Calls[I]);const auto Callee=helperInteger(Row,"callee");
    require(helperInteger(Row,"key")==I && compositionSite(Row)==In.Entry && Callee<OriginalHelpers.size(),
      "actual root-only canonical call");R.Calls.push_back(OriginalHelpers[Callee]);
  }
  const auto &Occurrences=compositionArray(Rows,"occurrences",Expected[3]);
  for(size_t I=0;I<Occurrences.size();++I) {
    const auto &Row=compositionRow(Occurrences[I]);const auto Def=helperInteger(Row,"definition");
    require(helperInteger(Row,"key")==I && Def<R.Definitions.size() &&
      helperInteger(Row,"root")==helperInteger(Rows,"root_function_ordinal"),"occurrence key/owner");
    auto Call=Row.getInteger("incoming_call");
    if(Call) require(*Call>=0 && size_t(*Call)<R.Calls.size() && R.Calls[*Call]==R.Definitions[Def].Symbol,"actual occurrence call/definition");
    else require(Row.get("incoming_call") && Row.get("incoming_call")->kind()==json::Value::Null &&
      R.Definitions[Def].Symbol==In.Entry,"root occurrence");
    ++R.Expanded[compositionKind(R.Definitions[Def].Descriptors)];
  }
  const auto Literal=Feature=="ordered-composition-const-monos"?std::array<size_t,3>{0,1,1}:
    std::array<size_t,3>{Expected[3],0,0};
  require(R.Expanded==Literal,"independent source program occurrence census");
  const std::string Base=Report.substr(0,Report.rfind('/')).str();
  const std::string Prefix=Base+"/"+Feature.str()+".finite.";
  const auto Canonical=helperPinned(In,Prefix+"observe.output/canonical-v17.bin",helperString(O,"canonical_sha256"),4*1024*1024);
  (void)Canonical;
  In.Llvm.Kind=InputKind::LlvmTextIr;
  In.Llvm.Bytes=helperPinned(In,Prefix+"llvm.output",helperString(O,"llvm_sha256"),1024*1024);
  require(In.Llvm.Bytes==helperPinned(In,Prefix+"observe.output/canonical.ll",helperString(O,"llvm_sha256"),1024*1024),"unchanged canonical LLVM");
  In.Llvm.Digest=SHA256::hash(In.Llvm.Bytes);
  (void)helperPinned(In,Prefix+"handoff.output",helperString(O,"handoff_sha256"),4*1024*1024);
  (void)helperPinned(In,Prefix+"observe.output/handoff-v2.bin",helperString(O,"handoff_sha256"),4*1024*1024);
  (void)helperPinned(In,Prefix+"observe.output/descriptor-v1.bin",helperString(O,"descriptor_sha256"),65536);
  const auto &Sources=compositionArray(Root,"sources",6);
  for(const auto &V:Sources) {
    const auto &S=compositionRow(V);const auto Path=helperString(S,"path");
    require(StringRef(Path).starts_with("crates/") && Path.find("..")==std::string::npos &&
      Path.find("//")==std::string::npos && Path.size()<512,"source relative path");
    (void)helperPinned(In,Repository.str()+"/"+Path,helperString(S,"sha256"),1024*1024,helperInteger(S,"bytes"));
  }
  return R;
}
