// Synthetic controls exercise the SAME module guard; no worker/target machine.
json::Object instructionShapeControls() {
  const std::array<const char *, 26> Names{{
      "same-block", "guarded-store", "wrong-last-opcode", "wrong-register",
      "wrong-constraints", "e64", "duplicate-program", "hidden-asm",
      "unused-result", "extra-program-use", "wrong-live-in", "wrong-arity",
      "no-sideeffect", "alignstack", "wrong-store-value", "non-global-store",
      "atomic-store", "volatile-store", "extra-store", "nonentry-program",
      "live-prefix", "dead-prefix", "wrong-target", "wrong-wave",
      "module-asm", "extra-helper"}};
  size_t Positives = 0, Negatives = 0;
  for (InstructionProfile Profile : {InstructionProfile::Default, InstructionProfile::Edited})
    for (const char *Name : Names) {
      const StringRef Case(Name);
      LLVMContext Context; Module ModuleValue("synthetic-instruction-shape", Context);
      ModuleValue.setTargetTriple(Triple(TripleName));
      Type *I32 = Type::getInt32Ty(Context);
      auto *FunctionTypeValue = FunctionType::get(Type::getVoidTy(Context),
          {PointerType::get(Context, 1), Type::getInt64Ty(Context), I32, I32, I32}, false);
      auto *Kernel = Function::Create(FunctionTypeValue, GlobalValue::ExternalLinkage,
          KernelName, ModuleValue);
      Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
      Kernel->addFnAttr("target-cpu", Case == "wrong-target" ? "gfx950" : "gfx942");
      Kernel->addFnAttr("target-features", Case == "wrong-wave"
          ? "+wavefrontsize32,-wavefrontsize64,-xnack" : "-wavefrontsize32,+wavefrontsize64,-xnack");
      Kernel->addFnAttr("amdgpu-flat-work-group-size", "64,64");
      Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, {
          ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
          ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
          ConstantAsMetadata::get(ConstantInt::get(I32, 1))}));
      auto *IntrinsicType = FunctionType::get(I32, false);
      auto *Group = Function::Create(IntrinsicType, GlobalValue::ExternalLinkage,
          "llvm.amdgcn.workgroup.id.x", ModuleValue);
      auto *Local = Function::Create(IntrinsicType, GlobalValue::ExternalLinkage,
          "llvm.amdgcn.workitem.id.x", ModuleValue);
      auto *Entry = BasicBlock::Create(Context, "entry", Kernel);
      IRBuilder<> Builder(Entry);
      if (Case == "nonentry-program") {
        auto *Other = BasicBlock::Create(Context, "other", Kernel);
        Builder.CreateBr(Other); Builder.SetInsertPoint(Other);
      }
      Value *A = Kernel->getArg(2), *B = Kernel->getArg(3), *Mask = Kernel->getArg(4);
      Value *Prefix = nullptr;
      if (Case == "live-prefix" || Case == "dead-prefix")
        Prefix = Builder.CreateOr(A, Mask, "unadmitted-prefix");
      std::string Text = assembly(Profile).str();
      if (Case == "wrong-last-opcode") Text = assembly(Profile == InstructionProfile::Default
          ? InstructionProfile::Edited : InstructionProfile::Default).str();
      if (Case == "wrong-register") Text.replace(Text.find("v4"), 2, "v6");
      if (Case == "e64") Text.replace(Text.rfind("_e32"), 4, "_e64");
      const bool Short = Case == "wrong-arity";
      std::vector<Type *> Parameters(Short ? 2 : 3, I32);
      auto *AsmType = FunctionType::get(I32, Parameters, false);
      const std::string ConstraintText = Short ? "=&{v5},{v0},{v1},~{v4}"
          : Case == "wrong-constraints" ? "=&{v6},{v0},{v1},{v2},~{v4}" : Constraints.str();
      auto *Asm = InlineAsm::get(AsmType, Text, ConstraintText,
          Case != "no-sideeffect", Case == "alignstack");
      std::vector<Value *> Arguments{Case == "wrong-live-in" ? B : A, B};
      if (!Short) Arguments.push_back(Mask);
      auto *Program = Builder.CreateCall(AsmType, Asm, Arguments, "program");
      if (Case == "duplicate-program") Builder.CreateCall(AsmType, Asm, Arguments, "duplicate");
      if (Case == "hidden-asm") {
        auto *HiddenType = FunctionType::get(I32, {I32}, false);
        auto *Hidden = InlineAsm::get(HiddenType, "v_mov_b32 $0, $1", "=v,v", true);
        Builder.CreateCall(HiddenType, Hidden, {A}, "hidden");
      }
      if (Case == "extra-program-use") Builder.CreateAdd(Program, B, "extra");
      // Preserve exactly one otherwise permitted use so the final direct-store
      // boundary, not the earlier unused-result check, must refuse this case.
      if (Case == "wrong-store-value") Builder.CreateICmpEQ(Program, A, "not-stored");
      Builder.CreateCall(IntrinsicType, Group, {}, "group");
      Builder.CreateCall(IntrinsicType, Local, {}, "local");
      BasicBlock *Exit = nullptr;
      if (Case == "guarded-store") {
        auto *Active = BasicBlock::Create(Context, "active", Kernel);
        Exit = BasicBlock::Create(Context, "exit", Kernel);
        Builder.CreateCondBr(Builder.CreateICmpEQ(A, B), Active, Exit);
        Builder.SetInsertPoint(Active);
      }
      Value *Stored = Case == "unused-result" || Case == "wrong-store-value" ? A : Program;
      if (Case == "live-prefix") Stored = Builder.CreateXor(Program, Prefix, "unadmitted-result");
      Value *Pointer = Kernel->getArg(0);
      // No unsupported alloca: isolate the store address-space predicate.
      if (Case == "non-global-store")
        Pointer = ConstantPointerNull::get(PointerType::get(Context, 0));
      auto *Store = Builder.CreateStore(Stored, Pointer); Store->setAlignment(Align(4));
      if (Case == "atomic-store") Store->setAtomic(AtomicOrdering::Monotonic);
      if (Case == "volatile-store") Store->setVolatile(true);
      if (Case == "extra-store") Builder.CreateStore(A, Pointer);
      if (Exit) { Builder.CreateBr(Exit); Builder.SetInsertPoint(Exit); }
      Builder.CreateRetVoid();
      if (Case == "module-asm") ModuleValue.setModuleInlineAsm("s_nop 0");
      if (Case == "extra-helper") {
        auto *HelperType = FunctionType::get(Type::getVoidTy(Context), false);
        auto *Helper = Function::Create(HelperType, GlobalValue::InternalLinkage,
            "unreviewed", ModuleValue);
        auto *Block = BasicBlock::Create(Context, "entry", Helper);
        IRBuilder<>(Block).CreateRetVoid();
      }
      std::string Errors; raw_string_ostream Stream(Errors);
      require(!verifyModule(ModuleValue, &Stream), "synthetic control must remain valid typed LLVM");
      if (Case == "wrong-store-value" || Case == "non-global-store")
        require(exactInstructionCall(*Kernel, *Program, Profile),
            "store boundary controls must preserve the exact one-use assembly call");
      const bool Expected = Case == "same-block" || Case == "guarded-store";
      require(instructionModuleShape(ModuleValue, Profile) == Expected,
          "synthetic exact instruction-module guard control");
      if (Expected) ++Positives; else ++Negatives;
    }
  require(Positives == 4 && Negatives == 48, "typed control census");
  const std::vector<uint8_t> Bytes{'s', 'y', 'n', 't', 'h'};
  const auto Digest = hex(SHA256::hash(Bytes));
  require(exactInputIdentity(Bytes, Digest, Bytes.size()), "input identity positive");
  require(!exactInputIdentity(Bytes, std::string(64, '1'), Bytes.size()) &&
      !exactInputIdentity(Bytes, Digest, Bytes.size() + 1) &&
      !exactInputIdentity(std::vector<uint8_t>{'x', 'y', 'n', 't', 'h'}, Digest, Bytes.size()),
      "input digest/length/content negative controls");
  struct stat Snapshot {};
  Snapshot.st_dev = 1; Snapshot.st_ino = 2; Snapshot.st_mode = S_IFREG | 0600;
  Snapshot.st_nlink = 1; Snapshot.st_size = 5;
  require(sameInputStat(Snapshot, Snapshot), "same file snapshot positive");
  for (size_t Index = 0; Index < 9; ++Index) {
    struct stat Changed = Snapshot;
    switch (Index) {
    case 0: ++Changed.st_dev; break;
    case 1: ++Changed.st_ino; break;
    case 2: Changed.st_mode ^= 1; break;
    case 3: ++Changed.st_nlink; break;
    case 4: ++Changed.st_size; break;
    case 5: ++Changed.st_mtim.tv_sec; break;
    case 6: ++Changed.st_mtim.tv_nsec; break;
    case 7: ++Changed.st_ctim.tv_sec; break;
    case 8: ++Changed.st_ctim.tv_nsec; break;
    }
    require(!sameInputStat(Snapshot, Changed), "changed file snapshot negative");
  }
  return json::Object{{"typed_shape_positives", Positives},
      {"typed_shape_negatives", Negatives}, {"input_identity_positives", 1},
      {"input_identity_negatives", 3}, {"file_snapshot_positives", 1},
      {"file_snapshot_negatives", 9}, {"worker_or_target_machine_invoked", false},
      {"captured_llvm_mutated", false}};
}
