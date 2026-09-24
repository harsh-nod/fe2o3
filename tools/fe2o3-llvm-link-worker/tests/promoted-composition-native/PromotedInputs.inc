// Bounded diagnostic joins. File bytes never reconstruct a source owner.
struct PromotedSourceStamp {
  std::string Path;
  uint64_t Device,Inode,Bytes;
  int64_t Mtime,MtimeNs,Ctime,CtimeNs;
};
struct PromotedInput {
  CompositionRecord Record;
  std::vector<PromotedSourceStamp> Sources;
};
int64_t promotedSigned(const json::Object &O,StringRef Key) {
  const auto V=O.getInteger(Key);require(V.has_value(),"source stamp integer");return *V;
}
void promotedStampCheck(const PromotedSourceStamp &S) {
  struct stat Now{};
  require(lstat(S.Path.c_str(),&Now)==0 && S_ISREG(Now.st_mode) &&
    uint64_t(Now.st_dev)==S.Device && uint64_t(Now.st_ino)==S.Inode &&
    Now.st_size>0 && uint64_t(Now.st_size)==S.Bytes &&
    Now.st_mtim.tv_sec==S.Mtime && Now.st_mtim.tv_nsec==S.MtimeNs &&
    Now.st_ctim.tv_sec==S.Ctime && Now.st_ctim.tv_nsec==S.CtimeNs,
    "actual published source inode/stamp changed");
}
void promotedSource(PromotedInput &P,StringRef Path,const json::Object &O) {
  require(P.Sources.size()<2,"source stamp census");
  PromotedSourceStamp S{Path.str(),helperInteger(O,"device"),helperInteger(O,"inode"),
    helperInteger(O,"bytes"),promotedSigned(O,"mtime"),promotedSigned(O,"mtime_ns"),
    promotedSigned(O,"ctime"),promotedSigned(O,"ctime_ns")};
  require(S.Bytes>0 && S.Bytes<=72*1024 && S.MtimeNs>=0 && S.MtimeNs<1000000000 &&
    S.CtimeNs>=0 && S.CtimeNs<1000000000,"source stamp bounds");
  promotedStampCheck(S);
  (void)helperPinned(P.Record.Input,Path,helperString(O,"sha256"),72*1024,S.Bytes);
  promotedStampCheck(S);P.Sources.push_back(std::move(S));
}
void promotedRecheck(const PromotedInput &P) {
  for(const auto &S:P.Sources)promotedStampCheck(S);
  helperRecheck(P.Record.Input);
  for(const auto &S:P.Sources)promotedStampCheck(S);
}
void promotedEqual(const json::Object &A,const json::Object &B,StringRef Key) {
  require(A.get(Key) && B.get(Key) && *A.get(Key)==*B.get(Key),"exact promoted row join");
}
void promotedFalse(const json::Object &O,std::initializer_list<StringRef> Keys) {
  for(const auto K:Keys)helperBool(O,K,false);
}
const json::Object &promotedFind(const json::Array &Rows,StringRef Key,StringRef Value,
                               StringRef Mode={}) {
  const json::Object *Found=nullptr;
  for(const auto &V:Rows) {
    const auto &O=compositionRow(V);
    if(O.getString(Key)!=std::optional<StringRef>(Value) ||
       (!Mode.empty() && O.getString("mode")!=std::optional<StringRef>(Mode)))continue;
    require(!Found,"duplicate promoted selected row");Found=&O;
  }
  require(Found,"missing promoted selected row");return *Found;
}
std::vector<uint8_t> promotedPin(HelperInput &In,StringRef Path,const json::Object &O,
    StringRef Field,size_t Limit=4*1024*1024) {
  return helperPinned(In,Path,helperString(O,Field),Limit);
}
void promotedDescriptorText(ArrayRef<uint8_t> Canonical,ArrayRef<uint8_t> Worker,
                            ArrayRef<uint8_t> Descriptor) {
  require(!Descriptor.empty() && Descriptor.size()<=65536,"descriptor byte bound");
  std::string Expected(Canonical.begin(),Canonical.end());
  Expected+="\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
  for(size_t I=0;I<Descriptor.size();I+=16) {
    Expected+="module asm \".byte ";
    for(size_t J=I;J<std::min(I+16,Descriptor.size());++J) {
      if(J!=I)Expected+=", ";
      const uint8_t B=Descriptor[J];Expected+="0x";
      Expected+=hex(ArrayRef<uint8_t>(&B,1));
    }
    Expected+="\"\n";
  }
  require(Worker.size()==Expected.size() &&
    std::equal(Worker.begin(),Worker.end(),Expected.begin()),"exact existing descriptor extension");
}
