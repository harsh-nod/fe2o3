// Exact numeric table measured by the root's caller-metadata actual R1.
// Selected table SHA256 31673b008249a9e4676b4006d3d794748ec80b3fcaa328555cec884fbcff1438.
// Names are absent from admission. This table is private to profile 3; the
// original two profiles retain their exact old opcode/operand shape tables.
enum class CallerOp : uint8_t {
 Move32,Move64,WriteLane,ReadLane,Move,AccWrite,Load1,Load2,Wait,Nop,SaveExec,GetPc,Add,AddCarry,Call
};
struct CallerInstructionShape {
  uint16_t Opcode;
  CallerOp Operation;
  uint8_t Defs,Count,Flags,ImplicitDefCount,ImplicitUseCount;
  std::array<TransportOperandShape,4> Operands;
  std::array<int8_t,4> TiedTo;
  std::array<uint16_t,2> ImplicitDefs;
  std::array<uint16_t,1> ImplicitUses;
};
constexpr std::array<CallerInstructionShape,15> TransportCallerShapes{{
 {33617,CallerOp::Move32,1,2,0,0,0,
  {{{36,32,2},{36,32,13},{},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {33622,CallerOp::Move64,1,2,0,0,0,
  {{{75,64,2},{73,64,14},{},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {45850,CallerOp::WriteLane,1,4,0,0,0,
  {{{27,32,2},{36,32,27},{36,32,27},{27,32,2}}},
  {-1,-1,-1,0},
  {},{}},
 {44837,CallerOp::ReadLane,1,3,0,0,0,
  {{{38,32,2},{26,32,2},{36,32,27},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {44150,CallerOp::Move,1,2,0,0,1,
  {{{27,32,2},{24,32,13},{},{}}},
  {-1,-1,-1,-1},
  {},{2}},
 {34447,CallerOp::AccWrite,1,2,0,0,1,
  {{{33,32,2},{24,32,27},{},{}}},
  {-1,-1,-1,-1},
  {},{2}},
 {33496,CallerOp::Load1,1,4,1,0,0,
  {{{41,32,2},{75,64,2},{-1,0,1},{-1,0,1}}},
  {-1,-1,-1,-1},
  {},{}},
 {33466,CallerOp::Load2,1,4,1,0,0,
  {{{76,64,2},{75,64,2},{-1,0,1},{-1,0,1}}},
  {-1,-1,-1,-1},
  {},{}},
 {33979,CallerOp::Wait,0,1,0,0,0,
  {{{-1,0,1},{},{},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {33668,CallerOp::Nop,0,1,0,0,0,
  {{{-1,0,1},{},{},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {33739,CallerOp::SaveExec,1,2,0,2,1,
  {{{75,64,2},{73,64,14},{},{}}},
  {-1,-1,-1,-1},
  {2,19},{2}},
 {33402,CallerOp::GetPc,1,1,0,0,0,
  {{{75,64,2},{},{},{}}},
  {-1,-1,-1,-1},
  {},{}},
 {31919,CallerOp::Add,1,3,0,1,0,
  {{{36,32,2},{36,32,13},{36,32,13},{}}},
  {-1,-1,-1,-1},
  {19},{}},
 {31899,CallerOp::AddCarry,1,3,0,1,1,
  {{{36,32,2},{36,32,13},{36,32,13},{}}},
  {-1,-1,-1,-1},
  {19},{19}},
 {33943,CallerOp::Call,1,2,64,0,0,
  {{{75,64,2},{73,64,14},{},{}}},
  {-1,-1,-1,-1},
  {},{}}
}};
const CallerInstructionShape *transportCallerShape(unsigned Opcode) {
  for(const auto &S:TransportCallerShapes)if(S.Opcode==Opcode)return &S;
  return nullptr;
}
bool TransportAttempt::callerShape(const DecodedInstruction &I,const TransportMcView &Mc) {
  const auto &MRI=Mc.Registers;const auto &MII=Mc.Instructions;
  if(I.Inst.getOpcode()>=MII.getNumOpcodes())return fail(ct::Refusal::Shape);
  const auto *S=transportCallerShape(I.Inst.getOpcode());
  if(!S || I.Size==0 || I.Size>8 || I.Encoding.size()!=I.Size ||
     I.Inst.getNumOperands()!=S->Count)return fail(ct::Refusal::Shape);
  const auto &D=MII.get(I.Inst.getOpcode());
  const unsigned Flags=(D.mayLoad()?1:0)|(D.mayStore()?2:0)|
      (D.isTerminator()?4:0)|(D.isBarrier()?8:0)|(D.isPredicable()?16:0)|
      (D.isTrap()?32:0)|(D.isCall()?64:0);
  if(D.getOpcode()!=S->Opcode || D.getNumDefs()!=S->Defs ||
     D.getNumOperands()!=S->Count || Flags!=S->Flags ||
     D.isVariadic() || D.hasOptionalDef() || D.isPseudo() ||
     D.implicit_defs().size()!=S->ImplicitDefCount ||
     D.implicit_uses().size()!=S->ImplicitUseCount)
    return fail(ct::Refusal::Shape);
  for(unsigned N=0;N<S->Count;++N) {
    const auto &O=I.Inst.getOperand(N);const auto T=D.operands()[N];
    if(D.getOperandConstraint(N,MCOI::TIED_TO)!=S->TiedTo[N] ||
       T.isLookupPtrRegClass() || T.isLookupRegClassByHwMode() ||
       T.RegClass!=S->Operands[N].Class || T.OperandType!=S->Operands[N].Type)
      return fail(ct::Refusal::Shape);
    if(T.RegClass>=0 &&
       (unsigned(T.RegClass)>=MRI.getNumRegClasses() ||
        MRI.getRegClass(T.RegClass).getSizeInBits()!=S->Operands[N].Width ||
        S->Operands[N].Width==0))return fail(ct::Refusal::Table);
    if(O.isReg()) {
      if(T.RegClass<0 || O.getReg()==0 || O.getReg()>=MRI.getNumRegs() ||
         !MRI.getRegClass(T.RegClass).contains(O.getReg()) ||
         !words(O.getReg(),S->Operands[N].Width,MRI))
        return fail(ct::Refusal::Register);
    } else if(!O.isImm() || N<S->Defs)return fail(ct::Refusal::Shape);
    if(S->TiedTo[N]>=0 &&
       (!O.isReg() || unsigned(S->TiedTo[N])>=N ||
        !I.Inst.getOperand(S->TiedTo[N]).isReg() ||
        O.getReg()!=I.Inst.getOperand(S->TiedTo[N]).getReg()))
      return fail(ct::Refusal::Alias);
  }
  const auto implicit=[&](unsigned Id) {
    return Id==19?cache(Id,MRI):words(Id,64,MRI);
  };
  for(unsigned N=0;N<S->ImplicitDefCount;++N)
    if(D.implicit_defs()[N]!=S->ImplicitDefs[N] || !implicit(D.implicit_defs()[N]))
      return fail(ct::Refusal::Shape);
  for(unsigned N=0;N<S->ImplicitUseCount;++N)
    if(D.implicit_uses()[N]!=S->ImplicitUses[N] || !implicit(D.implicit_uses()[N]))
      return fail(ct::Refusal::Shape);
  return true;
}
