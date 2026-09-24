// Fixed architectural roster, independently written from the approved profile.
// It is not caller supplied and does not decode machine bytes a second time.
struct TransportExpectedOperand {
  bool Register=false;TransportBank Bank=TransportBank::Scalar;
  uint16_t Index=0,Width=0;int64_t Immediate=0;
};
constexpr TransportExpectedOperand TS(unsigned I,unsigned W=32) {
  return {true,TransportBank::Scalar,uint16_t(I),uint16_t(W),0};
}
constexpr TransportExpectedOperand TV(unsigned I,unsigned W=32) {
  return {true,TransportBank::Vector,uint16_t(I),uint16_t(W),0};
}
constexpr TransportExpectedOperand TA(unsigned I) {
  return {true,TransportBank::Accumulator,uint16_t(I),32,0};
}
constexpr TransportExpectedOperand TC(unsigned I,unsigned W=64) {
  return {true,TransportBank::Control,uint16_t(I),uint16_t(W),0};
}
constexpr TransportExpectedOperand TM(int64_t I) {return {false,{},0,0,I};}
struct TransportExpected {
  TransportLiteral Literal;
  std::array<TransportExpectedOperand,4> Operands{};
};
constexpr TransportExpected TX(TransportLiteral L,TransportExpectedOperand A={},
  TransportExpectedOperand B={},TransportExpectedOperand C={},
  TransportExpectedOperand D={}) {return {L,{A,B,C,D}};}
constexpr auto TWait=TX(transportLiteral(TransportOp::Wait,0xbf8c0000),TM(0));
constexpr auto TNop=TX(transportLiteral(TransportOp::Nop,0xbf800000),TM(0));
constexpr auto TReturn=TX(transportLiteral(TransportOp::Return,0xbe801d1e),TS(30,64));
constexpr auto TXor=TX(transportLiteral(TransportOp::Xor,0x2a10170a),TV(8),TV(10),TV(11));
constexpr auto TAnd=TX(transportLiteral(TransportOp::And,0x26121908),TV(9),TV(8),TV(12));
constexpr auto TEdit=TX(transportLiteral(TransportOp::Move,0x7e12030c),TV(9),TV(12));
constexpr std::array<TransportExpected,10> TransportHelper{{
 TWait,
 TX(transportLiteral(TransportOp::Move,0x7e180302),TV(12),TV(2)),
 TX(transportLiteral(TransportOp::Move,0x7e160301),TV(11),TV(1)),
 TX(transportLiteral(TransportOp::Move,0x7e140300),TV(10),TV(0)),
 TXor,TAnd,TNop,
 TX(transportLiteral(TransportOp::AccWrite,0xd3d94000,0x18000109,8),TA(0),TV(9)),
 TX(transportLiteral(TransportOp::AccRead,0xd3d84000,0x18000100,8),TV(0),TA(0)),
 TReturn
}};
constexpr std::array<TransportExpected,19> TransportRoot{{
 TX(transportLiteral(TransportOp::Load8,0xc00e0100,0,8),TS(4,256),TS(0,64),TM(0),TM(0)),
 TX(transportLiteral(TransportOp::ScalarMove,0xbe830080),TS(3),TM(0)),
 TX(transportLiteral(TransportOp::ScalarShift,0x8e808602),TS(0,64),TS(2,64),TM(6)),
 TX(transportLiteral(TransportOp::Or,0x28000000),TV(0),TS(0),TV(0)),
 TX(transportLiteral(TransportOp::Move,0x7e020201),TV(1),TS(1)),
 TX(transportLiteral(TransportOp::Wait,0xbf8cc07f),TM(49279)),
 TX(transportLiteral(TransportOp::Compare,0x7dd80006),TS(6,64),TV(0,64)),
 TX(transportLiteral(TransportOp::Move,0x7e140208),TV(10),TS(8)),
 TX(transportLiteral(TransportOp::Move,0x7e160209),TV(11),TS(9)),
 TX(transportLiteral(TransportOp::Move,0x7e18020a),TV(12),TS(10)),
 TXor,TAnd,
 TX(transportLiteral(TransportOp::SaveExec,0xbe80206a),TS(0,64),TC(106)),
 TX(transportLiteral(TransportOp::Branch,0xbf880006),TM(6)),
 TX(transportLiteral(TransportOp::Move,0x7e040204),TV(2),TS(4)),
 TX(transportLiteral(TransportOp::Move,0x7e060205),TV(3),TS(5)),
 TX(transportLiteral(TransportOp::Address,0xd2080000,0x04090500,8),
    TV(0,64),TV(0,64),TM(2),TV(2,64)),
 TX(transportLiteral(TransportOp::Store,0xdc708000,0x007f0900,8),
    TV(0,64),TV(9),TM(0),TM(0)),
 TX(transportLiteral(TransportOp::End,0xbf810000),TM(0))
}};
TransportExpected transportExpected(ct::Profile P,ct::Program Program,size_t I) {
  const bool Edit=Program==ct::Program::MoveInput2;
  if(P==ct::Profile::ConditionalHelperO0) {
    if(Edit && I==4)return TEdit;
    return TransportHelper[I+(Edit && I>=5?1:0)];
  }
  if(Edit && I==10)return TEdit;
  return TransportRoot[I+(Edit && I>=11?1:0)];
}
