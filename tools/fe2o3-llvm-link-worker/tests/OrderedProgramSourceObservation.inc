// TEST-ONLY fixed-profile source-byte observation. Caller labels and raw LLVM
// do not authenticate Rust, canonical-owner custody, or finalizer authority.
// All parser/module checks are observations/refusals, not a production
// selector.

// This is a closed LLVM call-shape observation, not source/CFG custody. Every
// inline-assembly call is classified; unrelated non-assembly calls are
// unchanged.
struct InlineAssemblyCallCensus {
  size_t Programs = 0;
  size_t ScalarMoves = 0;
};

bool observeInlineAssemblyCall(const CallBase &Call, StringRef ExpectedAssembly,
                               bool Used, InlineAssemblyCallCensus &Census) {
  const auto *Asm =
      dyn_cast<InlineAsm>(Call.getCalledOperand()->stripPointerCasts());
  if (!Asm)
    return true;
  const FunctionType *Type = Call.getFunctionType();
  if (Type != Asm->getFunctionType() || Type->isVarArg() ||
      !Type->getReturnType()->isIntegerTy(32) || !Asm->hasSideEffects() ||
      Asm->isAlignStack() || Asm->canThrow() ||
      Asm->getDialect() != InlineAsm::AD_ATT ||
      Type->getNumParams() != Call.arg_size() ||
      !llvm::all_of(
          Type->params(),
          [](const llvm::Type *Value) { return Value->isIntegerTy(32); }) ||
      !llvm::all_of(Call.args(), [](const Use &Argument) {
        return Argument->getType()->isIntegerTy(32);
      }))
    return false;
  if (Asm->getAsmString() == "v_mov_b32 $0, $1" &&
      Asm->getConstraintString() == "=v,v")
    return Call.arg_size() == 1 && ++Census.ScalarMoves == 1;
  if (Asm->getAsmString() == ExpectedAssembly &&
      Asm->getConstraintString() == Constraints)
    return Call.arg_size() == 3 && Call.use_empty() == !Used &&
           ++Census.Programs == 1;
  return false;
}

// Pure in-process parser/typed-call controls. They construct only synthetic IR;
// no captured LLVM bytes are edited, and no target machine or worker is
// invoked.
json::Object inlineAssemblyGuardControls() {
  size_t Positives = 0, Negatives = 0;
  const auto Check = [&](Profile Kind, bool Used, StringRef ExtraCalls,
                         bool Expected, size_t ProgramCount = 1) {
    std::string EscapedAssembly;
    for (char Byte : assembly(Kind)) {
      if (Byte == '\n')
        EscapedAssembly += "\\0A";
      else if (Byte == '\t')
        EscapedAssembly += "\\09";
      else
        EscapedAssembly += Byte;
    }
    std::string Text = "declare i32 @ordinary(i32)\n"
                       "define void @control() {\n";
    Text += ExtraCalls.str();
    for (size_t Index = 0; Index < ProgramCount; ++Index) {
      const std::string Name = "%program" + std::to_string(Index);
      Text += "  " + Name + " = call i32 asm sideeffect \"" + EscapedAssembly +
              "\", \"" + Constraints.str() + "\"(i32 1, i32 2, i32 3)\n";
      if (Used)
        Text +=
            "  %keep" + std::to_string(Index) + " = add i32 " + Name + ", 0\n";
    }
    Text += "  ret void\n}\n";
    require(Text.size() <= FixtureByteLimit, "inline-asm guard fixture cap");
    LLVMContext Context;
    SMDiagnostic Diagnostic;
    auto Parsed = parseAssemblyString(Text, Diagnostic, Context);
    require(Parsed != nullptr, "inline-asm guard control must parse");
    std::string Verification;
    raw_string_ostream Errors(Verification);
    require(!verifyModule(*Parsed, &Errors),
            "inline-asm guard control must be valid typed LLVM");
    const Function *FunctionValue = Parsed->getFunction("control");
    require(FunctionValue != nullptr, "inline-asm guard control function");
    InlineAssemblyCallCensus Census;
    bool Accepted = true;
    const std::string ExpectedAssembly = assembly(Kind);
    for (const BasicBlock &Block : *FunctionValue)
      for (const Instruction &InstructionValue : Block)
        if (const auto *Call = dyn_cast<CallBase>(&InstructionValue))
          Accepted = observeInlineAssemblyCall(*Call, ExpectedAssembly, Used,
                                               Census) &&
                     Accepted;
    Accepted = Accepted && Census.Programs == 1;
    require(Accepted == Expected, "inline-asm guard control expectation");
    if (Expected)
      ++Positives;
    else
      ++Negatives;
  };
  constexpr StringLiteral Scalar = "  %scalar = call i32 asm sideeffect "
                                   "\"v_mov_b32 $0, $1\", \"=v,v\"(i32 7)\n";
  for (Profile Kind : {Profile::One, Profile::Three, Profile::Sixteen})
    for (bool Used : {false, true}) {
      Check(Kind, Used, "", true);
      Check(Kind, Used, Scalar, true);
    }
  Check(Profile::Three, true, "  %ordinary = call i32 @ordinary(i32 7)\n",
        true);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm sideeffect \"v_not_b32 $0, $1\", "
        "\"=v,v\"(i32 7)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm sideeffect \"v_mov_b32 $0, $1\", "
        "\"=s,s\"(i32 7)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm \"v_mov_b32 $0, $1\", \"=v,v\"(i32 7)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i64 asm sideeffect \"v_mov_b32 $0, $1\", "
        "\"=v,v\"(i64 7)\n",
        false);
  Check(Profile::Three, true,
        Scalar.str() + "  %scalar2 = call i32 asm sideeffect \"v_mov_b32 $0, "
                       "$1\", \"=v,v\"(i32 8)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm sideeffect alignstack \"v_mov_b32 $0, $1\", "
        "\"=v,v\"(i32 7)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm sideeffect unwind \"v_mov_b32 $0, $1\", "
        "\"=v,v\"(i32 7)\n",
        false);
  Check(Profile::Three, true,
        "  %scalar = call i32 asm sideeffect inteldialect \"v_mov_b32 $0, "
        "$1\", \"=v,v\"(i32 7)\n",
        false);
  Check(Profile::Three, true, "  call void asm sideeffect \"\", \"\"()\n",
        false);
  Check(Profile::Three, true, Scalar, false, 0);
  Check(Profile::Three, true, "", false, 2);
  require(Positives == 13 && Negatives == 11,
          "inline-asm guard control census");
  return json::Object{{"pure_parser_typed_call_positives", Positives},
                      {"pure_parser_typed_call_negatives", Negatives},
                      {"worker_or_target_machine_invoked", false}};
}

Input readSourceLlvm(StringRef Path) {
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
              static_cast<uint64_t>(Before.st_size) <= FixtureByteLimit,
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

std::string sourceKernelSymbol(const Input &InputValue, Profile Kind,
                               bool Used) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  // Sentinel-bearing parser copy preserves exact logical bytes and digest.
  const std::string Text(InputValue.Bytes.begin(), InputValue.Bytes.end());
  auto Parsed = parseAssemblyString(Text, Diagnostic, Context);
  require(Parsed != nullptr, "LLVM observation input is not valid textual IR");
  require(Parsed->getTargetTriple().str() == TripleName &&
              Parsed->getModuleInlineAsm().empty() && Parsed->alias_empty() &&
              Parsed->ifunc_empty(),
          "LLVM input requires exact triple and no module asm/aliases/ifuncs");
  std::string Symbol;
  size_t Instructions = 0, DefinedFunctions = 0;
  InlineAssemblyCallCensus Census;
  const std::string ExpectedAssembly = assembly(Kind);
  for (const Function &FunctionValue : *Parsed) {
    if (FunctionValue.isDeclaration())
      continue;
    require(++DefinedFunctions == 1 &&
                FunctionValue.getCallingConv() == CallingConv::AMDGPU_KERNEL,
            "LLVM input requires one defined AMD kernel and no helpers");
    Symbol = FunctionValue.getName().str();
    require(FunctionValue.hasFnAttribute("target-cpu") &&
                FunctionValue.getFnAttribute("target-cpu").getValueAsString() ==
                    "gfx942" &&
                FunctionValue.hasFnAttribute("target-features") &&
                FunctionValue.hasFnAttribute("amdgpu-flat-work-group-size") &&
                FunctionValue.getFnAttribute("amdgpu-flat-work-group-size")
                        .getValueAsString() == "64,64",
            "LLVM input must explicitly declare exact gfx942 and workgroup64 "
            "attributes");
    const StringRef Features =
        FunctionValue.getFnAttribute("target-features").getValueAsString();
    require(Features == "-xnack,-wavefrontsize32,+wavefrontsize64" ||
                Features == "-wavefrontsize32,+wavefrontsize64,-xnack",
            "LLVM input must explicitly declare the closed wave64/xnack-off "
            "feature set");
    for (const BasicBlock &Block : FunctionValue) {
      for (const Instruction &InstructionValue : Block) {
        require(++Instructions <= 8192,
                "preoptimization LLVM instruction bound");
        const auto *Call = dyn_cast<CallBase>(&InstructionValue);
        if (!Call)
          continue;
        require(
            observeInlineAssemblyCall(*Call, ExpectedAssembly, Used, Census),
            "LLVM inline asm is outside the exact program and optional "
            "single scalar-move profile");
      }
    }
  }
  require(Census.Programs == 1,
          "LLVM input has no exact fixed-register ordered program call");
  require(!Symbol.empty() && Symbol.size() <= 128 &&
              ((Symbol[0] >= 'A' && Symbol[0] <= 'Z') ||
               (Symbol[0] >= 'a' && Symbol[0] <= 'z') || Symbol[0] == '_') &&
              llvm::all_of(Symbol,
                           [](char Byte) {
                             return (Byte >= 'A' && Byte <= 'Z') ||
                                    (Byte >= 'a' && Byte <= 'z') ||
                                    (Byte >= '0' && Byte <= '9') || Byte == '_';
                           }),
          "kernel symbol is outside the closed observation profile");
  require(!Parsed->getDataLayoutStr().empty(),
          "LLVM input must retain its explicit compiler-owned layout");
  auto Machine = createMachine();
  require(Parsed->getDataLayoutStr() ==
              Machine->createDataLayout().getStringRepresentation(),
          "LLVM input layout is not the pinned gfx942 worker layout");
  std::string Verification;
  raw_string_ostream Stream(Verification);
  require(!verifyModule(*Parsed, &Stream), "LLVM input fails verification");
  return Symbol;
}

json::Object descriptorResources(const BuiltFixture &Built, StringRef Symbol) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  StringRef Data(reinterpret_cast<const char *>(Payload.data()),
                 Payload.size());
  auto Object = unwrap(object::ObjectFile::createObjectFile(
      MemoryBufferRef(Data, "<test-only-final-descriptor>")));
  std::optional<json::Object> Result;
  size_t Count = 0;
  for (const auto &Candidate : Object->symbols()) {
    require(++Count <= 4096, "descriptor symbol scan exceeds test bound");
    if (unwrap(Candidate.getName()) != Symbol.str() + ".kd")
      continue;
    require(!Result, "duplicate final descriptor symbol");
    auto Section = unwrap(Candidate.getSection());
    require(Section != Object->section_end(), "descriptor has no section");
    auto Contents = unwrap(Section->getContents());
    uint64_t Address = unwrap(Candidate.getAddress());
    require(Address >= Section->getAddress(),
            "descriptor address precedes section");
    uint64_t Relative = Address - Section->getAddress();
    require(Relative <= Contents.size() && Contents.size() - Relative >= 64,
            "descriptor is outside its exact section bytes");
    const uint8_t *Bytes =
        reinterpret_cast<const uint8_t *>(Contents.data()) + Relative;
    const uintptr_t Base = reinterpret_cast<uintptr_t>(Payload.data());
    const uintptr_t Begin = reinterpret_cast<uintptr_t>(Bytes);
    require(Begin >= Base && Begin - Base <= Payload.size() &&
                Payload.size() - (Begin - Base) >= 64,
            "descriptor does not name final payload storage");
    const auto Digest = SHA256::hash(ArrayRef<uint8_t>(Bytes, 64));
    require(Digest == Built.Evidence.Entries[0].DescriptorIdentity,
            "independent descriptor bytes differ from final decoder");
    const uint32_t Rsrc3 = support::endian::read32le(Bytes + 44);
    const uint32_t Rsrc1 = support::endian::read32le(Bytes + 48);
    // Gfx942 is a gfx90a-family allocation profile: RSRC1 uses EIGHT,
    // not generic gfx9 wave64's four-register granule. RSRC3's architected
    // VGPR / accumulator boundary uses four. Neither field is a live-use count.
    const uint32_t Capacity = ((Rsrc1 & 0x3f) + 1) * 8;
    const uint32_t ArchitectedBoundary = ((Rsrc3 & 0x3f) + 1) * 4;
    require(Capacity >= 37 && ArchitectedBoundary >= 37 &&
                Capacity >= ArchitectedBoundary,
            "final descriptor does not cover fixed v0..v36 footprint");
    Result = json::Object{
        {"descriptor_file_offset", Begin - Base},
        {"descriptor_bytes", 64},
        {"descriptor_sha256", hex(Digest)},
        {"compute_pgm_rsrc1", Rsrc1},
        {"compute_pgm_rsrc3", Rsrc3},
        {"vgpr_capacity", Capacity},
        {"architected_vgpr_boundary", ArchitectedBoundary},
        {"required_footprint_high_water", 37},
        {"interpretation", "encoded-capacity-not-metadata-usage-or-lifetime"}};
  }
  require(Result.has_value(), "final descriptor symbol is absent");
  return std::move(*Result);
}

int observeSourceLlvm(Profile Kind, bool Used, StringRef Path) {
  const Input InputValue = readSourceLlvm(Path);
  const std::string Symbol = sourceKernelSymbol(InputValue, Kind, Used);
  json::Array Cases;
  for (OptimizationLevel Level :
       {OptimizationLevel::O0, OptimizationLevel::O3}) {
    auto Built =
        buildRequest(makeInputRequest(InputValue, Level, Symbol), Symbol);
    auto Case = describePositive(Kind, Built, Level, Used, Symbol);
    Case["descriptor_resources"] = descriptorResources(Built, Symbol);
    Cases.emplace_back(std::move(Case));
  }
  const Input After = readSourceLlvm(Path);
  require(After.Bytes == InputValue.Bytes && After.Digest == InputValue.Digest,
          "exact LLVM input bytes changed during the two compilations");
  emitReport(json::Object{
      {"schema", "fe2o3-ordered-program-llvm-machine-observation-v1"},
      {"authority", "unauthenticated-test-transport"},
      {"source_ancestry", "not-established-by-llvm-file"},
      {"synthetic_worker_request_identity_fields", true},
      {"production_exact_program_admission", false},
      {"protected_finalizer_admission", false},
      {"hardware_executed", false},
      {"runtime_closure_attestation", "unavailable"},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"whole_kernel_order_or_byte_stability_claim", false},
      {"target", "gfx942:xnack-"},
      {"wave_width", 64},
      {"workgroup_size", 64},
      {"code_object_version", 6},
      {"kernel_symbol", Symbol},
      {"profile", profileName(Kind)},
      {"program_count", steps(Kind).size()},
      {"result_used", Used},
      {"result_use_scope",
       "checked SSA use in supplied LLVM; not source authentication"},
      {"expected_program", expectedJson(Kind)},
      {"llvm_text_sha256", hex(InputValue.Digest)},
      {"llvm_text_bytes", InputValue.Bytes.size()},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"encoding_reference_scope", "public gfx900 cross-check; actual pinned "
                                   "gfx942 qualification is this run"},
      {"cases", std::move(Cases)}});
  return 0;
}
