// Inert record joins only. Neither a JSON record nor these hashes create source custody.
struct HelperPin { std::string Path,Sha; size_t Bytes; };
struct HelperInput {
  Input Llvm;
  std::string Label,Entry,Descriptor;
  std::vector<std::string> Helpers;
  std::vector<HelperPin> Pins;
};
std::string helperString(const json::Object &O,StringRef Key) {
  auto V=O.getString(Key); require(V.has_value(),"missing helper record string"); return V->str();
}
uint64_t helperInteger(const json::Object &O,StringRef Key) {
  auto V=O.getInteger(Key); require(V && *V>=0,"missing nonnegative helper record number");
  return static_cast<uint64_t>(*V);
}
void helperBool(const json::Object &O,StringRef Key,bool Expected) {
  auto V=O.getBoolean(Key); require(V && *V==Expected,"helper record boolean mismatch");
}
bool helperDigest(StringRef S) {
  return S.size()==64 && llvm::all_of(S,[](char C){return (C>='0'&&C<='9')||(C>='a'&&C<='f');});
}
bool helperSymbol(StringRef S,bool Descriptor=false) {
  if(Descriptor && !S.consume_back(".kd")) return false;
  if(S.empty() || S.size()>128 || !((S.front()>='A'&&S.front()<='Z')||
      (S.front()>='a'&&S.front()<='z')||S.front()=='_')) return false;
  return llvm::all_of(S,[](char C){return (C>='A'&&C<='Z')||(C>='a'&&C<='z')||
      (C>='0'&&C<='9')||C=='_';});
}
std::vector<uint8_t> helperRead(StringRef Path,size_t Limit) {
  require(Path.starts_with("/") && Path.size()<=4096 && !Path.contains('\0') &&
      llvm::none_of(Path,[](char C){return static_cast<unsigned char>(C)<32||C==127;}),
      "helper input path bound");
  char Canonical[PATH_MAX];
  require(realpath(Path.str().c_str(),Canonical)!=nullptr && Path==Canonical,
      "helper input must be canonical without symlinks");
  int FD=open(Path.str().c_str(),O_RDONLY|O_CLOEXEC|O_NOFOLLOW);
  require(FD>=0,"helper input open");
  struct stat Before{},After{};
  require(fstat(FD,&Before)==0 && S_ISREG(Before.st_mode) && Before.st_size>0 &&
      static_cast<uint64_t>(Before.st_size)<=Limit,"helper input regular-file size");
  std::vector<uint8_t> Bytes(static_cast<size_t>(Before.st_size)); size_t At=0;
  while(At<Bytes.size()) {
    ssize_t N=read(FD,Bytes.data()+At,Bytes.size()-At);
    if(N<0 && errno==EINTR) continue;
    require(N>0,"helper input short read"); At+=static_cast<size_t>(N);
  }
  uint8_t Extra=0; ssize_t N;
  do { N=read(FD,&Extra,1); } while(N<0 && errno==EINTR);
  require(N==0 && fstat(FD,&After)==0 && Before.st_dev==After.st_dev &&
      Before.st_ino==After.st_ino && Before.st_size==After.st_size &&
      Before.st_mtim.tv_sec==After.st_mtim.tv_sec && Before.st_mtim.tv_nsec==After.st_mtim.tv_nsec &&
      Before.st_ctim.tv_sec==After.st_ctim.tv_sec && Before.st_ctim.tv_nsec==After.st_ctim.tv_nsec,
      "helper input changed during read");
  require(close(FD)==0,"helper input close"); return Bytes;
}
std::vector<uint8_t> helperPinned(HelperInput &In,StringRef Path,StringRef Digest,
    size_t Limit,std::optional<uint64_t> Expected=std::nullopt) {
  require(helperDigest(Digest) && In.Pins.size()<12,"helper pin bounds");
  auto Bytes=helperRead(Path,Limit);
  require(hex(SHA256::hash(Bytes))==Digest && (!Expected || Bytes.size()==*Expected),
      "helper retained input identity mismatch");
  In.Pins.push_back({Path.str(),Digest.str(),Bytes.size()}); return Bytes;
}
const json::Object &helperObject(const json::Object &O,StringRef Key) {
  auto *V=O.getObject(Key); require(V!=nullptr,"helper record object"); return *V;
}
HelperInput helperRecord(StringRef Path,StringRef Digest,StringRef Label) {
  require(Label=="default256"||Label=="edited512"||Label=="repeat"||Label=="two","closed helper label");
  HelperInput In; In.Label=Label.str();
  auto Bytes=helperPinned(In,Path,Digest,4*1024*1024);
  auto Parsed=json::parse(StringRef(reinterpret_cast<const char*>(Bytes.data()),Bytes.size()));
  require(static_cast<bool>(Parsed),"helper ladder JSON parse");
  auto *Root=Parsed->getAsObject(); require(Root!=nullptr,"helper ladder JSON root");
  require(helperString(*Root,"schema")=="fe2o3-generated-helper-normal-driver-ladder-v30" &&
      helperInteger(*Root,"normal_driver_runs")==8,"helper source ladder schema");
  helperBool(*Root,"exact_source_unchanged",true);
  helperBool(*Root,"normal_llvm_handoff_descriptor_extension_join",true);
  for(StringRef Key:{"native_llvm_executed","hardware_observed","physical_helper_abi_qualified",
      "protected_finalizer_admitted","grants_artifact_or_launch_authority","milestone_completion"})
    helperBool(*Root,Key,false);
  auto *Descriptors=Root->getArray("descriptor_observations");
  auto *Observations=Root->getArray("observations");
  require(Descriptors && Descriptors->size()==4 && Observations && Observations->size()==8,
      "exact helper ladder observation counts");
  const json::Object *Selected=nullptr,*Llvm=nullptr,*Handoff=nullptr;
  for(const auto &V:*Descriptors) {
    auto *O=V.getAsObject(); require(O!=nullptr,"descriptor observation object");
    if(helperString(*O,"label")!=Label) continue;
    require(!Selected,"duplicate helper descriptor observation"); Selected=O;
  }
  for(const auto &V:*Observations) {
    auto *O=V.getAsObject(); require(O!=nullptr,"source observation object");
    if(helperString(*O,"label")!=Label) continue;
    auto Mode=helperString(*O,"mode");
    require(Mode=="llvm"||Mode=="handoff","helper observation mode");
    auto &Slot=(Mode=="llvm"?Llvm:Handoff);
    require(!Slot,"duplicate helper source observation"); Slot=O;
    require(helperString(*O,"schema")=="fe2o3-generated-helper-normal-driver-observation-v30",
        "helper child observation schema");
    helperBool(*O,"normal_source_driver_completed",true); helperBool(*O,"outputs_are_inert",true);
    for(StringRef Key:{"native_llvm_executed","hardware_observed",
        "protected_finalizer_admitted","grants_artifact_or_launch_authority"}) helperBool(*O,Key,false);
  }
  require(Selected && Llvm && Handoff,"selected helper observations absent");
  const auto &S=*Selected;
  require(helperString(S,"target")=="gfx942:xnack-" && helperInteger(S,"code_object_version")==6,
      "helper target/COV mismatch");
  for(StringRef Key:{"exact_pre_descriptor_prefix","exact_canonical_descriptor_extension",
      "descriptor_kernel_and_symbol_roster_join"}) helperBool(S,Key,true);
  helperBool(S,"authenticates_compiler_origin",false); helperBool(S,"grants_launch_authority",false);
  In.Entry=helperString(S,"kernel_entry"); In.Descriptor=helperString(S,"kernel_descriptor");
  require(helperSymbol(In.Entry) && In.Descriptor==In.Entry+".kd" && helperSymbol(In.Descriptor,true),
      "helper exact entry/descriptor symbol");
  auto *Roles=S.getArray("source_symbol_roles");
  const size_t HelperCount=Label=="two"?2:1;
  require(Roles && Roles->size()==HelperCount+2,"exact selected source symbol count");
  size_t Entries=0,Kds=0;
  for(const auto &V:*Roles) {
    auto *O=V.getAsObject(); require(O && O->size()==2,"source role/name row");
    const auto Role=helperString(*O,"role"),Name=helperString(*O,"symbol");
    if(Role=="kernel_entry") { require(Name==In.Entry,"source entry mismatch"); ++Entries; }
    else if(Role=="kernel_descriptor") { require(Name==In.Descriptor,"source descriptor mismatch"); ++Kds; }
    else { require(Role=="internal_helper" && helperSymbol(Name) && Name!=In.Entry,
        "unexpected source symbol role"); In.Helpers.push_back(Name); }
  }
  std::sort(In.Helpers.begin(),In.Helpers.end());
  require(Entries==1 && Kds==1 && In.Helpers.size()==HelperCount &&
      std::adjacent_find(In.Helpers.begin(),In.Helpers.end())==In.Helpers.end(),"helper roster duplicate");
  require(helperString(S,"pre_descriptor_llvm_sha256")==helperString(*Llvm,"llvm_sha256") &&
      helperInteger(S,"pre_descriptor_llvm_bytes")==helperInteger(*Llvm,"llvm_bytes") &&
      helperString(S,"pre_descriptor_llvm_sha256")==helperString(*Llvm,"output_sha256") &&
      helperInteger(S,"pre_descriptor_llvm_bytes")==helperInteger(*Llvm,"output_bytes") &&
      helperString(S,"handoff_sha256")==helperString(*Handoff,"output_sha256") &&
      helperInteger(S,"handoff_bytes")==helperInteger(*Handoff,"output_bytes") &&
      helperString(S,"worker_llvm_sha256")==helperString(*Handoff,"llvm_sha256") &&
      helperInteger(S,"worker_llvm_bytes")==helperInteger(*Handoff,"llvm_bytes"),"source/descriptor byte-domain join");
  In.Llvm.Kind=InputKind::LlvmTextIr;
  In.Llvm.Bytes=helperPinned(In,helperString(S,"pre_descriptor_llvm_path"),
      helperString(S,"pre_descriptor_llvm_sha256"),1024*1024,helperInteger(S,"pre_descriptor_llvm_bytes"));
  In.Llvm.Digest=SHA256::hash(In.Llvm.Bytes);
  (void)helperPinned(In,helperString(S,"handoff_path"),helperString(S,"handoff_sha256"),
      2*1024*1024,helperInteger(S,"handoff_bytes"));
  (void)helperPinned(In,helperString(S,"descriptor_source_path"),helperString(S,"descriptor_source_sha256"),
      64*1024,helperInteger(S,"descriptor_source_bytes"));
  const auto &Invocation=helperObject(*Llvm,"invocation");
  require(Llvm->get("invocation") && Handoff->get("invocation") &&
      *Llvm->get("invocation")==*Handoff->get("invocation"),"helper source invocation mismatch");
  require(helperString(Invocation,"schema")=="fe2o3-generated-helper-source-invocation-v30" &&
      helperString(Invocation,"label")==Label,"helper invocation schema/label");
  const auto &Snapshot=helperObject(Invocation,"snapshot");
  const auto Fixture=helperString(Snapshot,"fixture");
  for(const auto &[Leaf,Key,Limit]:std::array<std::tuple<StringRef,StringRef,size_t>,3>{{
      {"src/lib.rs","source_sha256",65536},{"Cargo.toml","manifest_sha256",65536},
      {"Cargo.lock","lock_sha256",1024*1024}}})
    (void)helperPinned(In,Fixture+"/"+Leaf.str(),helperString(Snapshot,Key),Limit);
  return In;
}
void helperRecheck(const HelperInput &In) {
  for(const auto &P:In.Pins) {
    auto Bytes=helperRead(P.Path,P.Bytes);
    require(Bytes.size()==P.Bytes && hex(SHA256::hash(Bytes))==P.Sha,"helper input changed across native work");
  }
}
