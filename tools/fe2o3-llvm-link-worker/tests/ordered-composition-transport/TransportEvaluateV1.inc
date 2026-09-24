bool transportAvailable(TransportFact F,TransportDomain Use) {
  if(F.Ready!=TransportReady::Ready)return false;
  return F.Domain==TransportDomain::Uniform || F.Domain==Use ||
    (F.Domain==TransportDomain::Entry && Use==TransportDomain::Intersection);
}
bool TransportAttempt::evaluate(ArrayRef<DecodedInstruction> Code,
    const FunctionCfg &Cfg,const TransportMcView &Mc,const SymbolRecord &Function) {
  const bool Helper=Profile==ct::Profile::ConditionalHelperO0;
  const bool Edit=Program==ct::Program::MoveInput2;
  const size_t Count=(Helper?10U:19U)-(Edit?1U:0U);
  if(Code.size()!=Count || Cfg.InstructionBlocks.size()!=Count)
    return fail(ct::Refusal::Cfg);
  if(Helper) {
    if(Cfg.Blocks.size()!=1 || Cfg.Blocks[0].Begin!=0 ||
       Cfg.Blocks[0].End!=Count || !Cfg.Blocks[0].Successors.empty() ||
       !Cfg.Blocks[0].Predecessors.empty())return fail(ct::Refusal::Cfg);
  } else {
    const size_t Split=Edit?13:14;
    if(Cfg.Blocks.size()!=3 || Cfg.Blocks[0].Begin!=0 ||
       Cfg.Blocks[0].End!=Split || Cfg.Blocks[1].Begin!=Split ||
       Cfg.Blocks[1].End!=Split+4 || Cfg.Blocks[2].Begin!=Split+4 ||
       Cfg.Blocks[2].End!=Count || Cfg.Blocks[0].Successors.size()!=2 ||
       !llvm::is_contained(Cfg.Blocks[0].Successors,size_t(1)) ||
       !llvm::is_contained(Cfg.Blocks[0].Successors,size_t(2)) ||
       (Cfg.Blocks[1].Successors.size()!=1 || Cfg.Blocks[1].Successors[0]!=2) ||
       !Cfg.Blocks[2].Successors.empty() ||
       !Cfg.Blocks[0].Predecessors.empty() ||
       (Cfg.Blocks[1].Predecessors.size()!=1 || Cfg.Blocks[1].Predecessors[0]!=0) ||
       Cfg.Blocks[2].Predecessors.size()!=2 ||
       !llvm::is_contained(Cfg.Blocks[2].Predecessors,size_t(0)) ||
       !llvm::is_contained(Cfg.Blocks[2].Predecessors,size_t(1)))
      return fail(ct::Refusal::Cfg);
    uint64_t Target=0;
    const auto &Branch=Code[Split-1];
    if(!Mc.Analysis.evaluateBranch(Branch.Inst,Branch.Address,Branch.Size,Target) ||
       Target!=Code[Split+4].Address)return fail(ct::Refusal::Cfg);
  }
  if(!tables(Mc.Registers))return false;
  // Exact selected extent and physical site identity, not a detached trace.
  uint64_t Cursor=Function.Address;
  if(Function.FileOffset>UINT64_MAX-Function.Size)return fail(ct::Refusal::Roster);
  for(const auto &I:Code) {
    if(I.Address!=Cursor || I.Size>UINT64_MAX-Cursor)
      return fail(ct::Refusal::Encoding);
    Cursor+=I.Size;
  }
  if(Cursor-Function.Address!=Function.Size)return fail(ct::Refusal::Encoding);
  // Admit all actual register closure before retaining any semantic facts.
  for(size_t I=0;I<Code.size();++I) {
    if(!shape(Code[I],Mc))return false;
    const auto E=transportExpected(Profile,Program,I);
    if(!transportEncoding(Code[I],E.Literal))return fail(ct::Refusal::Encoding);
    for(size_t O=0;O<Code[I].Inst.getNumOperands();++O) {
      const auto &Actual=Code[I].Inst.getOperand(O);const auto Want=E.Operands[O];
      if(Want.Register) {
        if(!Actual.isReg() || !role(Actual.getReg(),Want.Bank,Want.Index,Want.Width))
          return fail(ct::Refusal::Register);
      } else if(!Actual.isImm() || Actual.getImm()!=Want.Immediate)
        return fail(ct::Refusal::Shape);
    }
  }
  auto &State=W.States[0];State={};
  const auto realWord=[&](TransportBank Bank,unsigned Index)->unsigned {
    for(size_t I=0;I<WordCount;++I)
      if(W.Words[I].Bank==Bank && W.Words[I].Index==Index)return W.Words[I].Id;
    return 0;
  };
  if(Helper) {
    for(unsigned I=0;I<3;++I)
      if(!set(realWord(TransportBank::Vector,I),
          {uint16_t(I+1),1,TransportDomain::Entry,TransportReady::Ready,1,0},State))
        return false;
  } else {
    for(unsigned I=0;I<2;++I)
      if(!set(realWord(TransportBank::Scalar,I),
          {0,1,TransportDomain::Uniform,TransportReady::Ready,uint8_t(10+I),0},State))
        return false;
  }
  const auto overlap=[&](unsigned A,unsigned B) {
    const auto *L=find(A),*R=find(B);if(!L || !R)return false;
    size_t I=0,J=0;
    while(I<L->UnitCount && J<R->UnitCount) {
      if(L->Units[I]==R->Units[J])return true;
      if(L->Units[I]<R->Units[J])++I;else ++J;
    }
    return false;
  };
  unsigned Stores=0,Returns=0;
  for(size_t Block=0;Block<Cfg.Blocks.size();++Block) {
    if(Block==1)State=W.States[1];
    if(Block==2) {
      State=W.States[1];
      const auto &Other=W.States[2];
      if(State.Current!=Other.Current || State.LoadReady!=Other.LoadReady ||
         State.Masked!=Other.Masked)return fail(ct::Refusal::Domain);
      for(size_t I=0;I<64;++I)
        if(State.Facts[I]!=Other.Facts[I])State.Facts[I]={};
    }
    for(size_t At=Cfg.Blocks[Block].Begin;At<Cfg.Blocks[Block].End;++At) {
      const auto &I=Code[At];const auto Op=TransportOp(I.Inst.getOpcode());
      const auto &D=Mc.Instructions.get(I.Inst.getOpcode());
      const auto reg=[&](unsigned O){return I.Inst.getOperand(O).getReg();};
      if(Cfg.InstructionBlocks[At]!=Block)return fail(ct::Refusal::Cfg);
      // No pending word may be consumed, even by an otherwise opaque address op.
      for(unsigned O=D.getNumDefs();O<I.Inst.getNumOperands();++O) {
        const auto &Operand=I.Inst.getOperand(O);if(!Operand.isReg())continue;
        const auto *R=find(Operand.getReg());
        for(size_t N=0;N<R->WordCount;++N)
          if(fact(R->Words[N],State).Ready==TransportReady::Pending)
            return fail(ct::Refusal::Pending);
      }
      TransportFact Source{},Right{};
      if(Op==TransportOp::Move || Op==TransportOp::AccRead ||
         Op==TransportOp::AccWrite)Source=fact(reg(1),State);
      if(Op==TransportOp::Xor || Op==TransportOp::And) {
        Source=fact(reg(1),State);Right=fact(reg(2),State);
      }
      if(Op==TransportOp::Load8) {
        const auto *Base=find(reg(1));
        const auto Lo=fact(Base->Words[0],State),Hi=fact(Base->Words[1],State);
        if(State.LoadIssued || Lo.Origin!=10 || Hi.Origin!=11 ||
           Lo.Generation!=1 || Hi.Generation!=Lo.Generation ||
           !transportAvailable(Lo,TransportDomain::Uniform) ||
           !transportAvailable(Hi,TransportDomain::Uniform))
          return fail(ct::Refusal::Undefined);
      }
      if(Op==TransportOp::SaveExec &&
         (!State.Compared || State.VccGeneration==0 || State.Masked ||
          State.Current!=TransportDomain::Entry))
        return fail(ct::Refusal::Domain);
      const auto write=[&](unsigned Id) {
        if(overlap(Id,2) && Op!=TransportOp::SaveExec)
          return fail(ct::Refusal::Domain);
        if(overlap(Id,45)) {State.Compared=false;State.VccGeneration=0;}
        return kill(Id,State);
      };
      for(unsigned O=0;O<D.getNumDefs();++O)if(!write(reg(O)))return false;
      for(unsigned Id:D.implicit_defs())if(!write(Id))return false;
      switch(Op) {
      case TransportOp::Load8: {
        const auto *Dest=find(reg(0));
        if(Dest->WordCount!=8)return fail(ct::Refusal::Register);
        State.LoadIssued=true;
        for(unsigned N=0;N<8;++N) {
          const uint16_t Expr=N>=4 && N<=6?uint16_t(N-3):0;
          if(!set(Dest->Words[N],
              {Expr,2,TransportDomain::Uniform,TransportReady::Pending,2,0},State))
            return false;
        }
        break;
      }
      case TransportOp::Wait:
        if(Helper) {if(State.LoadIssued)return fail(ct::Refusal::Pending);break;}
        if(!State.LoadIssued || State.LoadReady)return fail(ct::Refusal::Pending);
        for(unsigned N=4;N<12;++N) {
          const unsigned Id=realWord(TransportBank::Scalar,N);
          auto F=fact(Id,State);
          if(F.Generation!=2 || F.Origin!=2 || F.Ready!=TransportReady::Pending)
            return fail(ct::Refusal::Pending);
          F.Ready=TransportReady::Ready;
          if(!set(Id,F,State))return false;
        }
        State.LoadReady=true;break;
      case TransportOp::Move:
      case TransportOp::AccRead:
      case TransportOp::AccWrite: {
        // O3 address-only moves keep opaque values; all program/return moves
        // require the exact source expression, separately constrained by roster.
        const auto *Dest=find(reg(0));
        const bool Semantic=Helper || Dest->Index>=8;
        if(Semantic && (!transportAvailable(Source,State.Current) ||
                         Source.Expression==0))return fail(ct::Refusal::Undefined);
        if(!Semantic && Source.Ready==TransportReady::Unknown)
          Source={0,0,State.Current,TransportReady::Ready,0,0};
        Source.Domain=State.Current;
        if(!set(reg(0),Source,State))return false;
        break;
      }
      case TransportOp::Xor:
      case TransportOp::And: {
        const uint16_t L=Op==TransportOp::Xor?1:4,R=Op==TransportOp::Xor?2:3;
        if(!transportAvailable(Source,State.Current) ||
           !transportAvailable(Right,State.Current) ||
           Source.Expression!=L || Right.Expression!=R)
          return fail(ct::Refusal::Undefined);
        if(!set(reg(0),{uint16_t(Op==TransportOp::Xor?4:5),0,State.Current,
                       TransportReady::Ready,0,0},State))return false;
        break;
      }
      case TransportOp::Compare:
        if(!State.LoadReady || State.Compared || State.Current!=TransportDomain::Entry)
          return fail(ct::Refusal::Domain);
        State.Compared=true;State.VccGeneration=3;break;
      case TransportOp::SaveExec:
        State.Masked=true;State.Current=TransportDomain::Intersection;break;
      case TransportOp::Branch:
        if(!State.Masked || State.Current!=TransportDomain::Intersection)
          return fail(ct::Refusal::Domain);
        break;
      case TransportOp::Store: {
        const auto Data=fact(reg(1),State);
        if(++Stores!=1 || !State.Masked || !State.LoadReady ||
           State.Current!=TransportDomain::Intersection ||
           !transportAvailable(Data,State.Current) ||
           Data.Expression!=(Edit?3:5))return fail(ct::Refusal::Undefined);
        Summary.DataUseOffset=Function.FileOffset+(I.Address-Function.Address);
        break;
      }
      case TransportOp::Return: {
        const auto Data=fact(realWord(TransportBank::Vector,0),State);
        if(++Returns!=1 || State.Current!=TransportDomain::Entry ||
           !transportAvailable(Data,State.Current) || Data.Expression!=(Edit?3:5))
          return fail(ct::Refusal::Undefined);
        Summary.DataUseOffset=Function.FileOffset+(I.Address-Function.Address);
        break;
      }
      case TransportOp::ScalarMove: case TransportOp::ScalarShift:
      case TransportOp::Or: case TransportOp::Address:
        // Writes already killed overlapping facts. Address results are opaque.
        break;
      case TransportOp::Nop: case TransportOp::End:break;
      }
      if(At==(Helper?4U:10U))
        Summary.ProgramOffset=Function.FileOffset+(I.Address-Function.Address);
    }
    W.States[Block+1]=State;
  }
  return (Helper?(Returns==1 && Stores==0):(Stores==1 && Returns==0)) ||
         fail(ct::Refusal::Roster);
}
