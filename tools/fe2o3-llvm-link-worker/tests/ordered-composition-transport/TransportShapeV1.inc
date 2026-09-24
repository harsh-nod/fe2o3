// Opcode IDs are from the exact root-qualified f58b06d LLVM table. They are
// independently joined to the real operand classes/effects and literal machine
// encoding. No diagnostic name is an opcode or register authority.
enum class TransportOp : uint16_t {
  Wait=33979, Move=44150, Xor=45911, And=34720, Nop=33668,
  AccWrite=34447, AccRead=34446, Return=33853, Load8=33486,
  ScalarMove=33617, ScalarShift=33536, Or=44523, Compare=37947,
  SaveExec=31978, Branch=33032, Address=42964, Store=17568, End=33348
};
struct TransportOperandShape {int16_t Class;uint16_t Width;uint8_t Type;};
struct TransportInstructionShape {
  TransportOp Op;uint8_t Defs,Count,Flags,ImplicitDefCount,ImplicitUseCount;
  std::array<TransportOperandShape,4> Operands;
  std::array<uint16_t,2> ImplicitDefs;
  std::array<uint16_t,1> ImplicitUses;
};
constexpr TransportOperandShape TI{-1,0,1};
constexpr std::array<TransportInstructionShape,18> TransportShapes{{
 {TransportOp::Wait,0,1,0,0,0,{{TI,{},{},{}}},{},{}},
 {TransportOp::Move,1,2,0,0,1,{{{27,32,2},{24,32,13},{},{}}},{},{2}},
 {TransportOp::Xor,1,3,0,0,1,{{{27,32,2},{24,32,13},{27,32,2},{}}},{},{2}},
 {TransportOp::And,1,3,0,0,1,{{{27,32,2},{24,32,13},{27,32,2},{}}},{},{2}},
 {TransportOp::Nop,0,1,0,0,0,{{TI,{},{},{}}},{},{}},
 {TransportOp::AccWrite,1,2,0,0,1,{{{33,32,2},{24,32,27},{},{}}},{},{2}},
 {TransportOp::AccRead,1,2,0,0,1,{{{27,32,2},{33,32,2},{},{}}},{},{2}},
 {TransportOp::Return,0,1,12,0,0,{{{75,64,2},{},{},{}}},{},{}},
 {TransportOp::Load8,1,4,1,0,0,{{{333,256,2},{75,64,2},TI,TI}},{},{}},
 {TransportOp::ScalarMove,1,2,0,0,0,{{{36,32,2},{36,32,13},{},{}}},{},{}},
 {TransportOp::ScalarShift,1,3,0,1,0,{{{75,64,2},{73,64,14},{36,32,13},{}}},{19},{}},
 {TransportOp::Or,1,3,0,0,1,{{{27,32,2},{24,32,13},{27,32,2},{}}},{},{2}},
 {TransportOp::Compare,0,2,0,1,1,{{{49,64,14},{51,64,2},{},{}}},{45},{2}},
 {TransportOp::SaveExec,1,2,0,2,1,{{{75,64,2},{73,64,14},{},{}}},{2,19},{2}},
 {TransportOp::Branch,0,1,4,0,1,{{{-1,0,4},{},{},{}}},{},{2}},
 {TransportOp::Address,1,4,0,0,1,{{{51,64,2},{49,64,14},{24,32,13},{49,64,14}}},{},{2}},
 {TransportOp::Store,0,4,2,0,1,{{{51,64,2},{23,32,2},TI,TI}},{},{2}},
 {TransportOp::End,0,1,12,0,0,{{TI,{},{},{}}},{},{}}
}};
const TransportInstructionShape *transportShape(unsigned Opcode) {
  for(const auto &S:TransportShapes) if(unsigned(S.Op)==Opcode)return &S;
  return nullptr;
}
bool TransportAttempt::shape(const DecodedInstruction &I,const TransportMcView &Mc) {
  const auto &MRI=Mc.Registers;const auto &MII=Mc.Instructions;
  if(I.Inst.getOpcode()>=MII.getNumOpcodes())return fail(ct::Refusal::Shape);
  const auto *S=transportShape(I.Inst.getOpcode());
  if(!S || I.Size==0 || I.Size>16 || I.Encoding.size()!=I.Size ||
     I.Inst.getNumOperands()!=S->Count)return fail(ct::Refusal::Shape);
  const auto &D=MII.get(I.Inst.getOpcode());
  const unsigned Flags=(D.mayLoad()?1:0)|(D.mayStore()?2:0)|
      (D.isTerminator()?4:0)|(D.isBarrier()?8:0)|
      (D.isPredicable()?16:0)|(D.isTrap()?32:0);
  if(D.getOpcode()!=I.Inst.getOpcode() || D.getNumDefs()!=S->Defs ||
     D.getNumOperands()!=S->Count || Flags!=S->Flags ||
     D.isCall() || D.isVariadic() || D.hasOptionalDef() || D.isPseudo() ||
     D.implicit_defs().size()!=S->ImplicitDefCount ||
     D.implicit_uses().size()!=S->ImplicitUseCount ||
     S->Count+S->ImplicitDefCount+S->ImplicitUseCount>16)
    return fail(ct::Refusal::Shape);
  for(size_t N=0;N<S->Count;++N) {
    const auto &O=I.Inst.getOperand(N);const auto T=D.operands()[N];
    if(D.getOperandConstraint(N,MCOI::TIED_TO)!=-1 ||
       T.isLookupPtrRegClass() || T.isLookupRegClassByHwMode() ||
       T.RegClass!=S->Operands[N].Class || T.OperandType!=S->Operands[N].Type)
      return fail(ct::Refusal::Shape);
    if(T.RegClass>=0) {
      if(unsigned(T.RegClass)>=MRI.getNumRegClasses() ||
         MRI.getRegClass(T.RegClass).getSizeInBits()!=S->Operands[N].Width ||
         S->Operands[N].Width==0) return fail(ct::Refusal::Table);
    }
    if(O.isReg()) {
      if(T.RegClass<0 || O.getReg()==0 || O.getReg()>=MRI.getNumRegs() ||
         !MRI.getRegClass(T.RegClass).contains(O.getReg()) ||
         !words(O.getReg(),S->Operands[N].Width,MRI))
        return fail(ct::Refusal::Register);
    } else if(!O.isImm() || N<S->Defs) return fail(ct::Refusal::Shape);
  }
  for(size_t N=0;N<S->ImplicitDefCount;++N) {
    const unsigned Id=D.implicit_defs()[N];
    if(Id!=S->ImplicitDefs[N] ||
       !(Id==19?cache(Id,MRI):words(Id,64,MRI)))
      return fail(ct::Refusal::Shape);
  }
  for(size_t N=0;N<S->ImplicitUseCount;++N) {
    const unsigned Id=D.implicit_uses()[N];
    if(Id!=S->ImplicitUses[N] || !words(Id,64,MRI))
      return fail(ct::Refusal::Shape);
  }
  return true;
}
// Exactly the finite observed architectural forms. The bytes are independent
// literal ISA expectations; operands are separately checked by live MC above.
struct TransportLiteral {TransportOp Op;uint32_t Low,High;uint8_t Bytes;};
constexpr TransportLiteral transportLiteral(TransportOp O,uint32_t L,
                                            uint32_t H=0,uint8_t B=4) {
  return {O,L,H,B};
}
bool transportEncoding(const DecodedInstruction &I,TransportLiteral L) {
  return I.Inst.getOpcode()==unsigned(L.Op) && I.Encoding.size()==L.Bytes &&
    support::endian::read32le(I.Encoding.data())==L.Low &&
    (L.Bytes==4 || support::endian::read32le(I.Encoding.data()+4)==L.High);
}
