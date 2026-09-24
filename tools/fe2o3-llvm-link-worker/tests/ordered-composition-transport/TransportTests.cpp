// Pure CPU/inert fixtures. No source, code-object or runtime authority is minted.
#define FE2O3_PRIVATE_COMPOSITION_TRANSPORT_V1 1
#define FE2O3_PRIVATE_COMPOSITION_TRANSPORT_CONTROLS_V1 1
#include "../../src/WorkerMachineEffect.cpp"
#include <cstdlib>
#include <type_traits>
namespace fe2o3::worker { namespace {
struct TransportControls {
  static bool evaluate(TransportAttempt &A,ArrayRef<DecodedInstruction> I,
                       const FunctionCfg &C,const McState &M,const SymbolRecord &F) {
    return A.evaluate(I,C,{*M.Registers,*M.Instructions,*M.Analysis},F);
  }
  static bool caller(TransportAttempt &A,ArrayRef<DecodedInstruction> I,
                     const FunctionCfg &C,const McState &M,const SymbolRecord &F,
                     const AnalyzedFunction &E) {
    return A.caller(I,C,{*M.Registers,*M.Instructions,*M.Analysis},F,E);
  }
  static bool callerStep(TransportAttempt &A,const DecodedInstruction &I,
                         const McState &M,size_t At) {
    return A.callerStep(I,{*M.Registers,*M.Instructions,*M.Analysis},At);
  }
  static void callerFault(TransportAttempt &A,unsigned Fault) {
    if(Fault==0)++A.W.Caller.Arguments[0].Generation;
    if(Fault==1)A.W.Caller.Arguments[1].Origin=1;
    if(Fault==2)A.W.Caller.ExpandedExec=true;
    if(Fault==3)A.W.Caller.Arguments[2].Ready=TransportReady::Pending;
    if(Fault==4)A.W.Caller.ReturnPair=7675; // actual measured s0:s1
    if(Fault==5)A.W.Caller.TargetOffset+=4;
    if(Fault==6)A.W.Caller.Consumed=true;
  }
  static bool callerLaneKill(TransportAttempt &A,const MCRegisterInfo &MRI) {
    const unsigned Id=A.W.Caller.LaneRegister;
    for(MCRegister Half:MRI.subregs(Id)) {
      if(!A.cache(Half.id(),MRI))return false;
      if(A.find(Half.id())->WordCount!=0)continue;
      if(!A.callerWrite(Half.id(),A.W.States[0],false))return false;
      return llvm::all_of(A.W.Caller.Lanes,[](auto F){
        return F.Ready==TransportReady::Unknown;});
    }
    return false;
  }
  static void callerPending(TransportAttempt &A) {
    const auto *R=A.find(A.callerWord(TransportBank::Scalar,16));
    A.W.States[0].Facts[R->Slot].Ready=TransportReady::Pending;
  }
  static void callerSavedExecStale(TransportAttempt &A) {
    A.W.Caller.ExpandedExec=true;
    const auto *R=A.find(A.callerWord(TransportBank::Scalar,20));
    ++A.W.States[0].Facts[R->Slot].Generation;
  }
  static bool pendingKill(TransportAttempt &A) {
    for(size_t I=0;I<A.WordCount;++I) if(A.W.Words[I].Bank==TransportBank::Scalar &&
        A.W.Words[I].Index==8) {
      A.W.States[0].Facts[I].Ready=TransportReady::Pending;
      return !A.kill(A.W.Words[I].Id,A.W.States[0]) &&
             A.refusal()==ct::Refusal::Pending;
    }
    return false;
  }
  static bool accDistinct(const TransportAttempt &A) {
    const TransportRegister *V=nullptr,*Acc=nullptr;
    for(size_t I=0;I<A.RegisterCount;++I) {
      const auto &R=A.W.Registers[I];
      if(R.Width==32 && R.Bank==TransportBank::Vector && R.Index==0)V=&R;
      if(R.Width==32 && R.Bank==TransportBank::Accumulator && R.Index==0)Acc=&R;
    }
    return V && Acc && V->Slot!=Acc->Slot && V->Units[0]!=Acc->Units[0];
  }
  static bool halfKill(TransportAttempt &A,const MCRegisterInfo &MRI) {
    const TransportRegister *V=nullptr;
    for(size_t I=0;I<A.RegisterCount;++I)
      if(A.W.Registers[I].Width==32 &&
         A.W.Registers[I].Bank==TransportBank::Vector && A.W.Registers[I].Index==0)
        V=&A.W.Registers[I];
    if(!V)return false;
    const auto Id=V->Id;const auto Slot=V->Slot;
    for(MCRegister Sub:MRI.subregs(Id)) {
      if(!A.cache(Sub.id(),MRI))return false;
      const auto *Part=A.find(Sub.id());
      if(Part->WordCount!=0)continue;
      A.W.States[0].Facts[Slot]={1,1,TransportDomain::Entry,TransportReady::Ready,0,0};
      return A.kill(Sub.id(),A.W.States[0]) &&
             A.W.States[0].Facts[Slot].Ready==TransportReady::Unknown;
    }
    return false;
  }
  static void observing(TransportAttempt &A) {A.Observing=true;}
  static void aggregateFixture(TransportAttempt &A,PhysicalMachineEffectEvidence &E,
                               const SymbolRecord &F) {
    // Inert cursor-control fixture only. The actual API cannot call this helper.
    A.Bound=true;A.RootSeen=true;A.SelectedSeen=true;
    A.FunctionCount=1;A.BlockCount=3;A.InstructionCount=19;
    A.Summary.SelectedOffset=F.FileOffset;A.Summary.SelectedBytes=F.Size;
    A.Summary.NativeDescriptor.fill(7);
    E.Entries.push_back({"root",A.Summary.NativeDescriptor,F.FileOffset,F.Size});
    E.Functions.push_back({"root",F.FileOffset,F.Size,{}});
    E.Blocks.resize(3);E.Instructions.resize(19);
  }
};
} }
using namespace fe2o3::worker;
namespace {
unsigned Checks=0;
void require(bool B,const char *Name) {
  if(!B){errs()<<"transport control failed: "<<Name<<"\n";std::exit(1);}
  ++Checks;
}
template<class T>T unwrap(Expected<T> V) {
  if(!V){errs()<<toString(V.takeError())<<"\n";std::exit(1);}
  return std::move(*V);
}
template<class T>T unwrap(std::optional<T> V) {
  if(!V){errs()<<"transport fixed reservation refused\n";std::exit(1);}
  return std::move(*V);
}
ct::SourceProjection source(ct::Profile P,ct::Program Program) {
  ct::SourceProjection S;S.Root="root";S.Helper="helper";S.Selected=P;
  S.SelectedProgram=Program;S.Canonical.fill(1);S.Llvm.fill(2);
  S.SourceReport.fill(3);S.Descriptor.fill(4);
  const auto row=[&](uint32_t T,uint32_t A,uint32_t B,uint32_t C) {
    if(S.RowCount>=64)std::abort();
    S.Rows[S.RowCount++]={T,A,B,C};
  };
  row(1,0,4,0);
  for(uint32_t I=0;I<4;++I)row(2,I,I,I?1:2);
  row(3,1,3,1);for(uint32_t I=0;I<3;++I)row(4,I,I,1);
  row(5,1,0,0);row(6,1,0,0);row(7,1,10,1);
  for(uint32_t I=0;I<3;++I){row(8,I,I+1,I);row(16,I,I+1,I+1);}
  row(5,2,0,0);row(6,2,0,1);
  row(11,8,9,Program==ct::Program::XorAnd?2:1);
  for(uint32_t I=0;I<3;++I){row(9,I,I,1);row(12,I,10+I,0);}
  if(Program==ct::Program::XorAnd){row(13,0,133,0);row(13,1,315,0);}
  else row(13,0,40,0);
  row(10,4,1,0);row(5,3,0,1);row(6,3,0,1);row(14,4,10,1);
  row(5,4,1,0);row(6,4,1,0);row(15,10,20,21);row(16,3,10,10);
  row(17,0,0,(8U<<16)|8);row(18,0,8,(8U<<16)|8);
  for(uint32_t I=0;I<3;++I)row(19,I+1,16+4*I,(4U<<16)|4);
  return S;
}
struct Fixture {
  std::vector<uint8_t> Bytes;
  SymbolRecord F;
  std::vector<DecodedInstruction> Code;
  FunctionCfg Cfg;
};
Fixture fixture(ct::Profile P,ct::Program Program,McState &Mc) {
  Fixture X;
  const auto Count=(P==ct::Profile::ConditionalHelperO0?10U:19U)-
      (Program==ct::Program::MoveInput2?1U:0U);
  for(size_t I=0;I<Count;++I) {
    const auto E=transportExpected(P,Program,I);
    uint8_t B[8];support::endian::write32le(B,E.Literal.Low);
    support::endian::write32le(B+4,E.Literal.High);
    X.Bytes.insert(X.Bytes.end(),B,B+E.Literal.Bytes);
  }
  X.F.Name=P==ct::Profile::ConditionalHelperO0?"helper":"root";
  X.F.Address=4096;X.F.FileOffset=512;X.F.Size=X.Bytes.size();
  X.F.Type=ELF::STT_FUNC;X.F.Text=true;X.F.Bytes=X.Bytes;
  X.Code=unwrap(decodeFunction(X.F,Mc));
  X.Cfg=unwrap(buildFunctionCfg(X.Code,Mc,X.F.Name));
  return X;
}
bool evaluate(const ct::SourceProjection &S,Fixture &F,const McState &Mc) {
  ct::Account A(ct::StorageBytes,ct::WorkUnits);
  auto R=unwrap(ct::Reservation::acquire(A));
  TransportAttempt T(S);
  return T.good() && TransportControls::evaluate(T,F.Code,F.Cfg,Mc,F.F);
}
void sourceControls() {
  for(const auto Program:{ct::Program::XorAnd,ct::Program::MoveInput2}) {
    auto S=source(ct::Profile::InlineRootO3,Program);TransportProjection P;
    require(transportProjection(S,P),"source exact typed row grammar");
    auto Changed=S;Changed.Rows[0][1]=1;
    require(!transportProjection(Changed,P),"source same root/helper refuses");
    Changed=S;Changed.Rows[13][2]^=1;
    require(!transportProjection(Changed,P),"source distinct SSA refuses");
    Changed=S;Changed.Rows[Changed.RowCount-1][2]=28;
    require(!transportProjection(Changed,P),"padding is not argument");
    Changed=S;--Changed.RowCount;
    require(!transportProjection(Changed,P),"missing ABI component");
    Changed=S;Changed.Descriptor.fill(0);
    require(!transportProjection(Changed,P),"absent descriptor relation");
  }
}
void accountingControls() {
  static_assert(!std::is_copy_constructible_v<ct::Result>);
  static_assert(!std::is_default_constructible_v<ct::Result>);
  static_assert(!std::is_constructible_v<ct::Result,PhysicalMachineEffectEvidence&&,
                                       ct::Summary,ct::Reservation&&>);
  static_assert(!std::is_copy_constructible_v<ct::Reservation>);
  ct::Account A(ct::StorageBytes+17,ct::WorkUnits+23,17,23);
  {
    auto R=ct::Reservation::acquire(A);
    require(bool(R),"exact prefixed account");
    require(A.storage()==ct::StorageBytes+17 && A.work()==ct::WorkUnits+23,
            "original floor and work retained");
  }
  require(A.storage()==17 && A.peak()==ct::StorageBytes+17 &&
          A.work()==ct::WorkUnits+23,"single storage release no work refund");
  auto Again=ct::Reservation::acquire(A);
  require(!Again,"second attempt cannot reset work");
  ct::Account S(ct::StorageBytes-1,ct::WorkUnits);
  auto ShortStorage=ct::Reservation::acquire(S);
  require(!ShortStorage && S.storage()==0 && S.work()==0 && S.denials()==1,
          "one-short storage before retention");
  ct::Account W(ct::StorageBytes,ct::WorkUnits-1);
  auto ShortWork=ct::Reservation::acquire(W);
  require(!ShortWork && W.storage()==0 && W.work()==0 && W.denials()==1,
          "one-short work before retention");
  ct::Account Overflow(UINT64_MAX,UINT64_MAX,UINT64_MAX-1,UINT64_MAX-1);
  auto O=ct::Reservation::acquire(Overflow);
  require(!O && Overflow.storage()==UINT64_MAX-1 && Overflow.work()==UINT64_MAX-1,
          "overflow denied without wrap");
  ct::Account Late(ct::StorageBytes,ct::WorkUnits);
  {
    auto R=unwrap(ct::Reservation::acquire(Late));
    TransportAttempt T(source(ct::Profile::InlineRootO3,ct::Program::XorAnd));
    PhysicalMachineEffectEvidence Missing;
    require(!T.finish(Missing),"aggregate cannot commit missing observation");
  }
  require(Late.storage()==0 && Late.work()==ct::WorkUnits,
          "late aggregate failure preserves debit releases once");
}
void machineControls(McState &Mc) {
  for(const auto Profile:{ct::Profile::ConditionalHelperO0,ct::Profile::InlineRootO3})
  for(const auto Program:{ct::Program::XorAnd,ct::Program::MoveInput2}) {
    auto S=source(Profile,Program);auto F=fixture(Profile,Program,Mc);
    require(evaluate(S,F,Mc),"real MC fixed profile positive");
    auto Bad=F;
    Bad.Code[1].Inst.addOperand(MCOperand::createImm(0));
    require(!evaluate(S,Bad,Mc),"extra actual operand");
    Bad=F;Bad.Code.back().Inst.setOpcode(unsigned(TransportOp::Nop));
    require(!evaluate(S,Bad,Mc),"return opcode differs");
    Bad=F;Bad.Code[0].Encoding[0]^=1;
    require(!evaluate(S,Bad,Mc),"encoding differs from typed decoded subject");
    Bad=F;Bad.Code[1].Address+=4;
    require(!evaluate(S,Bad,Mc),"actual physical site substitution");
    Bad=F;Bad.F.Size+=4;
    require(!evaluate(S,Bad,Mc),"selected extent trailing bytes");
    Bad=F;Bad.Cfg.Blocks[0].Successors.push_back(0);
    require(!evaluate(S,Bad,Mc),"actual CFG loop/extra edge");
    const size_t Move=Profile==ct::Profile::ConditionalHelperO0?1:7;
    Bad=F;
    Bad.Code[Move].Inst.getOperand(1).setReg(
      F.Code[Move+1].Inst.getOperand(1).getReg());
    require(!evaluate(S,Bad,Mc),"actual argument register substitution");
    Bad=F;Bad.Code.erase(Bad.Code.begin());
    require(!evaluate(S,Bad,Mc),"missing actual load or wait row");
    ct::Account A(ct::StorageBytes,ct::WorkUnits);
    auto Held=unwrap(ct::Reservation::acquire(A));TransportAttempt T(S);
    require(TransportControls::evaluate(T,F.Code,F.Cfg,Mc,F.F),
            "state fixture admitted using actual MC");
    if(Profile==ct::Profile::ConditionalHelperO0) {
      require(TransportControls::accDistinct(T),"AGPR0 is not VGPR0 alias");
      require(TransportControls::halfKill(T,*Mc.Registers),"real half alias kills full word");
      Bad=F;Bad.Code.back().Inst.getOperand(0).setReg(
        fixture(ct::Profile::InlineRootO3,Program,Mc).Code[0].Inst.getOperand(1).getReg());
      require(!evaluate(S,Bad,Mc),"wrong physical return pair");
    } else {
      require(TransportControls::pendingKill(T),"pending definition clobber refuses");
      Bad=F;Bad.Code[5].Inst.getOperand(0).setImm(49280);
      require(!evaluate(S,Bad,Mc),"nonzero LGKM wait refuses");
      Bad=F;Bad.Code[5].Inst.getOperand(0).setImm(49279|0x1000);
      require(!evaluate(S,Bad,Mc),"reserved wait bit refuses");
      Bad=F;Bad.Code[0].Inst.getOperand(2).setImm(4);
      require(!evaluate(S,Bad,Mc),"wrong kernarg byte offset");
      Bad=F;Bad.Code[0].Inst.getOperand(3).setImm(1);
      require(!evaluate(S,Bad,Mc),"changed load cache policy");
      Bad=F;Bad.Code[Bad.Code.size()-2].Inst.getOperand(1).setReg(
        F.Code[7].Inst.getOperand(0).getReg());
      require(!evaluate(S,Bad,Mc),"store DATA substitution");
      Bad=F;Bad.Cfg.Blocks[0].Successors[0]=0;
      require(!evaluate(S,Bad,Mc),"wrong actual branch target");
    }
  }
}
void descriptorAndCursorControls() {
  auto S=source(ct::Profile::InlineRootO3,ct::Program::XorAnd);
  MetadataKernel K{"root","root.kd",288,0,0,true};
  SymbolRecord F;F.Name="root";F.FileOffset=512;F.Size=88;
  std::array<uint8_t,64> D{};
  support::endian::write32le(D.data()+8,288);
  support::endian::write32le(D.data()+44,3);
  support::endian::write32le(D.data()+48,11468929);
  support::endian::write32le(D.data()+52,132);
  support::endian::write16le(D.data()+56,8);
  const auto bind=[&](MetadataKernel M,std::array<uint8_t,64> Bytes) {
    ct::Account A(ct::StorageBytes,ct::WorkUnits);
    auto Held=unwrap(ct::Reservation::acquire(A));TransportAttempt T(S);
    return T.bind(M,F,Bytes);
  };
  require(bind(K,D),"closed descriptor byte shape");
  auto Bad=D;Bad[56]^=4;require(!bind(K,Bad),"descriptor enable order");
  Bad=D;Bad[52]^=2;require(!bind(K,Bad),"descriptor user SGPR count");
  Bad=D;Bad[57]^=4;require(!bind(K,Bad),"wave32 properties");
  auto Missing=K;Missing.TransportShape=false;
  require(!bind(Missing,D),"actual metadata ABI prefix absent");
  ct::Account A(ct::StorageBytes,ct::WorkUnits);
  auto Held=unwrap(ct::Reservation::acquire(A));TransportAttempt T(S);
  require(T.bind(K,F,D),"first descriptor bind");
  require(!T.bind(K,F,D),"bind cannot renew");
  TransportAttempt Reentry(S);TransportControls::observing(Reentry);
  require(!Reentry.bind(K,F,D),"nested observation refuses");
  TransportAttempt Done(S);PhysicalMachineEffectEvidence E;
  TransportControls::aggregateFixture(Done,E,F);
  auto Wrong=E;Wrong.Entries[0].DescriptorIdentity[0]^=1;
  require(!Done.finish(Wrong),"late descriptor substitution refuses");
  require(!Done.finish(E),"late failure is sticky");
  TransportAttempt Once(S);PhysicalMachineEffectEvidence Good;
  TransportControls::aggregateFixture(Once,Good,F);
  require(Once.finish(Good),"inert aggregate cursor positive");
  require(!Once.finish(Good),"finish once only");
  const TransportFact Entry{1,0,TransportDomain::Entry,TransportReady::Ready,0,0};
  const TransportFact Narrow{1,0,TransportDomain::Intersection,TransportReady::Ready,0,0};
  require(transportAvailable(Entry,TransportDomain::Intersection),"entry facts narrow");
  require(!transportAvailable(Narrow,TransportDomain::Entry),"narrow facts cannot expand");
  const TransportFact Pending{1,2,TransportDomain::Uniform,TransportReady::Pending,2,0};
  require(!transportAvailable(Pending,TransportDomain::Entry),
          "opaque load result is unavailable before wait");
}
#include "TransportCallerTestsV1.inc"
}
int main(int Argc,char **) {
  if(Argc!=1){errs()<<"transport controls accept no arguments\n";return 2;}
  sourceControls();accountingControls();
  auto Mc=unwrap(createMcState());machineControls(Mc);descriptorAndCursorControls();
  callerControls(Mc);
  outs()<<"transport controls "<<Checks<<" passed; no source/native authority\n";
  return 0;
}
