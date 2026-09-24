// Exact finite copy/preserve/edit O0 entry prefix. Opcode identities come
// from the independent measured MC table; operands and bytes are separately
// checked. Relocated layouts outside this profile are an explicit refusal.
struct CallerExpected {
  CallerOp Operation;
  uint32_t Low,High;
  uint8_t Bytes;
  std::array<TransportExpectedOperand,4> Operands;
};
constexpr CallerExpected CX(CallerOp Op,uint32_t Low,uint32_t High,uint8_t Bytes,
    TransportExpectedOperand A={},TransportExpectedOperand B={},
    TransportExpectedOperand C={},TransportExpectedOperand D={}) {
  return {Op,Low,High,Bytes,{A,B,C,D}};
}
constexpr std::array<CallerExpected,45> TransportCallerPrefix{{
 CX(CallerOp::Move32,0xbea00080,0,4,TS(32),TM(0)),
 CX(CallerOp::Move32,0xbe8e000a,0,4,TS(14),TS(10)),
 CX(CallerOp::Move32,0xbe8d0009,0,4,TS(13),TS(9)),
 CX(CallerOp::Move32,0xbe8c0008,0,4,TS(12),TS(8)),
 CX(CallerOp::WriteLane,0xd28a0004,0x0001000c,8,TV(4),TS(12),TM(0),TV(4)),
 CX(CallerOp::Move64,0xbe8a0106,0,4,TS(10,64),TS(6,64)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010204,8,TV(4),TS(4),TM(1),TV(4)),
 CX(CallerOp::Nop,0xbf800001,0,4,TM(1)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010405,8,TV(4),TS(5),TM(2),TV(4)),
 CX(CallerOp::Move64,0xbe860102,0,4,TS(6,64),TS(2,64)),
 CX(CallerOp::Move64,0xbe840100,0,4,TS(4,64),TS(0,64)),
 CX(CallerOp::ReadLane,0xd2890000,0x00010304,8,TS(0),TV(4),TM(1)),
 CX(CallerOp::ReadLane,0xd2890001,0x00010504,8,TS(1),TV(4),TM(2)),
 CX(CallerOp::Move,0x7e3e0300,0,4,TV(31),TV(0)),
 CX(CallerOp::AccWrite,0xd3d94001,0x1800011f,8,TA(1),TV(31)),
 CX(CallerOp::Load2,0xc0060480,0,8,TS(18,64),TS(0,64),TM(0),TM(0)),
 CX(CallerOp::Load2,0xc0060200,8,8,TS(8,64),TS(0,64),TM(8),TM(0)),
 CX(CallerOp::Load1,0xc0020400,16,8,TS(16),TS(0,64),TM(16),TM(0)),
 CX(CallerOp::Load1,0xc00200c0,20,8,TS(3),TS(0,64),TM(20),TM(0)),
 CX(CallerOp::Load1,0xc0020080,24,8,TS(2),TS(0,64),TM(24),TM(0)),
 CX(CallerOp::Wait,0xbf8cc07f,0,4,TM(49279)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010612,8,TV(4),TS(18),TM(3),TV(4)),
 CX(CallerOp::Nop,0xbf800001,0,4,TM(1)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010813,8,TV(4),TS(19),TM(4),TV(4)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010a08,8,TV(4),TS(8),TM(5),TV(4)),
 CX(CallerOp::Nop,0xbf800001,0,4,TM(1)),
 CX(CallerOp::WriteLane,0xd28a0004,0x00010c09,8,TV(4),TS(9),TM(6),TV(4)),
 CX(CallerOp::SaveExec,0xbe9421c1,0,4,TS(20,64),TM(-1)),
 CX(CallerOp::AccWrite,0xd3d94002,0x18000104,8,TA(2),TV(4)),
 CX(CallerOp::Move64,0xbefe0114,0,4,TC(126),TS(20,64)),
 CX(CallerOp::Move64,0xbe9201a0,0,4,TS(18,64),TM(32)),
 CX(CallerOp::Move32,0xbe880000,0,4,TS(8),TS(0)),
 CX(CallerOp::Move32,0xbe800001,0,4,TS(0),TS(1)),
 CX(CallerOp::Move32,0xbe890012,0,4,TS(9),TS(18)),
 CX(CallerOp::Move32,0xbe810013,0,4,TS(1),TS(19)),
 CX(CallerOp::Add,0x80080908,0,4,TS(8),TS(8),TS(9)),
 CX(CallerOp::AddCarry,0x82000100,0,4,TS(0),TS(0),TS(1)),
 CX(CallerOp::Move32,0xbe890000,0,4,TS(9),TS(0)),
 CX(CallerOp::GetPc,0xbe801c00,0,4,TS(0,64)),
 CX(CallerOp::Add,0x8000ff00,0xfffffe24,8,TS(0),TS(0),TM(4294966820LL)),
 CX(CallerOp::AddCarry,0x8201ff01,0xffffffff,8,TS(1),TS(1),TM(4294967295LL)),
 CX(CallerOp::Move,0x7e000210,0,4,TV(0),TS(16)),
 CX(CallerOp::Move,0x7e020203,0,4,TV(1),TS(3)),
 CX(CallerOp::Move,0x7e040202,0,4,TV(2),TS(2)),
 CX(CallerOp::Call,0xbe9e1e00,0,4,TS(30,64),TS(0,64))
}};
static_assert(TransportCallerPrefix.size()+TransportHelper.size()<=256);
bool transportCallerEncoding(const DecodedInstruction &I,const CallerExpected &E) {
  const auto *S=transportCallerShape(I.Inst.getOpcode());
  return S && S->Operation==E.Operation && I.Encoding.size()==E.Bytes &&
    support::endian::read32le(I.Encoding.data())==E.Low &&
    (E.Bytes==4 || support::endian::read32le(I.Encoding.data()+4)==E.High);
}
