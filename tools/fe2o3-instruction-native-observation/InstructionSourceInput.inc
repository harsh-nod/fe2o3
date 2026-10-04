// Exact retained raw bytes plus typed two-case no-prefix LLVM input guard.
bool sameInputStat(const struct stat &A, const struct stat &B) {
  return A.st_dev == B.st_dev && A.st_ino == B.st_ino &&
      A.st_mode == B.st_mode && A.st_nlink == B.st_nlink &&
      A.st_size == B.st_size &&
      A.st_mtim.tv_sec == B.st_mtim.tv_sec &&
      A.st_mtim.tv_nsec == B.st_mtim.tv_nsec &&
      A.st_ctim.tv_sec == B.st_ctim.tv_sec &&
      A.st_ctim.tv_nsec == B.st_ctim.tv_nsec;
}
bool exactDigestText(StringRef Text) {
  return Text.size() == 64 && Text != std::string(64, '0') &&
      llvm::all_of(Text, [](char C) {
        return (C >= '0' && C <= '9') || (C >= 'a' && C <= 'f');
      });
}
bool exactSizeText(StringRef Text, uint64_t &Size) {
  return !Text.empty() && Text.size() <= 5 && Text.front() != '0' &&
      llvm::all_of(Text, [](char C) { return C >= '0' && C <= '9'; }) &&
      !Text.getAsInteger(10, Size) && Size > 0 && Size <= FixtureByteLimit;
}
bool exactInputIdentity(ArrayRef<uint8_t> Bytes, StringRef Digest, uint64_t Size) {
  return !Bytes.empty() && Bytes.size() <= FixtureByteLimit &&
      Bytes.size() == Size && exactDigestText(Digest) &&
      hex(SHA256::hash(Bytes)) == Digest;
}
class RetainedInstructionLlvm {
  std::string Path;
  int File = -1;
  struct stat Before {};
  Input Value;

  void canonicalPath() const {
    char *Resolved = realpath(Path.c_str(), nullptr);
    require(Resolved != nullptr, "input path is unavailable");
    const bool Equal = Path == Resolved;
    free(Resolved);
    require(Equal, "input path must be canonical without symlink traversal");
  }
  std::vector<uint8_t> readBytes() const {
    std::vector<uint8_t> Bytes(static_cast<size_t>(Before.st_size));
    size_t Offset = 0;
    while (Offset < Bytes.size()) {
      const ssize_t Count = pread(File, Bytes.data() + Offset,
          Bytes.size() - Offset, static_cast<off_t>(Offset));
      if (Count < 0 && errno == EINTR) continue;
      require(Count > 0, "retained LLVM shrank or became unreadable");
      Offset += static_cast<size_t>(Count);
    }
    uint8_t Extra = 0; ssize_t Count;
    do { Count = pread(File, &Extra, 1, static_cast<off_t>(Bytes.size())); }
    while (Count < 0 && errno == EINTR);
    require(Count == 0, "retained LLVM grew");
    return Bytes;
  }
public:
  RetainedInstructionLlvm(StringRef Name, StringRef ExpectedDigest,
                          StringRef ExpectedSize) : Path(Name.str()) {
    uint64_t Size = 0;
    require(Name.starts_with("/") && Name.size() <= 4096 &&
        llvm::none_of(Name, [](char C) {
          return static_cast<unsigned char>(C) < 32 || C == 127;
        }) && exactDigestText(ExpectedDigest) && exactSizeText(ExpectedSize, Size),
        "bounded absolute input and exact lowercase hash/size required");
    canonicalPath();
    File = open(Path.c_str(), O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
    require(File >= 0 && fstat(File, &Before) == 0 &&
        S_ISREG(Before.st_mode) && Before.st_size > 0 &&
        static_cast<uint64_t>(Before.st_size) == Size,
        "retained regular LLVM file differs from selected input size");
    Value.Kind = InputKind::LlvmTextIr;
    Value.Bytes = readBytes();
    Value.Digest = SHA256::hash(Value.Bytes);
    require(exactInputIdentity(Value.Bytes, ExpectedDigest, Size),
        "retained LLVM differs from selected producer hash");
    unchanged();
  }
  RetainedInstructionLlvm(const RetainedInstructionLlvm &) = delete;
  RetainedInstructionLlvm &operator=(const RetainedInstructionLlvm &) = delete;
  ~RetainedInstructionLlvm() { if (File >= 0) close(File); }
  const Input &value() const { return Value; }
  void unchanged() const {
    canonicalPath();
    struct stat Current {}, Named {};
    require(fstat(File, &Current) == 0 && lstat(Path.c_str(), &Named) == 0 &&
        sameInputStat(Before, Current) && sameInputStat(Before, Named),
        "retained LLVM file identity or metadata changed");
    const auto Bytes = readBytes();
    require(Bytes == Value.Bytes && SHA256::hash(Bytes) == Value.Digest,
        "retained LLVM bytes changed");
    require(fstat(File, &Current) == 0 && lstat(Path.c_str(), &Named) == 0 &&
        sameInputStat(Before, Current) && sameInputStat(Before, Named),
        "retained LLVM changed while rechecked");
  }
};

bool exactInstructionCall(const Function &FunctionValue, const CallBase &Call,
                          InstructionProfile Profile) {
  if (FunctionValue.arg_size() != 5 || !isa<CallInst>(Call)) return false;
  const auto *Asm = dyn_cast<InlineAsm>(Call.getCalledOperand()->stripPointerCasts());
  if (!Asm || Asm->getAsmString() != assembly(Profile) ||
      Asm->getConstraintString() != Constraints || !Asm->hasSideEffects() ||
      Asm->isAlignStack() || Asm->canThrow() || Asm->getDialect() != InlineAsm::AD_ATT ||
      Asm->getFunctionType() != Call.getFunctionType() ||
      Call.getFunctionType()->isVarArg() || !Call.getType()->isIntegerTy(32) ||
      Call.arg_size() != 3 || !Call.hasOneUse()) return false;
  for (size_t Index = 0; Index < 3; ++Index)
    if (!Call.getArgOperand(Index)->getType()->isIntegerTy(32) ||
        Call.getArgOperand(Index) != FunctionValue.getArg(Index + 2)) return false;
  return true;
}

bool instructionModuleShape(Module &ModuleValue, InstructionProfile Profile) {
  if (ModuleValue.getTargetTriple().str() != TripleName ||
      !ModuleValue.getModuleInlineAsm().empty() || !ModuleValue.alias_empty() ||
      !ModuleValue.ifunc_empty() || !ModuleValue.global_empty()) return false;
  size_t Definitions = 0, Declarations = 0, Instructions = 0, Blocks = 0;
  size_t GroupCalls = 0, LocalCalls = 0, Stores = 0;
  const CallBase *Program = nullptr;
  const StoreInst *Store = nullptr;
  Function *Kernel = nullptr;
  for (Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration()) {
      if (++Declarations > 2 || !FunctionValue.arg_empty() || FunctionValue.isVarArg() ||
          !FunctionValue.getReturnType()->isIntegerTy(32) ||
          (FunctionValue.getName() != "llvm.amdgcn.workgroup.id.x" &&
           FunctionValue.getName() != "llvm.amdgcn.workitem.id.x")) return false;
      continue;
    }
    if (++Definitions != 1 || FunctionValue.getName() != KernelName ||
        FunctionValue.getCallingConv() != CallingConv::AMDGPU_KERNEL ||
        !FunctionValue.getReturnType()->isVoidTy() || FunctionValue.isVarArg() ||
        FunctionValue.arg_size() != 5 || FunctionValue.empty()) return false;
    Kernel = &FunctionValue;
    const auto *Pointer = dyn_cast<PointerType>(FunctionValue.getArg(0)->getType());
    if (!Pointer || Pointer->getAddressSpace() != 1 ||
        !FunctionValue.getArg(1)->getType()->isIntegerTy(64)) return false;
    for (size_t Index = 2; Index < 5; ++Index)
      if (!FunctionValue.getArg(Index)->getType()->isIntegerTy(32)) return false;
    if (!FunctionValue.hasFnAttribute("target-cpu") ||
        !FunctionValue.hasFnAttribute("target-features") ||
        !FunctionValue.hasFnAttribute("amdgpu-flat-work-group-size") ||
        FunctionValue.getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
        FunctionValue.getFnAttribute("amdgpu-flat-work-group-size").getValueAsString() != "64,64")
      return false;
    const StringRef Features = FunctionValue.getFnAttribute("target-features").getValueAsString();
    if (Features != "-wavefrontsize32,+wavefrontsize64,-xnack" &&
        Features != "-xnack,-wavefrontsize32,+wavefrontsize64") return false;
    const MDNode *Launch = FunctionValue.getMetadata("reqd_work_group_size");
    if (!Launch || Launch->getNumOperands() != 3) return false;
    for (size_t Index = 0; Index < 3; ++Index) {
      const auto *Dimension = mdconst::dyn_extract<ConstantInt>(Launch->getOperand(Index));
      if (!Dimension || Dimension->getValue().getLimitedValue() != (Index == 0 ? 64 : 1))
        return false;
    }
    for (const BasicBlock &Block : FunctionValue) {
      if (++Blocks > 128) return false;
      for (const Instruction &Value : Block) {
        if (++Instructions > 8192) return false;
        if (const auto *Call = dyn_cast<CallBase>(&Value)) {
          if (!isa<CallInst>(Call)) return false;
          if (isa<InlineAsm>(Call->getCalledOperand()->stripPointerCasts())) {
            if (Program || !exactInstructionCall(FunctionValue, *Call, Profile)) return false;
            Program = Call;
          } else {
            const auto *Callee = Call->getCalledFunction();
            if (!Callee || !Callee->isDeclaration() || !Call->arg_empty()) return false;
            if (Callee->getName() == "llvm.amdgcn.workgroup.id.x") ++GroupCalls;
            else if (Callee->getName() == "llvm.amdgcn.workitem.id.x") ++LocalCalls;
            else return false;
          }
        } else if (const auto *Candidate = dyn_cast<StoreInst>(&Value)) {
          if (++Stores != 1 || Candidate->isAtomic() || Candidate->isVolatile() ||
              Candidate->getPointerAddressSpace() != 1) return false;
          Store = Candidate;
        } else if (const auto *Binary = dyn_cast<BinaryOperator>(&Value)) {
          // Only ordinary i64 launch-index arithmetic remains outside the program.
          if (!Binary->getType()->isIntegerTy(64) ||
              (Binary->getOpcode() != Instruction::Add &&
               Binary->getOpcode() != Instruction::Mul)) return false;
        } else if (!isa<ZExtInst>(Value) && !isa<ICmpInst>(Value) &&
                   !isa<GetElementPtrInst>(Value) && !isa<BranchInst>(Value) &&
                   !isa<SwitchInst>(Value) && !isa<UnreachableInst>(Value) &&
                   !isa<ReturnInst>(Value)) return false;
      }
    }
  }
  if (Definitions != 1 || Declarations != 2 || GroupCalls != 1 || LocalCalls != 1 ||
      !Kernel || !Program || !Store || Program->getParent() != &Kernel->getEntryBlock() ||
      &Kernel->getEntryBlock().front() != Program || Store->getValueOperand() != Program)
    return false;
  DominatorTree Dominators(*Kernel);
  return Dominators.dominates(Program, Store);
}

void checkInstructionInput(const Input &Value, InstructionProfile Profile) {
  LLVMContext Context; SMDiagnostic Diagnostic;
  const std::string Text(Value.Bytes.begin(), Value.Bytes.end());
  auto ModuleValue = parseAssemblyString(Text, Diagnostic, Context);
  require(ModuleValue != nullptr, "raw LLVM parsing failed");
  std::string Errors; raw_string_ostream Stream(Errors);
  require(!verifyModule(*ModuleValue, &Stream), "raw LLVM module verification failed");
  require(instructionModuleShape(*ModuleValue, Profile), "closed no-prefix instruction profile refused");
  auto Machine = createMachine();
  require(ModuleValue->getDataLayoutStr() == Machine->createDataLayout().getStringRepresentation(),
      "raw compiler LLVM data layout differs from pinned target");
}
