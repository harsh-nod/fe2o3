// Ordinary Function shell; literal body and independent MC table are separate.
constexpr StringLiteral Body =
    "s_load_dwordx2 s[8:9], s[0:1], 0\n\t"
    "s_waitcnt 0\n\t"
    "s_mov_b32 s12, 0x13579bdf\n\t"
    "s_mov_b32 s13, 0x2468ace0\n\t"
    "v_mov_b32_e32 v2, v0\n\t"
    "v_mov_b32_e32 v3, 0\n\t"
    "v_lshlrev_b64 v[4:5], 2, v[2:3]\n\t"
    "v_mov_b32_e32 v6, s9\n\t"
    "v_add_co_u32_e32 v4, vcc, s8, v4\n\t"
    "v_addc_co_u32_e32 v5, vcc, v6, v5, vcc\n\t"
    "v_xor_b32_e32 v8, s12, v0\n\t"
    "v_xor_b32_e32 v9, s13, v0\n\t"
    "global_store_dword v[4:5], v8, off\n\t"
    "s_waitcnt 0\n\t"
    "s_trap 3\n\t"
    "s_endpgm";
constexpr StringLiteral Clobbers =
    "~{s8},~{s9},~{s12},~{s13},~{v2},~{v3},~{v4},~{v5},~{v6},"
    "~{v8},~{v9},~{vcc},~{memory}";
constexpr std::array<StringLiteral, 14> AbsenceAttributes{{
    "amdgpu-no-dispatch-ptr", "amdgpu-no-queue-ptr", "amdgpu-no-dispatch-id",
    "amdgpu-no-hostcall-ptr", "amdgpu-no-multigrid-sync-arg", "amdgpu-no-heap-ptr",
    "amdgpu-no-default-queue", "amdgpu-no-completion-action",
    "amdgpu-no-workgroup-id-y", "amdgpu-no-workgroup-id-z",
    "amdgpu-no-cluster-id-y", "amdgpu-no-cluster-id-z",
    "amdgpu-no-workitem-id-y", "amdgpu-no-workitem-id-z"}};
void initialize() {
  LLVMInitializeAMDGPUTargetInfo(); LLVMInitializeAMDGPUTarget();
  LLVMInitializeAMDGPUTargetMC(); LLVMInitializeAMDGPUAsmPrinter();
  LLVMInitializeAMDGPUAsmParser(); LLVMInitializeAMDGPUDisassembler();
}
Input fixedInput(bool MissingLaunch = false, bool WrongCpu = false) {
  LLVMContext Context; Module M("inert-gfx950-one-stop", Context);
  Triple TripleValue("amdgcn-amd-amdhsa"); std::string Diagnostic;
  const Target *T = TargetRegistry::lookupTarget("amdgcn", TripleValue, Diagnostic);
  require(T != nullptr, Diagnostic);
  TargetOptions Options;
  std::unique_ptr<TargetMachine> Machine(T->createTargetMachine(
      TripleValue, "gfx950", "-xnack,-wavefrontsize32,+wavefrontsize64",
      Options, Reloc::PIC_, CodeModel::Small, CodeGenOptLevel::None));
  require(Machine != nullptr, "gfx950 TargetMachine unavailable");
  M.setTargetTriple(TripleValue); M.setDataLayout(Machine->createDataLayout());
  M.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);
  auto *Signature = FunctionType::get(Type::getVoidTy(Context),
      {PointerType::get(Context, 1)}, false);
  auto *K = Function::Create(Signature, GlobalValue::ExternalLinkage, Name, M);
  K->setCallingConv(CallingConv::AMDGPU_KERNEL); K->addFnAttr(Attribute::NoUnwind);
  K->addFnAttr("target-cpu", WrongCpu ? "gfx942" : "gfx950");
  K->addFnAttr("target-features", "-xnack,-wavefrontsize32,+wavefrontsize64");
  K->addFnAttr("amdgpu-flat-work-group-size", "64,64");
  K->addFnAttr("amdgpu-max-num-workgroups", "1,1,1");
  for (auto A : AbsenceAttributes) K->addFnAttr(A);
  K->getArg(0)->setName("out");
  auto *I32 = Type::getInt32Ty(Context);
  if (!MissingLaunch) {
    Metadata *WG[] = {ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
      ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
      ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
    K->setMetadata("reqd_work_group_size", MDNode::get(Context, WG));
  }
  auto *AsmType = FunctionType::get(Type::getVoidTy(Context), false);
  IRBuilder<> Builder(BasicBlock::Create(Context, "entry", K));
  Builder.CreateCall(AsmType, InlineAsm::get(AsmType, Body, Clobbers, true), {});
  Builder.CreateUnreachable();
  require(K->size() == 1 && K->front().size() == 2 &&
      K->getArg(0)->use_empty() && !K->getArg(0)->hasInRegAttr() &&
      !K->hasFnAttribute(Attribute::Naked) && M.size() == 1 &&
      M.getModuleInlineAsm().empty(), "closed ordinary LLVM shell");
  std::string ErrorText; raw_string_ostream Errors(ErrorText);
  require(!verifyModule(M, &Errors), "LLVM verifier refused fixed source");
  std::string Text; raw_string_ostream Out(Text); M.print(Out, nullptr); Out.flush();
  require(!Text.empty() && Text.size() <= 16384, "LLVM input cap");
  Input Value; Value.Kind = InputKind::LlvmTextIr;
  Value.Bytes.assign(Text.begin(), Text.end()); Value.Digest = SHA256::hash(Value.Bytes);
  return Value;
}
Request requestFor(const Input &Value, OptimizationLevel Level) {
  Request R; R.RequestId.fill(0x41); R.Identity.fill(0x42);
  R.Protocol = ProtocolVersion::V2; R.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  R.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID; R.WorkerExecutableDigest.fill(0x51);
  R.WorkerExecutableBytes = 4096; R.CompilerEnvelopeIdentity.fill(0x62);
  R.Target = "gfx950:xnack-"; R.CodeObjectVersion = 6;
  R.LinkOptions = {Level, false, true}; R.CompilerModule = Value; R.Inputs = {Value};
  R.RequiredSymbols = {Name.str(), Name.str() + ".kd"};
  R.ExpectedDefinedSymbols = R.RequiredSymbols; R.ExportSymbols = {Name.str()};
  R.FinalSymbols = R.RequiredSymbols; R.MaxOutputBytes = ArtifactCap;
  return R;
}
