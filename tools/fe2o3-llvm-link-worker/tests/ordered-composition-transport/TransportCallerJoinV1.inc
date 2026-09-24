// A distinct finite O0 root prefix, not a general caller interpreter.
bool TransportAttempt::caller(ArrayRef<DecodedInstruction> Code,
    const FunctionCfg &Cfg,const TransportMcView &Mc,const SymbolRecord &F,
    const AnalyzedFunction &Analyzed) {
  if(Profile!=ct::Profile::CallerToHelperO0 || !Bound || W.Caller.PrefixReady ||
     W.Caller.Consumed || Code.size()!=144 || F.Size!=840 ||
     Cfg.Blocks.size()!=7 || Cfg.InstructionBlocks.size()!=Code.size() ||
     Analyzed.Instructions.size()!=Code.size() ||
     Analyzed.DirectCallSites!=1 || Analyzed.Evidence.DirectCallees.size()!=1 ||
     Analyzed.Evidence.DirectCallees[0]!=helper())
    return fail(ct::Refusal::Roster);
  const auto &Entry=Cfg.Blocks[0];
  if(Entry.Begin!=0 || Entry.End!=TransportCallerPrefix.size() ||
     !Entry.Predecessors.empty() || Entry.Successors.size()!=1 ||
     Entry.Successors[0]!=1)return fail(ct::Refusal::Cfg);
  // No backedge/re-entry into the interpreted prefix is accepted. Remaining
  // actual CFG/control/effects are still checked by the original analyzer;
  // they are not replaced with a projected graph.
  for(const auto &B:Cfg.Blocks)
    for(size_t Next:B.Successors)
      if(Next==0 || Next>=Cfg.Blocks.size())return fail(ct::Refusal::Cfg);
  if(F.FileOffset>UINT64_MAX-F.Size || F.Address>UINT64_MAX-F.Size)
    return fail(ct::Refusal::Roster);
  uint64_t Cursor=F.Address;unsigned Calls=0;
  const PhysicalMachineInstructionTrace *Call=nullptr;
  for(size_t N=0;N<Code.size();++N) {
    const auto &I=Code[N];const auto &T=Analyzed.Instructions[N];
    if(I.Address!=Cursor || I.Size==0 || I.Size>16 ||
       I.Size>F.Address+F.Size-Cursor ||
       T.FunctionSymbol!=F.Name || T.InstructionOffset!=F.FileOffset+(I.Address-F.Address) ||
       T.BlockOrdinal!=Cfg.InstructionBlocks[N] || T.Encoding!=I.Encoding ||
       I.Inst.getOpcode()>=Mc.Instructions.getNumOpcodes())
      return fail(ct::Refusal::Aggregate);
    Cursor+=I.Size;
    const bool IsCall=Mc.Instructions.get(I.Inst.getOpcode()).isCall();
    if(IsCall!=(T.BranchKind==PhysicalMachineBranchKind::DirectCall))
      return fail(ct::Refusal::Roster);
    if(IsCall) {
      if(++Calls!=1 || N!=44 || T.BlockOrdinal!=0)
        return fail(ct::Refusal::Roster);
      Call=&T;
    }
  }
  if(Cursor!=F.Address+F.Size || Calls!=1 || !Call)
    return fail(ct::Refusal::Roster);
  if(!tables(Mc.Registers))return false;
  for(size_t N=0;N<TransportCallerPrefix.size();++N) {
    const auto &I=Code[N];const auto &E=TransportCallerPrefix[N];
    if(Cfg.InstructionBlocks[N]!=0 || !callerShape(I,Mc) ||
       !transportCallerEncoding(I,E))return fail(ct::Refusal::Encoding);
    for(unsigned O=0;O<I.Inst.getNumOperands();++O) {
      const auto &A=I.Inst.getOperand(O);const auto &Want=E.Operands[O];
      if(Want.Register) {
        if(!A.isReg() || !role(A.getReg(),Want.Bank,Want.Index,Want.Width))
          return fail(ct::Refusal::Register);
      } else if(!A.isImm() || A.getImm()!=Want.Immediate)
        return fail(ct::Refusal::Shape);
    }
  }
  auto &S=W.States[0];S={};
  auto &C=W.Caller;C.LaneRegister=uint16_t(callerWord(TransportBank::Vector,4));
  if(!C.LaneRegister)return fail(ct::Refusal::Register);
  // Exact descriptor enable ordering was checked at bind: dispatch, queue,
  // kernarg, dispatch-id pairs place kernarg low/high at actual s4/s5.
  // These are symbolic ABI origins, never invented address bits.
  for(unsigned N=0;N<2;++N)
    if(!set(callerWord(TransportBank::Scalar,4+N),
       {0,1,TransportDomain::Uniform,TransportReady::Ready,uint8_t(10+N),0},S))
      return false;
  for(size_t N=0;N<TransportCallerPrefix.size();++N)
    if(!callerStep(Code[N],Mc,N))return false;
  if(C.ExpandedExec || !C.ExecSaved || !C.ReturnPair ||
     !role(C.ReturnPair,TransportBank::Scalar,30,64))
    return fail(ct::Refusal::Domain);
  C.RootOffset=F.FileOffset;C.RootBytes=F.Size;
  C.CallOffset=Call->InstructionOffset;C.TargetOffset=Call->BranchTarget;
  C.PrefixReady=true;
  Summary.CallerOffset=C.RootOffset;Summary.CallOffset=C.CallOffset;
  Summary.CallTargetOffset=C.TargetOffset;
  return true;
}
