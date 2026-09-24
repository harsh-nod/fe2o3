// The existing worker route and post-link checks remain unchanged. The only
// new analyzer profile admits the exact retained source helper closure.
void helperWriteAt(int Directory, StringRef Leaf, ArrayRef<uint8_t> Bytes) {
  int FD = openat(Directory, Leaf.str().c_str(),
                  O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  require(FD >= 0, "exclusive helper observation output");
  size_t At = 0;
  while (At < Bytes.size()) {
    ssize_t N = write(FD, Bytes.data() + At, Bytes.size() - At);
    if (N < 0 && errno == EINTR)
      continue;
    require(N > 0, "helper observation output write");
    At += static_cast<size_t>(N);
  }
  require(fsync(FD) == 0 && close(FD) == 0, "helper observation output close");
}
int helperDirectory(StringRef Path) {
  require(Path.starts_with("/") && Path.size() <= 4096 &&
              !Path.contains('\0') &&
              llvm::none_of(Path, [](char C) {
                return static_cast<unsigned char>(C) < 32 || C == 127;
              }),
          "helper output directory path bound");
  require(mkdir(Path.str().c_str(), 0700) == 0,
          "helper output directory must be fresh");
  char Canonical[PATH_MAX];
  require(realpath(Path.str().c_str(), Canonical) && Path == Canonical,
          "helper output directory must be canonical");
  int FD = open(Path.str().c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
  require(FD >= 0, "helper observation output directory");
  return FD;
}
Response helperBuild(const HelperInput &In, OptimizationLevel Level,
                     int Directory, json::Array &PostLink) {
  Request RequestValue = makeInputRequest(In.Llvm, Level, In.Entry);
  Response ResponseValue = execute(RequestValue);
  if (!ResponseValue.LinkedOutput) {
    checkDiagnosticBounds(ResponseValue.Diagnostics);
    for (const auto &D : ResponseValue.Diagnostics)
      errs() << D << '\n';
    fail("actual helper LLVM worker refused; no native success inferred");
  }
  const auto &OutputValue = *ResponseValue.LinkedOutput;
  require(ResponseValue.FailureStage == Stage::Complete &&
              ResponseValue.Protocol == ProtocolVersion::V2 &&
              ResponseValue.RequestId == RequestValue.RequestId &&
              ResponseValue.RequestIdentity == RequestValue.Identity &&
              ResponseValue.CompilerEnvelopeIdentity == RequestValue.CompilerEnvelopeIdentity &&
              ResponseValue.WorkerBuildIdentity == FE2O3_WORKER_BUILD_ID &&
              !ResponseValue.DeviceLibraryProvider &&
              !OutputValue.Bytes.empty() &&
              OutputValue.Bytes.size() <= PayloadByteLimit &&
              OutputValue.Digest == SHA256::hash(OutputValue.Bytes) &&
              ResponseValue.Derivation.has_value(),
          "worker success lacks bounded exact output/identity fields");
  const auto &D = *ResponseValue.Derivation;
  require(D.Hsaco.Digest == OutputValue.Digest &&
              D.Hsaco.ByteLength == OutputValue.Bytes.size() &&
              D.EvidenceIdentity == unwrap(calculateDerivationEvidenceIdentity(D)) &&
              D.NativeLinkInputs.size() == 1 &&
              D.NativeLinkInputs[0].Source == NativeLinkInputSource::GeneratedObject &&
              D.NativeLinkInputs[0].Content == D.GeneratedObject,
          "ordinary helper LLVM/object/LLD derivation mismatch");
  // Preserve the exact output even if subsequent inspection or decoding refuses.
  helperWriteAt(Directory, "output.hsaco", OutputValue.Bytes);
  auto Diagnostics =
      unwrap(inspectLinkedOutputForPublication(OutputValue.Bytes, RequestValue));
  checkInspectionDiagnostics(Diagnostics, In.Entry);
  for (const auto &Diagnostic : Diagnostics)
    PostLink.emplace_back(Diagnostic);
  return ResponseValue;
}
PhysicalMachineEffectEvidence helperAnalyze(const HelperInput &In,
                                             ArrayRef<uint8_t> Bytes) {
  const auto Identities = physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest RequestValue;
  RequestValue.AnalyzerIdentity = Identities.Analyzer;
  RequestValue.ToolchainIdentity = Identities.Toolchain;
  RequestValue.ExecutionChallenge.fill(0x70);
  RequestValue.RequestIdentity.fill(0x71); // Synthetic, never wire custody.
  RequestValue.RequestBytes = 4096;
  RequestValue.Payload.assign(Bytes.begin(), Bytes.end());
  RequestValue.PayloadDigest = SHA256::hash(Bytes);
  RequestValue.PayloadBytes = Bytes.size();
  RequestValue.Entries = {{In.Entry, {16, 8, 4, 3, 2}}};
  auto E = unwrap(analyzeGfx942PhysicalMachineEffects(RequestValue));
  require(E.PayloadDigest == RequestValue.PayloadDigest &&
              E.PayloadBytes == Bytes.size() &&
              E.ExecutionChallenge == RequestValue.ExecutionChallenge &&
              E.RequestIdentity == RequestValue.RequestIdentity &&
              E.RequestBytes == RequestValue.RequestBytes &&
              E.AnalyzerIdentity == Identities.Analyzer &&
              E.ToolchainIdentity == Identities.Toolchain &&
              E.Entries.size() == 1 && E.Entries[0].Symbol == In.Entry &&
              !E.Functions.empty() && E.Functions.size() <= In.Helpers.size() + 1 &&
              !E.Instructions.empty() && E.Instructions.size() <= 512 &&
              !E.Blocks.empty() && E.Blocks.size() <= 64 &&
              E.Effects.size() <= 64,
          "bounded helper analyzer identity/closure");
  std::vector<std::string> Names;
  for (const auto &F : E.Functions) {
    require((F.Symbol == In.Entry ||
             std::binary_search(In.Helpers.begin(), In.Helpers.end(), F.Symbol)) &&
                F.CodeOffset <= Bytes.size() && F.CodeSize > 0 &&
                F.CodeSize <= Bytes.size() - F.CodeOffset &&
                F.DirectCallees.size() <= In.Helpers.size(),
            "native helper function absent from exact source roster");
    for (const auto &Callee : F.DirectCallees)
      require(F.Symbol == In.Entry &&
                  std::binary_search(In.Helpers.begin(), In.Helpers.end(), Callee),
              "native helper call outside closed root/helper graph");
    Names.push_back(F.Symbol);
  }
  std::sort(Names.begin(), Names.end());
  require(std::adjacent_find(Names.begin(), Names.end()) == Names.end() &&
              std::binary_search(Names.begin(), Names.end(), In.Entry),
          "native helper function identity census");
  for (const auto &B : E.Blocks)
    require(std::binary_search(Names.begin(), Names.end(), B.FunctionSymbol) &&
                B.Successors.size() <= 2,
            "native helper block closure");
  size_t Calls = 0;
  for (const auto &I : E.Instructions) {
    require(std::binary_search(Names.begin(), Names.end(), I.FunctionSymbol) &&
                I.InstructionOffset <= Bytes.size() && !I.Encoding.empty() &&
                I.Encoding.size() <= 16 &&
                I.Encoding.size() <= Bytes.size() - I.InstructionOffset &&
                std::equal(I.Encoding.begin(), I.Encoding.end(),
                           Bytes.begin() + I.InstructionOffset) &&
                I.Operands.size() <= 16 && I.ImplicitDefinitions.size() <= 16 &&
                I.ImplicitUses.size() <= 16 && I.Opcode.size() <= 128,
            "native helper instruction bytes/closure bounds");
    if (I.BranchKind == PhysicalMachineBranchKind::DirectCall) {
      require(++Calls <= In.Helpers.size() && I.FunctionSymbol == In.Entry,
              "native direct call census");
      require(llvm::any_of(E.Functions, [&](const auto &F) {
                return F.Symbol != In.Entry && F.CodeOffset == I.BranchTarget;
              }),
              "native call target is not an exact retained helper entry");
    }
  }
  return E;
}
json::Object helperNativeObservation(const PhysicalMachineEffectEvidence &E) {
  json::Array Functions, Blocks, Instructions;
  size_t Calls = 0, Returns = 0, Vors = 0;
  for (const auto &F : E.Functions) {
    json::Array Callees;
    for (const auto &C : F.DirectCallees)
      Callees.emplace_back(C);
    Functions.emplace_back(json::Object{
        {"symbol", F.Symbol}, {"file_offset", F.CodeOffset},
        {"code_bytes", F.CodeSize}, {"direct_callees", std::move(Callees)}});
  }
  for (const auto &B : E.Blocks) {
    json::Array Successors;
    for (const auto S : B.Successors)
      Successors.emplace_back(S);
    Blocks.emplace_back(json::Object{
        {"function", B.FunctionSymbol}, {"ordinal", B.Ordinal},
        {"first_file_offset", B.FirstInstructionOffset},
        {"instruction_count", B.InstructionCount}, {"successors", std::move(Successors)}});
  }
  for (const auto &I : E.Instructions) {
    auto Row = instructionJson(I);
    json::Array Operands;
    for (const auto &O : I.Operands)
      Operands.emplace_back(json::Object{
          {"kind", static_cast<unsigned>(O.Kind)}, {"register", O.Register},
          {"value_u64_decimal", std::to_string(O.Value)}, {"tied_to", O.TiedTo}});
    Row["function"] = I.FunctionSymbol;
    Row["block_ordinal"] = I.BlockOrdinal;
    Row["operands"] = std::move(Operands);
    Row["explicit_definitions"] = I.ExplicitDefinitionCount;
    Row["memory_access"] = static_cast<unsigned>(I.MemoryAccess);
    Row["memory_width"] = I.MemoryWidth;
    Row["branch_kind"] = static_cast<unsigned>(I.BranchKind);
    Row["branch_target_file_offset"] = I.BranchTarget;
    Instructions.emplace_back(std::move(Row));
    Calls += I.BranchKind == PhysicalMachineBranchKind::DirectCall;
    Returns += I.BranchKind == PhysicalMachineBranchKind::Return;
    Vors += StringRef(I.Opcode).starts_with("V_OR_B32");
  }
  return json::Object{
      {"descriptor_sha256", hex(E.Entries[0].DescriptorIdentity)},
      {"analyzer_identity", hex(E.AnalyzerIdentity)},
      {"toolchain_identity", hex(E.ToolchainIdentity)},
      {"functions", std::move(Functions)}, {"blocks", std::move(Blocks)},
      {"instructions", std::move(Instructions)}, {"direct_calls", Calls},
      {"returns", Returns}, {"observed_v_or_b32_sites", Vors},
      {"native_helper_argument_result_flow_qualified", false},
      {"native_root_value_guard_store_semantics_qualified", false},
      {"retained_native_helper_calls_required", false},
      {"all_six_live_source_operations_qualified", false}};
}
