// A separate exact gfx950 MC observer, not the protected gfx942 analyzer.
struct Row {
  std::string Opcode, Order; std::vector<std::string> Registers;
  std::vector<uint64_t> Immediates; unsigned Defs, Bytes, Flags;
  std::vector<std::string> Uses, Writes;
};
std::vector<Row> expectedRows() {
  return {
    {"S_LOAD_DWORDX2_IMM_vi","rrii",{"SGPR8_SGPR9","SGPR0_SGPR1"},{0,0},1,8,1,{},{}},
    {"S_WAITCNT_vi","i",{},{0},0,4,0,{},{}},
    {"S_MOV_B32_vi","ri",{"SGPR12"},{0x13579bdf},1,8,0,{},{}},
    {"S_MOV_B32_vi","ri",{"SGPR13"},{0x2468ace0},1,8,0,{},{}},
    {"V_MOV_B32_e32_vi","rr",{"VGPR2","VGPR0"},{},1,4,0,{"EXEC"},{}},
    {"V_MOV_B32_e32_vi","ri",{"VGPR3"},{0},1,4,0,{"EXEC"},{}},
    {"V_LSHLREV_B64_vi","rir",{"VGPR4_VGPR5","VGPR2_VGPR3"},{2},1,8,0,{"EXEC"},{}},
    {"V_MOV_B32_e32_vi","rr",{"VGPR6","SGPR9"},{},1,4,0,{"EXEC"},{}},
    {"V_ADD_CO_U32_e32_gfx9","rrr",{"VGPR4","SGPR8","VGPR4"},{},1,4,0,{"EXEC"},{"VCC"}},
    {"V_ADDC_CO_U32_e32_gfx9","rrr",{"VGPR5","VGPR6","VGPR5"},{},1,4,0,{"EXEC","VCC"},{"VCC"}},
    {"V_XOR_B32_e32_vi","rrr",{"VGPR8","SGPR12","VGPR0"},{},1,4,0,{"EXEC"},{}},
    {"V_XOR_B32_e32_vi","rrr",{"VGPR9","SGPR13","VGPR0"},{},1,4,0,{"EXEC"},{}},
    {"GLOBAL_STORE_DWORD_vi","rrii",{"VGPR4_VGPR5","VGPR8"},{0,0},0,8,2,{"EXEC"},{}},
    {"S_WAITCNT_vi","i",{},{0},0,4,0,{},{}},
    // SOPP_Real does not propagate the pseudo's isTrap flag. Exact opcode,
    // immediate and encoding below independently classify the sole debug trap.
    {"S_TRAP_vi","i",{},{3},0,4,0,{},{}},
    {"S_ENDPGM_vi","i",{},{0},0,4,12,{},{}}
  };
}
struct Mc {
  MCTargetOptions Options; Triple TargetTriple{"amdgcn-amd-amdhsa"};
  std::unique_ptr<MCRegisterInfo> Registers; std::unique_ptr<MCInstrInfo> Instructions;
  std::unique_ptr<MCSubtargetInfo> Subtarget; std::unique_ptr<MCAsmInfo> AsmInfo;
  std::unique_ptr<MCContext> Context; std::unique_ptr<MCDisassembler> Disassembler;
  std::unique_ptr<MCCodeEmitter> Encoder;
  Mc() {
    std::string Diagnostic;
    const Target *T = TargetRegistry::lookupTarget("amdgcn", TargetTriple, Diagnostic);
    require(T != nullptr, Diagnostic);
    Registers.reset(T->createMCRegInfo(TargetTriple)); Instructions.reset(T->createMCInstrInfo());
    Subtarget.reset(T->createMCSubtargetInfo(TargetTriple,"gfx950","-xnack,-wavefrontsize32,+wavefrontsize64"));
    require(Registers && Instructions && Subtarget, "MC target tables");
    AsmInfo.reset(T->createMCAsmInfo(*Registers,TargetTriple,Options));
    require(AsmInfo != nullptr, "MC asm table");
    Context = std::make_unique<MCContext>(TargetTriple,AsmInfo.get(),Registers.get(),Subtarget.get(),nullptr,&Options);
    Disassembler.reset(T->createMCDisassembler(*Subtarget,*Context));
    Encoder.reset(T->createMCCodeEmitter(*Instructions,*Context));
    require(Disassembler && Encoder, "MC decoder/encoder");
  }
  std::vector<std::string> names(ArrayRef<MCPhysReg> Values) const {
    std::vector<std::string> Result;
    if (Values.size()>16) return {"<over-bound>"};
    for (auto V:Values) Result.emplace_back(Registers->getName(V));
    llvm::sort(Result); return Result;
  }
};
struct TraceRow { Row Actual; uint64_t Offset=0; std::vector<uint8_t> Encoding; MCInst Inst; };
Expected<std::vector<TraceRow>> decode(Mc &M, ArrayRef<uint8_t> Code, uint64_t Address) {
  if (Code.empty() || Code.size()>512) return refusal("entry byte bound");
  std::vector<TraceRow> Rows; size_t At=0;
  while (At<Code.size()) {
    if (Rows.size()>=64) return refusal("entry instruction cap");
    MCInst I; uint64_t Size=0;
    if (M.Disassembler->getInstruction(I,Size,Code.drop_front(At),Address+At,nulls())!=MCDisassembler::Success ||
        !Size || Size>16 || Size>Code.size()-At || I.size()>16) return refusal("strict MC decode");
    const auto &D=M.Instructions->get(I.getOpcode());
    if (D.getNumDefs()>I.size() || D.isBranch() || D.isCall() || D.isIndirectBranch())
      return refusal("unexpected MC branch/call");
    Row R{M.Instructions->getName(I.getOpcode()).str(),"",{},{},D.getNumDefs(),static_cast<unsigned>(Size),
      static_cast<unsigned>((D.mayLoad()?1:0)|(D.mayStore()?2:0)|(D.isTerminator()?4:0)|
          (D.isBarrier()?8:0)|(D.isPredicable()?16:0)|(D.isTrap()?32:0)),
      M.names(D.implicit_uses()),M.names(D.implicit_defs())};
    for (unsigned N=0;N<I.size();++N) {
      if (D.getOperandConstraint(N,MCOI::TIED_TO)>=0) return refusal("tied MC operand");
      const auto &O=I.getOperand(N);
      if (O.isReg() && O.getReg()!=0) { R.Order+='r'; R.Registers.emplace_back(M.Registers->getName(O.getReg())); }
      else if (O.isImm()) { R.Order+='i'; R.Immediates.push_back(static_cast<uint64_t>(O.getImm())); }
      else return refusal("unsupported MC operand");
    }
    Rows.push_back({std::move(R),At,std::vector<uint8_t>(Code.begin()+At,Code.begin()+At+Size),std::move(I)});
    At+=Size;
  }
  return Rows;
}
bool exactRows(Mc &M,const std::vector<TraceRow> &Actual) {
  const auto Expected=expectedRows(); if (Actual.size()!=Expected.size()) return false;
  uint64_t Cursor=0;
  for (size_t I=0;I<Expected.size();++I) {
    const auto &A=Actual[I].Actual; const auto &E=Expected[I];
    if (Actual[I].Offset!=Cursor || Actual[I].Encoding.size()!=E.Bytes ||
        A.Opcode!=E.Opcode || A.Order!=E.Order || A.Registers!=E.Registers ||
        A.Immediates!=E.Immediates || A.Defs!=E.Defs || A.Bytes!=E.Bytes ||
        A.Flags!=E.Flags || A.Uses!=E.Uses || A.Writes!=E.Writes) return false;
    // Encode only the already closed supported row. Unknown opcodes are never
    // fed to the emitter. Reserved/ignored bits cannot survive this byte join.
    SmallVector<char,16> Encoded; SmallVector<MCFixup,2> Fixups;
    M.Encoder->encodeInstruction(Actual[I].Inst,Encoded,Fixups,*M.Subtarget);
    if (!Fixups.empty() || Encoded.size()!=Actual[I].Encoding.size() ||
        !std::equal(Encoded.begin(),Encoded.end(),Actual[I].Encoding.begin(),
          [](char L,uint8_t R){return static_cast<uint8_t>(L)==R;})) return false;
    Cursor+=E.Bytes;
  }
  return Cursor==84 && support::endian::read32le(Actual[1].Encoding.data())==0xbf8c0000 &&
    support::endian::read32le(Actual[13].Encoding.data())==0xbf8c0000 &&
    support::endian::read32le(Actual[14].Encoding.data())==0xbf920003 &&
    support::endian::read32le(Actual[15].Encoding.data())==0xbf810000;
}
