// Independent fixed matrix, not a parser for renderer output or serialized plans.
enum class CompleteProfile { One, OutputDiamond, ScratchDiamond, TwoTerminals, Maximum };
enum class CompleteEnd { Jump, Zero, Tail };
struct CompleteBlock {
  uint8_t Label;
  std::vector<Step> Steps;
  CompleteEnd End;
  size_t Zero = 0;
  size_t Nonzero = 0;
};
struct CompletePlan {
  CompleteProfile Kind;
  StringRef Name;
  uint8_t Scratch = 32, Output = 33;
  std::array<uint8_t,3> Inputs{{34,35,36}};
  std::vector<CompleteBlock> Blocks;
};
CompletePlan completePlan(StringRef Name) {
  const Step Out{Opcode::Mov,33,34,0};
  if (Name == "one")
    return {CompleteProfile::One, "one",32,33,{{34,35,36}},{{255,{Out},CompleteEnd::Tail}}};
  if (Name == "output_diamond")
    return {CompleteProfile::OutputDiamond,"output_diamond",32,33,{{34,35,36}},
      {{240,{},CompleteEnd::Zero,1,2},{17,{Out},CompleteEnd::Jump,3},
       {2,{{Opcode::Mov,33,35,0}},CompleteEnd::Jump,3},{4,{},CompleteEnd::Tail}}};
  if (Name == "scratch_diamond")
    return {CompleteProfile::ScratchDiamond,"scratch_diamond",32,33,{{34,35,36}},
      {{0,{},CompleteEnd::Zero,1,2},{93,{{Opcode::Mov,32,35,0}},CompleteEnd::Jump,3},
       {7,{{Opcode::Mov,32,34,0}},CompleteEnd::Jump,3},
       {250,{{Opcode::Mov,33,32,0}},CompleteEnd::Tail}}};
  if (Name == "two_terminals")
    return {CompleteProfile::TwoTerminals,"two_terminals",32,33,{{34,35,36}},
      {{41,{},CompleteEnd::Zero,1,2},{10,{Out},CompleteEnd::Tail},{3,{Out},CompleteEnd::Tail}}};
  if (Name == "maximum") {
    std::vector<Step> Repeated(14,Step{Opcode::Mov,63,63,0});
    return {CompleteProfile::Maximum,"maximum",60,63,{{8,9,10}},
      {{200,{{Opcode::Mov,63,8,0}},CompleteEnd::Jump,1},{0,{},CompleteEnd::Jump,2},
       {255,std::move(Repeated),CompleteEnd::Jump,3},{18,{},CompleteEnd::Jump,4},
       {1,{},CompleteEnd::Jump,5},{99,{},CompleteEnd::Jump,6},{44,{},CompleteEnd::Jump,7},
       {2,{{Opcode::Mov,63,8,0}},CompleteEnd::Tail}}};
  }
  fail("unknown complete-body fixed profile");
}
std::string completeLabel(size_t Ordinal) {
  return ".Lfe2o3_cb_${:uid}_b" + std::to_string(Ordinal);
}
std::string completeAssembly(const CompletePlan &Plan) {
  std::string S;
  for (size_t I=0;I<Plan.Blocks.size();++I) {
    const auto &B=Plan.Blocks[I];
    S += completeLabel(I)+":\n\t";
    for (const auto &StepValue:B.Steps) {
      S += mnemonic(StepValue.Op).str()+" v"+std::to_string(StepValue.Destination)+
        ", v"+std::to_string(StepValue.Source0);
      if (StepValue.Op!=Opcode::Mov) S += ", v"+std::to_string(StepValue.Source1);
      S += "\n\t";
    }
    if (B.End==CompleteEnd::Zero) {
      S += "s_cmp_eq_u32 s22, 0\n\ts_cbranch_scc1 "+completeLabel(B.Zero)+
        "\n\ts_branch "+completeLabel(B.Nonzero)+"\n\t";
    } else if (B.End==CompleteEnd::Jump) S += "s_branch "+completeLabel(B.Zero)+"\n\t";
    else S += "s_branch .Lfe2o3_cb_${:uid}_tail\n\t";
  }
  S += ".Lfe2o3_cb_${:uid}_tail:\n\t"
    "v_lshlrev_b64 v[2:3], 2, v[0:1]\n\t"
    "v_add_co_u32_e32 v2, vcc, s16, v2\n\t"
    "v_addc_co_u32_e32 v3, vcc, v4, v3, vcc\n\t"
    "v_cmp_gt_u64_e32 vcc, s[18:19], v[0:1]\n\t"
    "s_and_saveexec_b64 s[20:21], vcc\n\t"
    "global_store_dword v[2:3], v"+std::to_string(Plan.Output)+", off\n\t"
    "s_waitcnt vmcnt(0)\n\ts_mov_b64 exec, s[20:21]\n\ts_endpgm\n\t";
  require(S.size()<=4096,"independent assembly cap");
  return S;
}
std::string completeConstraints(const CompletePlan &P) {
  return "{s16},{v4},{s18},{s19},{v"+std::to_string(P.Inputs[0])+"},{v"+
    std::to_string(P.Inputs[1])+"},{v"+std::to_string(P.Inputs[2])+
    "},{v0},{v1},{s22},~{v2},~{v3},~{v"+std::to_string(P.Scratch)+"},~{v"+
    std::to_string(P.Output)+"},~{s20},~{s21},~{vcc},~{scc},~{memory}";
}
