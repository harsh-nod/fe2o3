// Test-only typed parser/dataflow check mechanically adapted from SourceBodyAbiCandidate.cpp.
// Actual source custody is absent; complete plan is an independent fixed roster.
Input readCompleteLlvm(StringRef Path) {
  require(Path.starts_with("/") && Path.size() <= 4096 &&
              llvm::none_of(Path,
                            [](char Byte) {
                              return static_cast<unsigned char>(Byte) < 32 ||
                                     Byte == 127;
                            }),
          "LLVM input requires a bounded absolute path");
  int File =
      open(Path.str().c_str(), O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
  require(File >= 0, "cannot open non-symlink LLVM input");
  struct stat Before{}, After{};
  require(fstat(File, &Before) == 0 && S_ISREG(Before.st_mode) &&
              Before.st_size > 0 &&
              static_cast<uint64_t>(Before.st_size) <= 16 * 1024,
          "LLVM input is not a bounded regular file");
  Input Result;
  Result.Kind = InputKind::LlvmTextIr;
  Result.Bytes.resize(static_cast<size_t>(Before.st_size));
  size_t Offset = 0;
  while (Offset < Result.Bytes.size()) {
    ssize_t Count =
        read(File, Result.Bytes.data() + Offset, Result.Bytes.size() - Offset);
    if (Count < 0 && errno == EINTR)
      continue;
    require(Count > 0, "LLVM input shrank or became unreadable");
    Offset += static_cast<size_t>(Count);
  }
  require(fstat(File, &After) == 0 && Before.st_dev == After.st_dev &&
              Before.st_ino == After.st_ino &&
              Before.st_size == After.st_size &&
              Before.st_mtim.tv_sec == After.st_mtim.tv_sec &&
              Before.st_mtim.tv_nsec == After.st_mtim.tv_nsec &&
              Before.st_ctim.tv_sec == After.st_ctim.tv_sec &&
              Before.st_ctim.tv_nsec == After.st_ctim.tv_nsec,
          "LLVM input changed while reading");
  require(close(File) == 0, "LLVM input close failed");
  Result.Digest = SHA256::hash(Result.Bytes);
  return Result;
}

const Value *unaryValue(const Value *V, unsigned Op) {
  const auto *I = dyn_cast_or_null<Instruction>(V);
  return I && I->getOpcode() == Op && I->getNumOperands() == 1
      ? I->getOperand(0) : nullptr;
}
const Value *shift32(const Value *V) {
  const auto *I = dyn_cast_or_null<BinaryOperator>(V);
  const auto *C = I ? dyn_cast<ConstantInt>(I->getOperand(1)) : nullptr;
  return I && I->getOpcode() == Instruction::LShr && C && C->equalsInt(32)
      ? I->getOperand(0) : nullptr;
}
bool halves(const Value *Lo, const Value *Hi, const Value *InputValue) {
  return InputValue && unaryValue(Lo, Instruction::Trunc) == InputValue &&
      shift32(unaryValue(Hi, Instruction::Trunc)) == InputValue;
}
bool intrinsicValue(const Value *V, Intrinsic::ID Id) {
  const auto *Call = dyn_cast_or_null<CallInst>(V);
  return Call && Call->arg_empty() && Call->getCalledFunction() &&
      Call->getCalledFunction()->isDeclaration() &&
      Call->getCalledFunction()->getIntrinsicID() == Id &&
      Call->getType()->isIntegerTy(32);
}
bool ordinaryIntrinsicAttributes(const Function &F) {
  if (F.hasMetadata() ||
      F.arg_size() != 0 || !F.hasExternalLinkage() || !F.hasDefaultVisibility() ||
      F.getCallingConv() != CallingConv::C ||
      F.getDLLStorageClass() != GlobalValue::DefaultStorageClass || F.isDSOLocal() ||
      F.getUnnamedAddr() != GlobalValue::UnnamedAddr::None || F.hasSection() ||
      F.hasComdat() || F.hasGC() || F.hasPersonalityFn() || F.hasPrefixData() ||
      F.hasPrologueData() ||
      !F.doesNotAccessMemory() || !F.hasFnAttribute(Attribute::NoUnwind) ||
      !F.hasFnAttribute(Attribute::Speculatable) || !F.hasFnAttribute(Attribute::WillReturn))
    return false;
  // LLVM22 materializes the intrinsic's standard return attributes, including
  // noundef and workitem-id range(0,1024), while parsing the input. Compare the
  // complete typed attribute list with that exact intrinsic definition, not
  // with an empty return list or a permissive per-attribute allowlist.
  return F.getAttributes() ==
      Intrinsic::getAttributes(F.getContext(), F.getIntrinsicID(), F.getFunctionType());
}
// Complete typed mechanism dataflow/census, not source authority. The external
// runner pins renderer/worker executable custody; there is no source/V17 owner.
std::optional<std::string> completeSymbol(const Input &InputValue, const CompletePlan &Plan) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  std::string Text(InputValue.Bytes.begin(), InputValue.Bytes.end());
  auto M = parseAssemblyString(Text, Diagnostic, Context);
  if (!M || M->getTargetTriple().str() != TripleName ||
      !M->getModuleInlineAsm().empty() || !M->alias_empty() || !M->ifunc_empty() ||
      !M->global_empty() || !M->named_metadata_empty() || M->size() != 3 ||
      M->getDataLayoutStr() != createMachine()->createDataLayout().getStringRepresentation())
    return std::nullopt;
  std::string Verification;
  raw_string_ostream Errors(Verification);
  if (verifyModule(*M, &Errors)) return std::nullopt;
  const Function *Kernel = nullptr;
  for (const Function &F : *M) {
    if (F.isDeclaration()) {
      if (F.getIntrinsicID() != Intrinsic::amdgcn_workgroup_id_x &&
          F.getIntrinsicID() != Intrinsic::amdgcn_workitem_id_x) return std::nullopt;
      if (!ordinaryIntrinsicAttributes(F)) return std::nullopt;
    } else {
      if (Kernel) return std::nullopt;
      Kernel = &F;
    }
  }
  if (!Kernel || Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      Kernel->size() != 1 || Kernel->arg_size() != 6 || Kernel->isVarArg() ||
      !Kernel->getReturnType()->isVoidTy() || Kernel->hasFnAttribute(Attribute::Naked) ||
      Kernel->getEntryBlock().size() != 18 ||
      !isa<UnreachableInst>(Kernel->getEntryBlock().getTerminator()) ||
      Kernel->getName() != "complete_body_fixture" ||
      !Kernel->hasExternalLinkage() || !Kernel->hasDefaultVisibility() ||
      Kernel->getDLLStorageClass() != GlobalValue::DefaultStorageClass ||
      Kernel->isDSOLocal() || Kernel->getUnnamedAddr() != GlobalValue::UnnamedAddr::None ||
      Kernel->hasSection() || Kernel->hasComdat() || Kernel->hasGC() ||
      Kernel->hasPersonalityFn() || Kernel->hasPrefixData() || Kernel->hasPrologueData() ||
      Kernel->getAttributes().getFnAttrs().getNumAttributes() != 4 ||
      Kernel->getAttributes().getRetAttrs().hasAttributes() ||
      !Kernel->hasFnAttribute(Attribute::NoUnwind)) return std::nullopt;
  for (unsigned I = 0; I < Kernel->arg_size(); ++I)
    if (Kernel->getAttributes().getParamAttrs(I).hasAttributes()) return std::nullopt;
  SmallVector<std::pair<unsigned,MDNode*>,4> Metadata;
  Kernel->getAllMetadata(Metadata);
  const MDNode *Launch = Kernel->getMetadata("reqd_work_group_size");
  if (Metadata.size() != 1 || !Launch || Launch->getNumOperands() != 3 ||
      Metadata[0].first != Context.getMDKindID("reqd_work_group_size"))
    return std::nullopt;
  for (unsigned I = 0; I < 3; ++I) {
    const auto *MDC = dyn_cast_or_null<ConstantAsMetadata>(Launch->getOperand(I).get());
    const auto *C = MDC ? dyn_cast<ConstantInt>(MDC->getValue()) : nullptr;
    if (!C || !C->getType()->isIntegerTy(32) || !C->equalsInt(I == 0 ? 64 : 1))
      return std::nullopt;
  }
  const auto *Pointer = dyn_cast<PointerType>(Kernel->getArg(0)->getType());
  if (!Pointer || Pointer->getAddressSpace() != 1 ||
      !Kernel->getArg(1)->getType()->isIntegerTy(64) ||
      !Kernel->getArg(2)->getType()->isIntegerTy(32) ||
      !Kernel->getArg(3)->getType()->isIntegerTy(32) ||
      !Kernel->getArg(4)->getType()->isIntegerTy(32) ||
      !Kernel->getArg(5)->getType()->isIntegerTy(32) ||
      Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-xnack,-wavefrontsize32,+wavefrontsize64" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size").getValueAsString() != "64,64" ||
      Kernel->hasFnAttribute("amdgpu-implicitarg-num-bytes")) return std::nullopt;
  const CallInst *Unit = nullptr;
  size_t Calls = 0;
  for (const Instruction &I : Kernel->getEntryBlock()) {
    if (I.hasPoisonGeneratingAnnotations() || I.hasMetadata()) return std::nullopt;
    if (const auto *Call = dyn_cast<CallInst>(&I)) {
      if (!Call->getAttributes().isEmpty() || Call->getNumOperandBundles() != 0 || Call->isTailCall() ||
          Call->getCallingConv() != CallingConv::C) return std::nullopt;
      ++Calls;
      if (Call->isInlineAsm()) {
        if (Unit) return std::nullopt;
        Unit = Call;
      } else if (!intrinsicValue(Call, Intrinsic::amdgcn_workgroup_id_x) &&
                 !intrinsicValue(Call, Intrinsic::amdgcn_workitem_id_x)) return std::nullopt;
    } else if (I.mayHaveSideEffects() && !isa<UnreachableInst>(I)) return std::nullopt;
  }
  if (!Unit || Calls != 3 || Unit->arg_size() != 10 || !Unit->getType()->isVoidTy() ||
      Unit->getNextNode() != Kernel->getEntryBlock().getTerminator()) return std::nullopt;
  const auto *Asm = dyn_cast<InlineAsm>(Unit->getCalledOperand());
  if (!Asm || !Asm->hasSideEffects() || Asm->isAlignStack() || Asm->canThrow() ||
      Asm->getDialect() != InlineAsm::AD_ATT || Asm->getAsmString() != completeAssembly(Plan) ||
      Asm->getConstraintString() != completeConstraints(Plan) ||
      !llvm::all_of(Unit->args(), [](const Use &U) { return U->getType()->isIntegerTy(32); }))
    return std::nullopt;
  const auto *Ptr = dyn_cast_or_null<PtrToIntInst>(unaryValue(Unit->getArgOperand(0), Instruction::Trunc));
  if (!Ptr || Ptr->getPointerOperand() != Kernel->getArg(0) ||
      !Ptr->getType()->isIntegerTy(64) ||
      !halves(Unit->getArgOperand(0), Unit->getArgOperand(1), Ptr) ||
      !halves(Unit->getArgOperand(2), Unit->getArgOperand(3), Kernel->getArg(1)) ||
      Unit->getArgOperand(4) != Kernel->getArg(2) ||
      Unit->getArgOperand(5) != Kernel->getArg(3) ||
      Unit->getArgOperand(6) != Kernel->getArg(4) ||
      Unit->getArgOperand(9) != Kernel->getArg(5)) return std::nullopt;
  const auto *Index = dyn_cast_or_null<BinaryOperator>(unaryValue(Unit->getArgOperand(7), Instruction::Trunc));
  if (!Index || Index->getOpcode() != Instruction::Add || !Index->getType()->isIntegerTy(64) ||
      Index->hasNoUnsignedWrap() || Index->hasNoSignedWrap() ||
      !halves(Unit->getArgOperand(7), Unit->getArgOperand(8), Index)) return std::nullopt;
  const auto *Base = dyn_cast<BinaryOperator>(Index->getOperand(0));
  const auto *Stride = Base ? dyn_cast<ConstantInt>(Base->getOperand(1)) : nullptr;
  if (!Base || Base->getOpcode() != Instruction::Mul || !Stride || !Stride->equalsInt(64) ||
      Base->hasNoUnsignedWrap() || Base->hasNoSignedWrap() ||
      !intrinsicValue(unaryValue(Base->getOperand(0), Instruction::ZExt), Intrinsic::amdgcn_workgroup_id_x) ||
      !intrinsicValue(unaryValue(Index->getOperand(1), Instruction::ZExt), Intrinsic::amdgcn_workitem_id_x))
    return std::nullopt;
  return Kernel->getName().str();
}

json::Array completeLlvmNegatives(const Input &InputValue, const CompletePlan &P) {
  std::vector<std::pair<std::string,std::string>> Changes{
    {"~{memory}","~{v5}"},{"{s22}","{s23}"},
    {"~{s21}","~{s23}"},{"s_mov_b64 exec, s[20:21]","s_mov_b64 exec, s[22:23]"},
    {"v_addc_co_u32_e32 v3, vcc, v4, v3, vcc","v_add_co_u32_e32 v3, vcc, v4, v3"},
    {"v_cmp_gt_u64_e32","v_cmp_gt_i64_e32"},{"s_waitcnt vmcnt(0)","s_waitcnt vmcnt(1)"},
    {"lshr i64 %length, 32","lshr i64 %length, 31"},{"mul i64 %group, 64","mul i64 %group, 32"},
    {"!0 = !{i32 64, i32 1, i32 1}","!0 = !{i32 32, i32 1, i32 1}"},
    {"\"target-cpu\"=\"gfx942\"","\"target-cpu\"=\"gfx950\""},
    {"i32 %c, i32 %selector) #0","i32 %c, i64 %selector) #0"},
    {", i32 %selector)\n  unreachable",")\n  unreachable"},
    {"i32 %idx_hi, i32 %selector)\n  unreachable","i32 %selector, i32 %idx_hi)\n  unreachable"},
    {"lshr i64 %ptr, 32","lshr exact i64 %ptr, 32"},
    {"define amdgpu_kernel","define internal amdgpu_kernel"},
    {"  unreachable","  store i32 0, ptr addrspace(1) %data\n  unreachable"},
    {"~{v"+std::to_string(P.Scratch)+"}","~{v7}"},
    {"global_store_dword v[2:3], v"+std::to_string(P.Output)+", off",
     "global_store_dword v[2:3], v"+std::to_string(P.Inputs[0])+", off"}
  };
  if (P.Blocks[0].End==CompleteEnd::Zero) {
    Changes.emplace_back("s_cmp_eq_u32 s22, 0","s_cmp_eq_u32 s23, 0");
    Changes.emplace_back("s_cbranch_scc1 "+completeLabel(1),"s_cbranch_scc1 "+completeLabel(2));
    Changes.emplace_back("s_cbranch_scc1 "+completeLabel(1),"s_cbranch_scc1 "+completeLabel(0));
  }
  json::Array Results;
  for (size_t I=0;I<Changes.size();++I) {
    const auto &[From,To]=Changes[I];
    std::string Text(InputValue.Bytes.begin(),InputValue.Bytes.end());
    const size_t At=Text.find(From);
    require(At!=std::string::npos && Text.find(From,At+From.size())==std::string::npos,
      "typed LLVM mutation anchor missing or ambiguous");
    Text.replace(At,From.size(),To);
    Input Changed=InputValue;
    Changed.Bytes.assign(Text.begin(),Text.end());
    Changed.Digest=SHA256::hash(Changed.Bytes);
    require(!completeSymbol(Changed,P),"typed LLVM mutation accepted");
    Results.emplace_back(json::Object{{"ordinal",I},{"phase","llvm_parser_or_exact_typed_profile"},
      {"changed_sha256",hex(Changed.Digest)},{"accepted",false}});
  }
  return Results;
}
