bool TransportAttempt::bind(const MetadataKernel &K,const SymbolRecord &Entry,
                            ArrayRef<uint8_t> D) {
  if(!good() || Bound || Finished || Observing)return fail(ct::Refusal::Reentry);
  if(K.Name!=root() || Entry.Name!=root() || D.size()!=64 ||
     !K.TransportShape || K.KernargSize!=288 || K.GroupSize!=0)
    return fail(ct::Refusal::Descriptor);
  if(Profile==ct::Profile::InlineRootO3) {
    // Complete fixed observed descriptor resource shape. Kernarg is the sole
    // enabled user-data item, hence actual s0/s1; rsrc2's USER_SGPR is two.
    if(K.PrivateSize!=0 || support::endian::read32le(D.data())!=0 ||
       support::endian::read32le(D.data()+4)!=0 ||
       support::endian::read32le(D.data()+8)!=288 ||
       support::endian::read32le(D.data()+44)!=3 ||
       support::endian::read32le(D.data()+48)!=11468929 ||
       support::endian::read32le(D.data()+52)!=132 ||
       support::endian::read16le(D.data()+56)!=8)
      return fail(ct::Refusal::Descriptor);
  }
  if(Profile==ct::Profile::CallerToHelperO0) {
    // Finite observed O0 descriptor. In particular USER_SGPR=8 and enable
    // bits 1,2,3,4 select dispatch/queue/kernarg/dispatch-id in ABI order;
    // wave32 and private-segment-buffer are disabled. Dynamic-stack metadata
    // remains an observed property, not a claim of no stack/spill execution.
    if(K.PrivateSize!=0 || support::endian::read32le(D.data())!=0 ||
       support::endian::read32le(D.data()+4)!=0 ||
       support::endian::read32le(D.data()+8)!=288 ||
       support::endian::read32le(D.data()+44)!=7 ||
       support::endian::read32le(D.data()+48)!=11469061 ||
       support::endian::read32le(D.data()+52)!=913 ||
       support::endian::read16le(D.data()+56)!=2078)
      return fail(ct::Refusal::Descriptor);
  }
  Summary.NativeDescriptor=SHA256::hash(D);
  Bound=true;return true;
}
bool TransportAttempt::observe(const SymbolRecord &F,
    ArrayRef<DecodedInstruction> Code,const FunctionCfg &Cfg,const TransportMcView &Mc,
    const AnalyzedFunction &Analyzed) {
  if(!good() || !Bound || Finished || Observing)return fail(ct::Refusal::Reentry);
  Observing=true;
  // Always clear the private in-progress cursor, including a refusal.
  struct Exit {bool &Flag;~Exit(){Flag=false;}} ExitGuard{Observing};
  if(FunctionCount==2 || Code.size()>size_t(256)-InstructionCount ||
     Cfg.Blocks.size()>size_t(8)-BlockCount ||
     Analyzed.Evidence.Symbol!=F.Name ||
     Analyzed.Evidence.CodeOffset!=F.FileOffset ||
     Analyzed.Evidence.CodeSize!=F.Size)
    return fail(ct::Refusal::Roster);
  ++FunctionCount;InstructionCount+=uint16_t(Code.size());
  BlockCount+=uint16_t(Cfg.Blocks.size());
  for(const auto &B:Cfg.Blocks) {
    if(!B.Reachable || B.Successors.size()>size_t(16)-EdgeCount)
      return fail(ct::Refusal::Cfg);
    EdgeCount+=uint16_t(B.Successors.size());
  }
  if(F.Name==root()) {
    if(RootSeen)return fail(ct::Refusal::Roster);
    RootSeen=true;
    if(Profile!=ct::Profile::InlineRootO3) {
      if(Analyzed.DirectCallSites!=1 || Analyzed.Evidence.DirectCallees.size()!=1 ||
         Analyzed.Evidence.DirectCallees[0]!=helper())
        return fail(ct::Refusal::Roster);
      if(Profile==ct::Profile::CallerToHelperO0)return caller(Code,Cfg,Mc,F,Analyzed);
      // Deliberately no caller/address/spill interpretation. Its original full
      // analyzer checks and aggregate budgets still run before any commit.
      return true;
    }
    if(Analyzed.DirectCallSites!=0 || !Analyzed.Evidence.DirectCallees.empty())
      return fail(ct::Refusal::Roster);
  } else if(F.Name==helper() && Profile!=ct::Profile::InlineRootO3) {
    if(HelperSeen || Analyzed.DirectCallSites!=0 ||
       !Analyzed.Evidence.DirectCallees.empty())return fail(ct::Refusal::Roster);
    HelperSeen=true;
  } else return fail(ct::Refusal::Roster);
  if(SelectedSeen || !evaluate(Code,Cfg,Mc,F))return fail(ct::Refusal::Roster);
  SelectedSeen=true;Summary.SelectedOffset=F.FileOffset;Summary.SelectedBytes=F.Size;
  return true;
}
bool TransportAttempt::finish(const PhysicalMachineEffectEvidence &E) {
  if(!good() || Finished || Observing || !Bound || !SelectedSeen || !RootSeen)
    return fail(ct::Refusal::Aggregate);
  if(E.Entries.size()!=1 || E.Entries[0].Symbol!=root() ||
     E.Entries[0].DescriptorIdentity!=Summary.NativeDescriptor ||
     E.Functions.size()!=FunctionCount || E.Blocks.size()!=BlockCount ||
     E.Instructions.size()!=InstructionCount ||
     (Profile!=ct::Profile::InlineRootO3
        ?(!HelperSeen || FunctionCount!=2):(HelperSeen || FunctionCount!=1)))
    return fail(ct::Refusal::Aggregate);
  unsigned Selected=0;
  const StringRef Wanted=Profile!=ct::Profile::InlineRootO3?helper():root();
  if(Profile==ct::Profile::CallerToHelperO0 &&
     (!W.Caller.PrefixReady || !W.Caller.Consumed || W.Caller.ExpandedExec ||
      W.Caller.TargetOffset!=Summary.SelectedOffset))
    return fail(ct::Refusal::Aggregate);
  for(const auto &F:E.Functions) {
    if(F.Symbol!=root() && F.Symbol!=helper())return fail(ct::Refusal::Roster);
    if(F.Symbol==Wanted) {
      if(++Selected!=1 || F.CodeOffset!=Summary.SelectedOffset ||
         F.CodeSize!=Summary.SelectedBytes)return fail(ct::Refusal::Aggregate);
    }
  }
  if(Selected!=1)return fail(ct::Refusal::Aggregate);
  Summary.CachedRegisters=RegisterCount;Summary.WholeWords=WordCount;
  Summary.Whole32Classes=ClassCount;Summary.Functions=FunctionCount;
  Summary.Blocks=BlockCount;Summary.Instructions=InstructionCount;
  Finished=true;return true;
}
