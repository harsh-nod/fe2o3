// All identifiers/units below come from the actual borrowed MC tables.
// No printed register/opcode name is queried by this checker.
TransportRegister *TransportAttempt::find(unsigned Id) {
  size_t L=0,R=RegisterCount;
  while(L<R) {const size_t M=L+(R-L)/2;
    if(W.Registers[M].Id<Id)L=M+1;else R=M;
  }
  return L<RegisterCount && W.Registers[L].Id==Id ? &W.Registers[L] : nullptr;
}
const TransportRegister *TransportAttempt::find(unsigned Id) const {
  size_t L=0,R=RegisterCount;
  while(L<R) {const size_t M=L+(R-L)/2;
    if(W.Registers[M].Id<Id)L=M+1;else R=M;
  }
  return L<RegisterCount && W.Registers[L].Id==Id ? &W.Registers[L] : nullptr;
}
bool TransportAttempt::tables(const MCRegisterInfo &MRI) {
  if(ClassCount) return true;
  if(MRI.getNumRegs()>UINT16_MAX || MRI.getNumRegUnits()>16384 ||
     MRI.getNumRegClasses()>4096 || MRI.getNumRegUnits()==0)
    return fail(ct::Refusal::Table);
  for(const auto &C:MRI.regclasses()) {
    if(C.getID()>=MRI.getNumRegClasses() || C.getID()>UINT16_MAX)
      return fail(ct::Refusal::Table);
    if(C.getSizeInBits()!=32) continue;
    if(ClassCount==128) return fail(ct::Refusal::Table);
    W.Whole32Classes[ClassCount++]=uint16_t(C.getID());
  }
  return ClassCount!=0 || fail(ct::Refusal::Table);
}
bool TransportAttempt::cache(unsigned Id,const MCRegisterInfo &MRI) {
  if(find(Id))return true;
  if(Id==0 || Id>=MRI.getNumRegs() || Id>UINT16_MAX || RegisterCount==128)
    return fail(ct::Refusal::Register);
  TransportRegister R;
  R.Id=uint16_t(Id);R.Tag=MRI.getEncodingValue(Id);
  for(MCRegUnitIterator U(Id,&MRI);U.isValid();++U) {
    if(R.UnitCount==16 || *U>=MRI.getNumRegUnits() || *U>=16384 ||
       (R.UnitCount && R.Units[R.UnitCount-1]>=*U))
      return fail(ct::Refusal::Register);
    R.Units[R.UnitCount++]=uint16_t(*U);
  }
  if(!R.UnitCount)return fail(ct::Refusal::Register);
  bool Whole=false;
  for(size_t I=0;I<ClassCount;++I)
    Whole |= MRI.getRegClass(W.Whole32Classes[I]).contains(Id);
  if(Whole) {
    if((R.Tag & ~uint16_t(0x0fff))!=0 || (R.Tag&0xc00)==0xc00 ||
       R.UnitCount>4 || WordCount==64)
      return fail(ct::Refusal::Register);
    R.Index=R.Tag&0x3ff;
    R.Bank=(R.Tag&0x800)?TransportBank::Accumulator:
           (R.Tag&0x400)?TransportBank::Vector:TransportBank::Scalar;
    // Exact table IDs are joined to actual implicit descriptors and units.
    // They do not manufacture ordinary SGPR facts for control aliases.
    if(Id==3 || Id==4 || Id==46 || Id==47) R.Bank=TransportBank::Control;
    if((R.Bank==TransportBank::Scalar && R.Index>=112) ||
       ((R.Bank==TransportBank::Vector || R.Bank==TransportBank::Accumulator) &&
        R.Index>=256)) return fail(ct::Refusal::Register);
    R.Width=32;R.WordCount=1;R.Words[0]=R.Id;R.Slot=uint8_t(WordCount);
    for(size_t I=0;I<R.UnitCount;++I)
      if(W.UnitSlots[R.Units[I]]!=TransportNoSlot)
        return fail(ct::Refusal::Alias);
    for(size_t I=0;I<R.UnitCount;++I) W.UnitSlots[R.Units[I]]=WordCount;
    W.Words[WordCount++]={R.Id,R.Index,R.Bank,{}};
  }
  size_t At=RegisterCount;
  while(At && W.Registers[At-1].Id>Id) {
    W.Registers[At]=W.Registers[At-1];--At;
  }
  W.Registers[At]=R;++RegisterCount;
  return true;
}
bool TransportAttempt::words(unsigned Id,unsigned Width,const MCRegisterInfo &MRI) {
  if(Width==0 || Width>256 || Width%32!=0 || !cache(Id,MRI))
    return fail(ct::Refusal::Register);
  if(const auto *R=find(Id);R->Width) return R->Width==Width || fail(ct::Refusal::Register);
  if(TupleCount==16)return fail(ct::Refusal::Register);
  ++TupleCount;
  std::array<uint16_t,8> Parts{};size_t Count=0,Visits=0;
  for(MCRegister Sub:MRI.subregs(Id)) {
    if(++Visits>64 || !cache(Sub.id(),MRI))return fail(ct::Refusal::Register);
    const auto *R=find(Sub.id());
    if(R->Width!=32)continue;
    if(Count==8)return fail(ct::Refusal::Register);
    // At most eight entries. Keep architectural target-tag order, not ID order.
    size_t At=Count;
    while(At && find(Parts[At-1])->Tag>R->Tag) {Parts[At]=Parts[At-1];--At;}
    Parts[At]=R->Id;++Count;
  }
  if(Count!=Width/32)return fail(ct::Refusal::Register);
  const auto Parent=*find(Id);
  std::array<bool,16> Covered{};
  const auto First=*find(Parts[0]);
  if((Parent.Tag&0x0fff)!=First.Tag || (Parent.Tag&0xf000)!=0)
    return fail(ct::Refusal::Register);
  for(size_t I=0;I<Count;++I) {
    const auto *R=find(Parts[I]);
    if(R->Bank!=First.Bank || R->Index!=First.Index+I ||
       (I && Parts[I]==Parts[I-1]))return fail(ct::Refusal::Register);
    for(size_t U=0;U<R->UnitCount;++U) {
      size_t P=0;while(P<Parent.UnitCount && Parent.Units[P]!=R->Units[U])++P;
      if(P==Parent.UnitCount || Covered[P])return fail(ct::Refusal::Register);
      Covered[P]=true;
    }
  }
  for(size_t P=0;P<Parent.UnitCount;++P)
    if(!Covered[P])return fail(ct::Refusal::Register);
  auto *R=find(Id);R->Width=uint16_t(Width);R->WordCount=uint8_t(Count);
  R->Words=Parts;R->Bank=First.Bank;R->Index=First.Index;
  return true;
}
bool TransportAttempt::role(unsigned Id,TransportBank B,unsigned Index,unsigned Width) const {
  const auto *R=find(Id);
  return R && R->Width==Width && R->Bank==B && R->Index==Index &&
         R->WordCount==Width/32;
}
bool TransportAttempt::kill(unsigned Id,TransportState &S) {
  const auto *R=find(Id);if(!R)return fail(ct::Refusal::Register);
  for(size_t I=0;I<R->UnitCount;++I) {
    const auto Slot=W.UnitSlots[R->Units[I]];
    if(Slot==TransportNoSlot)continue;
    if(Slot>=WordCount)return fail(ct::Refusal::Register);
    if(S.Facts[Slot].Ready==TransportReady::Pending)
      return fail(ct::Refusal::Pending);
    S.Facts[Slot]={};
  }
  return true;
}
TransportFact TransportAttempt::fact(unsigned Id,const TransportState &S) const {
  const auto *R=find(Id);
  if(!R || R->Width!=32 || R->Slot>=WordCount)return {};
  return S.Facts[R->Slot];
}
bool TransportAttempt::set(unsigned Id,TransportFact F,TransportState &S) {
  const auto *R=find(Id);
  if(!R || R->Width!=32 || R->Slot>=WordCount)
    return fail(ct::Refusal::Register);
  S.Facts[R->Slot]=F;return true;
}
