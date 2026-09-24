// Diagnostic sidecar only: closed ASCII grammar, never source or canonical admission.
struct GlobalCopyStep {
  uint32_t Block=0, Native=0;
  std::array<uint8_t,8> Descriptor{};
};
struct GlobalCopyInput {
  std::string Entry, CanonicalDigest, LlvmDigest;
  uint64_t CanonicalBytes=0,LlvmBytes=0;
  std::vector<size_t> Starts,Counts;
  std::vector<uint8_t> Encodings;
  std::vector<std::vector<uint32_t>> Edges;
  std::vector<GlobalCopyStep> Steps;
};
bool globalCopyUnsigned(StringRef Text,uint64_t Maximum,uint64_t &Value) {
  if(Text.empty() || Text.size()>10 || (Text.size()>1 && Text.front()=='0') ||
      llvm::any_of(Text,[](char C){return C<'0'||C>'9';}) ||
      Text.getAsInteger(10,Value) || Value>Maximum) return false;
  return true;
}
bool globalCopyDigest(StringRef Text) {
  return Text.size()==64 && llvm::all_of(Text,[](char C){return (C>='0'&&C<='9')||(C>='a'&&C<='f');});
}
bool globalCopySymbol(StringRef Text) {
  auto Letter=[](char C){return (C>='a'&&C<='z')||(C>='A'&&C<='Z')||C=='_';};
  return !Text.empty() && Text.size()<=128 && Letter(Text.front()) &&
      llvm::all_of(Text,[&](char C){return Letter(C)||(C>='0'&&C<='9');});
}
std::optional<GlobalCopyInput> parseGlobalCopyInput(ArrayRef<uint8_t> Bytes) {
  if(Bytes.empty() || Bytes.size()>16*1024 || Bytes.back()!='\n' ||
      llvm::any_of(Bytes,[](uint8_t C){return C!='\n'&&(C<32||C>126);}) ||
      std::count(Bytes.begin(),Bytes.end(),'\n')>48) return std::nullopt;
  StringRef Text(reinterpret_cast<const char*>(Bytes.data()),Bytes.size());
  SmallVector<StringRef,48> Lines;Text.drop_back().split(Lines,'\n',-1,true);
  size_t At=0;
  auto Row=[&](StringRef Key,size_t Count)->std::optional<SmallVector<StringRef,12>> {
    if(At>=Lines.size() || Lines[At].empty()) return std::nullopt;
    SmallVector<StringRef,12> Parts;Lines[At++].split(Parts,' ',-1,true);
    if(Parts.size()!=Count || Parts[0]!=Key ||
        llvm::any_of(Parts,[](StringRef P){return P.empty();})) return std::nullopt;
    return Parts;
  };
  if(!Row("FE2O3_PHYSICAL_GLOBAL_COPY_V21_NATIVE_OBSERVATION_INPUT_V1",1)) return std::nullopt;
  GlobalCopyInput Result;
  auto Canonical=Row("canonical",3),Llvm=Row("llvm",3),Entry=Row("entry",2),Launch=Row("launch",7);
  if(!Canonical||!Llvm||!Entry||!Launch || !globalCopyDigest((*Canonical)[1]) ||
      !globalCopyDigest((*Llvm)[1]) || !globalCopySymbol((*Entry)[1]) ||
      !globalCopyUnsigned((*Canonical)[2],4*1024*1024,Result.CanonicalBytes) || !Result.CanonicalBytes ||
      !globalCopyUnsigned((*Llvm)[2],32*1024,Result.LlvmBytes) || !Result.LlvmBytes) return std::nullopt;
  Result.CanonicalDigest=(*Canonical)[1].str();Result.LlvmDigest=(*Llvm)[1].str();Result.Entry=(*Entry)[1].str();
  const std::array<StringRef,6> ExpectedLaunch{{"64","1","1","2","1","1"}};
  if(!std::equal(ExpectedLaunch.begin(),ExpectedLaunch.end(),Launch->begin()+1)) return std::nullopt;
  auto BlockCount=Row("blocks",2);uint64_t N=0;
  if(!BlockCount || !globalCopyUnsigned((*BlockCount)[1],1,N) || N!=1) return std::nullopt;
  size_t Next=0;
  for(uint64_t B=0;B<N;++B) {
    auto Fields=Row("block",7);if(!Fields) return std::nullopt;
    std::array<uint64_t,6> V{};
    for(size_t I=0;I<6;++I) if(!globalCopyUnsigned((*Fields)[I+1],255,V[I])) return std::nullopt;
    if(V[0]!=B || V[1]!=4 || V[2]!=Next || !V[3] || V[3]>32-Next)
      return std::nullopt;
    std::vector<uint32_t> Edges;
    if(V[4]!=255) Edges.push_back(static_cast<uint32_t>(V[4]));
    if(V[5]!=255) Edges.push_back(static_cast<uint32_t>(V[5]));
    if(!Edges.empty()) return std::nullopt;
    Result.Starts.push_back(Next);Result.Counts.push_back(static_cast<size_t>(V[3]));
    Result.Encodings.push_back(static_cast<uint8_t>(V[1]));Result.Edges.push_back(std::move(Edges));
    Next+=static_cast<size_t>(V[3]);
  }
  auto StepCount=Row("steps",2);uint64_t Count=0;
  if(!StepCount || !globalCopyUnsigned((*StepCount)[1],32,Count) ||
      Count!=Next-1) return std::nullopt;
  size_t Prior=0;bool First=true;
  for(uint64_t I=0;I<Count;++I) {
    auto Fields=Row("step",11);if(!Fields) return std::nullopt;
    uint64_t Block=0,Native=0;
    if(!globalCopyUnsigned((*Fields)[1],N-1,Block) || !globalCopyUnsigned((*Fields)[2],31,Native) ||
        (!First && Native<=Prior) || Native<Result.Starts[Block] ||
        Native>=Result.Starts[Block]+Result.Counts[Block] ||
        Native+1==Result.Starts[Block]+Result.Counts[Block])
      return std::nullopt;
    GlobalCopyStep Step;Step.Block=static_cast<uint32_t>(Block);Step.Native=static_cast<uint32_t>(Native);
    for(size_t D=0;D<8;++D) {
      uint64_t Value=0;if(!globalCopyUnsigned((*Fields)[D+3],255,Value)) return std::nullopt;
      Step.Descriptor[D]=static_cast<uint8_t>(Value);
    }
    Result.Steps.push_back(Step);Prior=static_cast<size_t>(Native);First=false;
  }
  if(!Row("end",1) || At!=Lines.size()) return std::nullopt;
  return Result;
}
