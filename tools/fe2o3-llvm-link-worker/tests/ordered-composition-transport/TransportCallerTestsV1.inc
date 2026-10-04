#include "TransportCallerFixtureV1.inc"
std::array<uint8_t,64> callerDescriptor() {
  std::array<uint8_t,64> D{};
  support::endian::write32le(D.data()+8,288);
  support::endian::write32le(D.data()+44,7);
  support::endian::write32le(D.data()+48,11469061);
  support::endian::write32le(D.data()+52,913);
  support::endian::write16le(D.data()+56,2078);
  return D;
}
Fixture callerFixture(McState &Mc) {
  Fixture F;
  const auto nibble=[](char C)->unsigned{return C<='9'?C-'0':C-'a'+10;};
  for(size_t I=0;I<sizeof(TransportCallerFixtureHex)-1;I+=2)
    F.Bytes.push_back(uint8_t((nibble(TransportCallerFixtureHex[I])<<4)|
                              nibble(TransportCallerFixtureHex[I+1])));
  F.F.Name="root";F.F.Address=2816;F.F.FileOffset=2816;
  F.F.Size=F.Bytes.size();F.F.Type=ELF::STT_FUNC;F.F.Text=true;F.F.Bytes=F.Bytes;
  F.Code=unwrap(decodeFunction(F.F,Mc));
  F.Cfg=unwrap(buildFunctionCfg(F.Code,Mc,F.F.Name));return F;
}
// Synthetic aggregate custody rows are explicitly inert. Instruction encodings,
// offsets, call opcode and CFG are taken from actual MC decoding above.
AnalyzedFunction callerAnalyzed(const Fixture &F) {
  AnalyzedFunction A;
  A.Evidence={F.F.Name,F.F.FileOffset,F.F.Size,{"helper"}};
  A.DirectCallSites=1;
  for(size_t N=0;N<F.Code.size();++N) {
    const auto &I=F.Code[N];PhysicalMachineInstructionTrace T;
    T.FunctionSymbol=F.F.Name;T.InstructionOffset=F.F.FileOffset+I.Address-F.F.Address;
    T.BlockOrdinal=uint32_t(F.Cfg.InstructionBlocks[N]);T.Encoding=I.Encoding;
    if(N==44){T.BranchKind=PhysicalMachineBranchKind::DirectCall;T.BranchTarget=2560;}
    A.Instructions.push_back(std::move(T));
  }
  return A;
}
bool callerBind(TransportAttempt &T,const Fixture &F,
                std::array<uint8_t,64> D=callerDescriptor()) {
  return T.bind(MetadataKernel{"root","root.kd",288,0,0,true},F.F,D);
}
Fixture callerHelper(ct::Program P,McState &Mc) {
  auto H=fixture(ct::Profile::ConditionalHelperO0,P,Mc);
  H.F.Address=2560;H.F.FileOffset=2560;H.F.Bytes=H.Bytes;
  H.Code=unwrap(decodeFunction(H.F,Mc));
  H.Cfg=unwrap(buildFunctionCfg(H.Code,Mc,H.F.Name));return H;
}
bool callerCase(Fixture &F,AnalyzedFunction &E,Fixture &H,McState &Mc,ct::Program P) {
  ct::Account Account(ct::StorageBytes,ct::WorkUnits);
  auto Reservation=unwrap(ct::Reservation::acquire(Account));
  TransportAttempt T(source(ct::Profile::CallerToHelperO0,P));
  return callerBind(T,F) && TransportControls::caller(T,F.Code,F.Cfg,Mc,F.F,E) &&
    TransportControls::evaluate(T,H.Code,H.Cfg,Mc,H.F);
}
void callerControls(McState &Mc) {
  auto F=callerFixture(Mc);auto E=callerAnalyzed(F);
  require(F.Code.size()==144 && F.Cfg.Blocks.size()==7,"caller actual inert MC/CFG");
  for(auto P:{ct::Program::XorAnd,ct::Program::MoveInput2}) {
    auto H=callerHelper(P,Mc);
    require(callerCase(F,E,H,Mc,P),"derived caller arguments reach actual helper result");
    for(unsigned N=0;N<7;++N) {
      ct::Account Account(ct::StorageBytes,ct::WorkUnits);
      auto Reservation=unwrap(ct::Reservation::acquire(Account));
      TransportAttempt T(source(ct::Profile::CallerToHelperO0,P));
      require(callerBind(T,F) && TransportControls::caller(T,F.Code,F.Cfg,Mc,F.F,E),
        "caller fault baseline first");
      TransportControls::callerFault(T,N);
      require(!TransportControls::evaluate(T,H.Code,H.Cfg,Mc,H.F),
        "stale caller argument/domain/link/target/consumption refused");
    }
    ct::Account Account(ct::StorageBytes,ct::WorkUnits);
    auto Reservation=unwrap(ct::Reservation::acquire(Account));
    TransportAttempt NoCaller(source(ct::Profile::CallerToHelperO0,P));
    require(!TransportControls::evaluate(NoCaller,H.Code,H.Cfg,Mc,H.F),
      "helper cannot manufacture profile3 inputs without caller");
  }
  auto H=callerHelper(ct::Program::XorAnd,Mc);
  for(unsigned N=0;N<17;++N) {
    auto Bad=F;auto A=E;
    switch(N) {
    case 0: Bad.Code[6].Inst.getOperand(2).setImm(2);break;
    case 1: Bad.Code[8].Inst.getOperand(1).setReg(F.Code[6].Inst.getOperand(1).getReg());break;
    case 2: Bad.Code[11].Inst.getOperand(2).setImm(0);break;
    case 3: Bad.Code[6].Inst.getOperand(3).setReg(F.Code[13].Inst.getOperand(0).getReg());break;
    case 4: Bad.Code[17].Inst.getOperand(2).setImm(20);break;
    case 5: Bad.Code[20].Inst.getOperand(0).setImm(49280);break;
    case 6: Bad.Code[29].Inst.getOperand(1).setReg(F.Code[5].Inst.getOperand(0).getReg());break;
    case 7: Bad.Code[41].Inst.getOperand(1).setReg(F.Code[42].Inst.getOperand(1).getReg());break;
    case 8: Bad.Code[44].Inst.getOperand(0).setReg(F.Code[5].Inst.getOperand(0).getReg());break;
    case 9: Bad.Cfg.Blocks[0].Predecessors.push_back(1);break;
    case 10: Bad.Cfg.Blocks[1].Successors.push_back(0);break;
    case 11: ++A.DirectCallSites;break;
    case 12: A.Instructions[44].BranchKind=PhysicalMachineBranchKind::None;break;
    case 13: A.Instructions[45].BranchKind=PhysicalMachineBranchKind::DirectCall;break;
    case 14: A.Instructions[44].BranchTarget+=4;break;
    case 15: A.Instructions[44].InstructionOffset+=4;break;
    case 16: A.Instructions[44].FunctionSymbol="foreign";break;
    }
    require(!callerCase(Bad,A,H,Mc,ct::Program::XorAnd),
      "actual lane/load/EXEC/call-site/CFG mutation refused");
  }
  for(unsigned N=0;N<8;++N) {
    auto D=callerDescriptor();
    constexpr unsigned Bytes[]{0,4,8,44,48,52,56,57};D[Bytes[N]]^=1;
    ct::Account Account(ct::StorageBytes,ct::WorkUnits);
    auto Reservation=unwrap(ct::Reservation::acquire(Account));
    TransportAttempt T(source(ct::Profile::CallerToHelperO0,ct::Program::XorAnd));
    require(!callerBind(T,F,D),"exact caller descriptor origin/resource mutation");
  }
  for(unsigned N=0;N<4;++N) {
    ct::Account Account(ct::StorageBytes+17,ct::WorkUnits+23,17,23);
    {
      auto Reservation=unwrap(ct::Reservation::acquire(Account));
      TransportAttempt T(source(ct::Profile::CallerToHelperO0,ct::Program::XorAnd));
      require(callerBind(T,F) && TransportControls::caller(T,F.Code,F.Cfg,Mc,F.F,E),
        "caller semantic fault baseline first");
      if(N==0) {
        require(TransportControls::callerLaneKill(T,*Mc.Registers),
          "actual half-register alias invalidates all selected lanes");
        require(!TransportControls::callerStep(T,F.Code[11],Mc,11),
          "readlane cannot recover invalidated pointer origin");
      }
      if(N==1) {
        TransportControls::callerPending(T);
        require(!TransportControls::callerStep(T,F.Code[41],Mc,41),
          "pending scalar argument cannot become ready by move");
      }
      if(N==2) {
        TransportControls::callerSavedExecStale(T);
        require(!TransportControls::callerStep(T,F.Code[29],Mc,29),
          "restore requires exact saved EXEC generation");
      }
      if(N==3) {
        TransportControls::callerFault(T,2);
        require(!TransportControls::callerStep(T,F.Code[41],Mc,41),
          "expanded EXEC cannot define entry-domain call arguments");
      }
    }
    require(Account.storage()==17 && Account.work()==ct::WorkUnits+23,
      "caller failure releases once and preserves original floor/work");
  }
  for(unsigned Short=0;Short<2;++Short) {
    ct::Account A(ct::StorageBytes+17-(Short==0?1:0),
                  ct::WorkUnits+23-(Short==1?1:0),17,23);
    PhysicalMachineEffectRequest Empty;
    auto Out=ct::analyze(Empty,source(ct::Profile::CallerToHelperO0,ct::Program::XorAnd),A);
    require(!Out && A.storage()==17 && A.work()==23 && A.denials()==1 &&
      A.last_denial()==(Short==0?ct::Refusal::ResourceStorage:ct::Refusal::ResourceWork),
      "caller one-short original ledger refuses before bytes/state");
    consumeError(Out.takeError());
  }
  // Full observe/commit interface, with inert aggregate evidence explicitly
  // marked by this fixture. No raw positive summary or reservation is exported.
  ct::Account Account(ct::StorageBytes,ct::WorkUnits);
  auto Reservation=unwrap(ct::Reservation::acquire(Account));
  TransportAttempt T(source(ct::Profile::CallerToHelperO0,ct::Program::XorAnd));
  require(callerBind(T,F),"caller aggregate bind");
  require(T.observe(F.F,F.Code,F.Cfg,{*Mc.Registers,*Mc.Instructions,*Mc.Analysis},E),
    "caller exact observed root");
  AnalyzedFunction HE;HE.Evidence={"helper",H.F.FileOffset,H.F.Size,{}};
  require(T.observe(H.F,H.Code,H.Cfg,{*Mc.Registers,*Mc.Instructions,*Mc.Analysis},HE),
    "caller exact observed helper");
  PhysicalMachineEffectEvidence Whole;
  Whole.Entries.push_back({"root",SHA256::hash(callerDescriptor()),F.F.FileOffset,F.F.Size});
  Whole.Functions={E.Evidence,HE.Evidence};
  Whole.Blocks.resize(F.Cfg.Blocks.size()+H.Cfg.Blocks.size());
  Whole.Instructions.resize(F.Code.size()+H.Code.size());
  require(T.finish(Whole),"caller whole aggregate commits only after both functions");
  require(T.summary().CallOffset==3064 && T.summary().CallTargetOffset==2560 &&
    T.summary().SelectedOffset==2560 && T.summary().CallerOffset==2816,
    "actual call coordinates retained");
  require(!T.finish(Whole),"caller aggregate cannot be committed twice");
}
