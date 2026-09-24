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
  int FD=open(Path.str().c_str(),O_RDONLY|O_CLOEXEC|O_NOFOLLOW|O_NONBLOCK);
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
  require(helperDigest(Digest) && In.Pins.size()<32,"helper pin bounds");
  auto Bytes=helperRead(Path,Limit);
  require(hex(SHA256::hash(Bytes))==Digest && (!Expected || Bytes.size()==*Expected),
      "helper retained input identity mismatch");
  In.Pins.push_back({Path.str(),Digest.str(),Bytes.size()}); return Bytes;
}
const json::Object &helperObject(const json::Object &O,StringRef Key) {
  auto *V=O.getObject(Key); require(V!=nullptr,"helper record object"); return *V;
}
void helperRecheck(const HelperInput &In) {
  for(const auto &P:In.Pins) {
    auto Bytes=helperRead(P.Path,P.Bytes);
    require(Bytes.size()==P.Bytes && hex(SHA256::hash(Bytes))==P.Sha,"helper input changed across native work");
  }
}
