// The finite caller semantic walk follows shape/encoding/CFG admission. Facts
// label source ABI roles, not numeric addresses or runtime memory permissions.
unsigned TransportAttempt::callerWord(TransportBank B,unsigned Index) const {
  for(size_t N=0;N<WordCount;++N)
    if(W.Words[N].Bank==B && W.Words[N].Index==Index)return W.Words[N].Id;
  return 0;
}
bool TransportAttempt::callerOverlap(unsigned A,unsigned B) const {
  const auto *L=find(A),*R=find(B);
  if(!L || !R)return false;
  size_t I=0,J=0;
  while(I<L->UnitCount && J<R->UnitCount) {
    if(L->Units[I]==R->Units[J])return true;
    if(L->Units[I]<R->Units[J])++I;else ++J;
  }
  return false;
}
bool TransportAttempt::callerWrite(unsigned Id,TransportState &S,bool PreserveLane) {
  if(!find(Id))return fail(ct::Refusal::Register);
  if(callerOverlap(Id,W.Caller.LaneRegister) && !PreserveLane)
    W.Caller.Lanes.fill({});
  return kill(Id,S);
}
bool TransportAttempt::callerStep(const DecodedInstruction &I,
    const TransportMcView &Mc,size_t At) {
  const auto *Shape=transportCallerShape(I.Inst.getOpcode());
  if(!Shape || At>=TransportCallerPrefix.size())return fail(ct::Refusal::Shape);
  const auto Op=Shape->Operation;
  const auto &D=Mc.Instructions.get(I.Inst.getOpcode());
  auto &S=W.States[0];auto &C=W.Caller;
  const auto reg=[&](unsigned O){return I.Inst.getOperand(O).getReg();};
  // This also visits the actual tied old destination of WRITELANE.
  for(unsigned O=D.getNumDefs();O<I.Inst.getNumOperands();++O) {
    const auto &A=I.Inst.getOperand(O);
    if(!A.isReg())continue;
    const auto *R=find(A.getReg());
    if(!R)return fail(ct::Refusal::Register);
    for(unsigned N=0;N<R->WordCount;++N)
      if(fact(R->Words[N],S).Ready==TransportReady::Pending)
        return fail(ct::Refusal::Pending);
  }
  TransportFact Source{},Lo{},Hi{};
  if(Op==CallerOp::Move32 || Op==CallerOp::Move || Op==CallerOp::WriteLane)
    if(I.Inst.getOperand(1).isReg())Source=fact(reg(1),S);
  if(Op==CallerOp::Move64 && I.Inst.getOperand(1).isReg()) {
    const auto *R=find(reg(1));
    if(!R || R->WordCount!=2)return fail(ct::Refusal::Register);
    Lo=fact(R->Words[0],S);Hi=fact(R->Words[1],S);
  }
  if(Op==CallerOp::ReadLane) {
    const auto Lane=I.Inst.getOperand(2).getImm();
    if(reg(1)!=C.LaneRegister || Lane<1 || Lane>2)
      return fail(ct::Refusal::Register);
    Source=C.Lanes[size_t(Lane)];
    if(Source.Origin!=9+Lane || Source.Generation!=1 ||
       !transportAvailable(Source,TransportDomain::Uniform))
      return fail(ct::Refusal::Undefined);
  }
  if(Op==CallerOp::Load1 || Op==CallerOp::Load2) {
    const auto *Base=find(reg(1));
    if(!Base || Base->WordCount!=2)return fail(ct::Refusal::Register);
    Lo=fact(Base->Words[0],S);Hi=fact(Base->Words[1],S);
    if(S.LoadReady || Lo.Origin!=10 || Hi.Origin!=11 ||
       Lo.Generation!=1 || Hi.Generation!=1 ||
       !transportAvailable(Lo,TransportDomain::Uniform) ||
       !transportAvailable(Hi,TransportDomain::Uniform))
      return fail(ct::Refusal::Undefined);
  }
  const bool Restore=Op==CallerOp::Move64 && reg(0)==2;
  if(Op==CallerOp::SaveExec &&
     (C.ExecSaved || C.ExpandedExec || S.Current!=TransportDomain::Entry))
    return fail(ct::Refusal::Domain);
  if(Restore && (!C.ExecSaved || !C.ExpandedExec ||
      Lo.Origin!=30 || Hi.Origin!=31 || Lo.Generation!=3 || Hi.Generation!=3 ||
      !transportAvailable(Lo,TransportDomain::Uniform) ||
      !transportAvailable(Hi,TransportDomain::Uniform)))
    return fail(ct::Refusal::Domain);
  const auto write=[&](unsigned Id,bool PreserveLane) {
    if(callerOverlap(Id,2) && Op!=CallerOp::SaveExec && !Restore)
      return fail(ct::Refusal::Domain);
    return callerWrite(Id,S,PreserveLane);
  };
  if(Op==CallerOp::WriteLane &&
     (reg(0)!=C.LaneRegister || reg(3)!=reg(0) ||
      D.getOperandConstraint(3,MCOI::TIED_TO)!=0))
    return fail(ct::Refusal::Alias);
  for(unsigned O=0;O<D.getNumDefs();++O)
    if(!write(reg(O),Op==CallerOp::WriteLane && O==0))return false;
  for(unsigned Id:D.implicit_defs())if(!write(Id,false))return false;
  switch(Op) {
  case CallerOp::Move32:
    return set(reg(0),Source,S);
  case CallerOp::Move64: {
    if(Restore){C.ExpandedExec=false;return true;}
    const auto *Dest=find(reg(0));
    return Dest && Dest->WordCount==2 &&
      set(Dest->Words[0],Lo,S) && set(Dest->Words[1],Hi,S);
  }
  case CallerOp::WriteLane: {
    const auto Lane=I.Inst.getOperand(2).getImm();
    if(Lane<0 || Lane>=int64_t(C.Lanes.size()))return fail(ct::Refusal::Register);
    if(Lane==1 || Lane==2) {
      if(Source.Origin!=9+Lane || Source.Generation!=1 ||
         !transportAvailable(Source,TransportDomain::Uniform))
        return fail(ct::Refusal::Undefined);
    }
    // The actual instruction ignores EXEC. Its tied old destination preserves
    // all other selected lanes. Unknown unused lanes stay unknown.
    C.Lanes[size_t(Lane)]=Source;return true;
  }
  case CallerOp::ReadLane:
    return set(reg(0),Source,S);
  case CallerOp::Load1: case CallerOp::Load2: {
    const auto *Dest=find(reg(0));
    const unsigned Count=Op==CallerOp::Load1?1:2;
    if(!Dest || Dest->WordCount!=Count || C.PendingWords>7-Count)
      return fail(ct::Refusal::Pending);
    const auto Offset=I.Inst.getOperand(2).getImm();
    for(unsigned N=0;N<Count;++N) {
      const auto Byte=Offset+4*N;
      const uint16_t Expr=Byte>=16 && Byte<=24?uint16_t((Byte-16)/4+1):0;
      if(!set(Dest->Words[N],{Expr,2,TransportDomain::Uniform,
          TransportReady::Pending,2,0},S))return false;
    }
    C.PendingWords+=uint8_t(Count);S.LoadIssued=true;return true;
  }
  case CallerOp::Wait: {
    if(!S.LoadIssued || S.LoadReady || C.PendingWords!=7)
      return fail(ct::Refusal::Pending);
    unsigned Count=0;
    for(size_t N=0;N<WordCount;++N) {
      auto &F=S.Facts[N];
      if(F.Ready!=TransportReady::Pending)continue;
      if(F.Origin!=2 || F.Generation!=2 || F.Domain!=TransportDomain::Uniform)
        return fail(ct::Refusal::Pending);
      F.Ready=TransportReady::Ready;++Count;
    }
    if(Count!=7)return fail(ct::Refusal::Pending);
    S.LoadReady=true;return true;
  }
  case CallerOp::SaveExec: {
    const auto *Dest=find(reg(0));
    if(!Dest || Dest->WordCount!=2)return fail(ct::Refusal::Register);
    C.ExecSaved=true;C.ExpandedExec=true;
    return set(Dest->Words[0],{0,3,TransportDomain::Uniform,TransportReady::Ready,30,0},S) &&
           set(Dest->Words[1],{0,3,TransportDomain::Uniform,TransportReady::Ready,31,0},S);
  }
  case CallerOp::Move: {
    const auto *Dest=find(reg(0));
    if(!Dest)return fail(ct::Refusal::Register);
    if(Dest->Index<=2) {
      if(C.ExpandedExec || !S.LoadReady || Source.Expression!=Dest->Index+1 ||
         Source.Generation!=2 || Source.Origin!=2 ||
         !transportAvailable(Source,TransportDomain::Uniform))
        return fail(ct::Refusal::Undefined);
      Source.Domain=TransportDomain::Entry;
      return set(reg(0),Source,S);
    }
    // Unrelated prologue DATA remains opaque; no equality is manufactured.
    return true;
  }
  case CallerOp::Call:
    if(C.ExpandedExec || !C.ExecSaved || !S.LoadReady)
      return fail(ct::Refusal::Domain);
    for(unsigned N=0;N<3;++N) {
      const auto F=fact(callerWord(TransportBank::Vector,N),S);
      if(F.Expression!=N+1 || F.Generation!=2 || F.Origin!=2 ||
         F.Domain!=TransportDomain::Entry || F.Ready!=TransportReady::Ready)
        return fail(ct::Refusal::Undefined);
      C.Arguments[N]=F;
    }
    C.ReturnPair=uint16_t(reg(0));return true;
  case CallerOp::AccWrite:
  case CallerOp::GetPc: case CallerOp::Add: case CallerOp::AddCarry:
  case CallerOp::Nop:
    // Actual effects/aliases were checked and killed above. These opaque
    // values do not acquire address, spill-result or control semantics.
    return true;
  }
  return fail(ct::Refusal::Shape);
}
