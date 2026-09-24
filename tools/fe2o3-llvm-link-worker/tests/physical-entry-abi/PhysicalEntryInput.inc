// Body spelling is literal. Expected native rows are independently specified.
constexpr StringLiteral PhysicalClobbers =
    "~{s8},~{s9},~{s10},~{s11},~{s12},~{s13},~{s14},~{s15},~{s16},"
    "~{s18},~{s19},~{v2},~{v3},~{v4},~{v6},~{v7},~{v8},~{vcc},~{scc},~{memory}";
constexpr StringLiteral PhysicalSetup =
    "s_load_dwordx2 s[8:9], s[0:1], 0\n\t"
    "s_load_dwordx2 s[10:11], s[0:1], 8\n\t"
    "s_load_dword s12, s[0:1], 16\n\t"
    "s_load_dword s13, s[0:1], 20\n\t"
    "s_load_dword s14, s[0:1], 24\n\t"
    "s_load_dword s15, s[0:1], 28\n\t"
    "s_waitcnt lgkmcnt(0)\n\t"
    "s_lshl_b32 s16, s2, 6\n\t"
    "v_add_u32_e32 v2, s16, v0\n\t"
    "v_mov_b32_e32 v3, 0\n\t"
    "v_mov_b32_e32 v4, s9\n\t";
constexpr StringLiteral PhysicalCopy = "v_mov_b32_e32 v8, s12\n\t";
constexpr StringLiteral PhysicalSelect =
    "s_cmp_eq_u32 s15, 0\n\t"
    "s_cbranch_scc1 .Lphysical_zero\n\t"
    "v_mov_b32_e32 v8, s13\n\t"
    "s_branch .Lphysical_join\n\t"
    ".Lphysical_zero:\n\t"
    "v_mov_b32_e32 v8, s12\n\t"
    ".Lphysical_join:\n\t";
constexpr StringLiteral PhysicalTail =
    "v_lshlrev_b64 v[6:7], 2, v[2:3]\n\t"
    "v_add_co_u32_e32 v6, vcc, s8, v6\n\t"
    "v_addc_co_u32_e32 v7, vcc, v4, v7, vcc\n\t"
    "v_cmp_gt_u64_e32 vcc, s[10:11], v[2:3]\n\t"
    "s_and_saveexec_b64 s[18:19], vcc\n\t"
    "global_store_dword v[6:7], v8, off\n\t"
    "s_waitcnt vmcnt(0)\n\t"
    "s_mov_b64 exec, s[18:19]\n\t"
    "s_endpgm";
// Supported absence attributes, pinned AMDGPUAttributes.def. No implicitarg
// size override, inreg preload, positive guessed attribute or IR argument use.
constexpr std::array<StringLiteral, 14> PhysicalAbsenceAttributes{{
    "amdgpu-no-dispatch-ptr", "amdgpu-no-queue-ptr", "amdgpu-no-dispatch-id",
    "amdgpu-no-hostcall-ptr", "amdgpu-no-multigrid-sync-arg", "amdgpu-no-heap-ptr",
    "amdgpu-no-default-queue", "amdgpu-no-completion-action",
    "amdgpu-no-workgroup-id-y", "amdgpu-no-workgroup-id-z",
    "amdgpu-no-cluster-id-y", "amdgpu-no-cluster-id-z",
    "amdgpu-no-workitem-id-y", "amdgpu-no-workitem-id-z"}};

Input physicalInput(bool Select, bool MissingLaunch = false, bool WrongCpu = false) {
  LLVMContext Context; Module M("inert-fixed-physical-entry", Context);
  auto Machine = createMachine();
  M.setTargetTriple(Triple(TripleName)); M.setDataLayout(Machine->createDataLayout());
  M.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);
  auto *I32 = Type::getInt32Ty(Context); auto *I64 = Type::getInt64Ty(Context);
  auto *Signature = FunctionType::get(Type::getVoidTy(Context),
      {PointerType::get(Context, 1), I64, I32, I32, I32, I32}, false);
  auto *Kernel = Function::Create(Signature, GlobalValue::ExternalLinkage, PhysicalName, M);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr(Attribute::NoUnwind);
  // Deliberately ordinary: pinned naked lowering skips required entry SGPR accounting.
  require(!Kernel->hasFnAttribute(Attribute::Naked), "ordinary shell required");
  Kernel->addFnAttr("target-cpu", WrongCpu ? "gfx950" : "gfx942");
  Kernel->addFnAttr("target-features", "-xnack,-wavefrontsize32,+wavefrontsize64");
  Kernel->addFnAttr("amdgpu-flat-work-group-size", "64,64");
  Kernel->addFnAttr("amdgpu-max-num-workgroups", "2,1,1");
  for (auto Name : PhysicalAbsenceAttributes) Kernel->addFnAttr(Name);
  const std::array<StringLiteral, 6> Names{{"data", "length", "a", "b", "c", "selector"}};
  size_t N = 0; for (auto &Argument : Kernel->args()) Argument.setName(Names[N++]);
  if (!MissingLaunch) {
    Metadata *WG[] = {ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1)), ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
    Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, WG));
  }
  const std::string Body = PhysicalSetup.str() +
      (Select ? PhysicalSelect.str() : PhysicalCopy.str()) + PhysicalTail.str();
  auto *AssemblyType = FunctionType::get(Type::getVoidTy(Context), false);
  IRBuilder<> Builder(BasicBlock::Create(Context, "entry", Kernel));
  auto *Assembly = InlineAsm::get(AssemblyType, Body, PhysicalClobbers, true);
  Builder.CreateCall(AssemblyType, Assembly, {}); Builder.CreateUnreachable();
  require(Kernel->size() == 1 && Kernel->front().size() == 2, "one asm plus unreachable");
  for (const auto &Argument : Kernel->args())
    require(Argument.use_empty() && !Argument.hasInRegAttr(), "physical inputs must have no LLVM argument uses/preloads");
  require(M.size() == 1 && M.getModuleInlineAsm().empty(), "no helper, intrinsic or module-asm entry");
  std::string Diagnostic; raw_string_ostream Errors(Diagnostic);
  require(!verifyModule(M, &Errors), "physical entry LLVM verification");
  std::string Text; raw_string_ostream Out(Text); M.print(Out, nullptr); Out.flush();
  require(!Text.empty() && Text.size() <= 16 * 1024, "physical LLVM cap");
  Input Value; Value.Kind = InputKind::LlvmTextIr;
  Value.Bytes.assign(Text.begin(), Text.end()); Value.Digest = SHA256::hash(Value.Bytes);
  return Value;
}
