// TEST-ONLY copied bounded pipeline support from the reviewed fixed-pair test.
// No production WorkerPipeline/Protocol/MachineEffect gate is altered.
// Request identity placeholders are visibly synthetic, not executable custody.

std::unique_ptr<TargetMachine> createMachine() {
  static const bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUAsmPrinter();
    LLVMInitializeAMDGPUAsmParser();
    LLVMInitializeAMDGPUDisassembler();
    return true;
  }();
  (void)Initialized;
  Triple TripleValue(TripleName);
  std::string Diagnostic;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, Diagnostic);
  require(TargetValue != nullptr, Diagnostic);
  TargetOptions Options;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, "gfx942", "-xnack,-wavefrontsize32,+wavefrontsize64",
      Options, Reloc::PIC_, CodeModel::Small, CodeGenOptLevel::None));
  require(Machine != nullptr, "cannot create pinned gfx942 machine");
  return Machine;
}

Request makeInputRequest(Input CompilerModule, OptimizationLevel Level,
                         StringRef Symbol = KernelName) {
  Request Value;
  Value.RequestId.fill(0x41);
  Value.Identity.fill(0x42);
  Value.Protocol = ProtocolVersion::V2;
  Value.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  Value.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
  // Exactly like the existing native pipeline tests, these are visibly
  // synthetic request fields. They are NOT measured executable/owner custody.
  Value.WorkerExecutableDigest.fill(0x51);
  Value.WorkerExecutableBytes = 4096;
  Value.CompilerEnvelopeIdentity.fill(0x62);
  Value.Target = "gfx942:xnack-";
  Value.CodeObjectVersion = 6;
  Value.LinkOptions = {Level, false, true};
  Value.CompilerModule = std::move(CompilerModule);
  Value.Inputs = {Value.CompilerModule};
  Value.RequiredSymbols = {Symbol.str(), Symbol.str() + ".kd"};
  Value.ExpectedDefinedSymbols = Value.RequiredSymbols;
  Value.ExportSymbols = {Symbol.str()};
  Value.FinalSymbols = Value.RequiredSymbols;
  Value.MaxOutputBytes = PayloadByteLimit;
  return Value;
}

PhysicalMachineEffectEvidence analyzePayload(ArrayRef<uint8_t> Bytes,
                                             StringRef Symbol = KernelName) {
  require(!Bytes.empty() && Bytes.size() <= PayloadByteLimit,
          "payload exceeds prototype cap");
  const auto Identities = physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest RequestValue;
  RequestValue.AnalyzerIdentity = Identities.Analyzer;
  RequestValue.ToolchainIdentity = Identities.Toolchain;
  RequestValue.ExecutionChallenge.fill(0x70);
  RequestValue.RequestIdentity.fill(
      0x71); // Native-test marker, not wire custody.
  RequestValue.RequestBytes = 4096;
  RequestValue.Payload.assign(Bytes.begin(), Bytes.end());
  RequestValue.PayloadDigest = SHA256::hash(Bytes);
  RequestValue.PayloadBytes = Bytes.size();
  RequestValue.Entries = {{Symbol.str(), {16, 8, 4, 2, 0}}};
  auto Result = unwrap(analyzeGfx942PhysicalMachineEffects(RequestValue));
  require(Result.PayloadDigest == RequestValue.PayloadDigest &&
              Result.PayloadBytes == Bytes.size() &&
              Result.ExecutionChallenge == RequestValue.ExecutionChallenge &&
              Result.RequestIdentity == RequestValue.RequestIdentity &&
              Result.RequestBytes == RequestValue.RequestBytes &&
              Result.AnalyzerIdentity == RequestValue.AnalyzerIdentity &&
              Result.ToolchainIdentity == RequestValue.ToolchainIdentity &&
              Result.Entries.size() == 1 && Result.Functions.size() == 1 &&
              Result.Entries[0].Symbol == Symbol &&
              Result.Functions[0].Symbol == Symbol &&
              !Result.Instructions.empty() &&
              Result.Instructions.size() <= InstructionLimit,
          "decoded fixture identity, entry or instruction bounds changed");
  return Result;
}

struct BuiltFixture {
  Request RequestValue;
  Response ResponseValue;
  PhysicalMachineEffectEvidence Evidence;
  std::vector<std::string> InspectionDiagnostics;
};

void checkDiagnosticBounds(ArrayRef<std::string> Diagnostics) {
  require(Diagnostics.size() <= MaxDiagnostics,
          "inspection diagnostic count exceeds existing worker cap");
  size_t Total = 0;
  for (const auto &Diagnostic : Diagnostics) {
    require(Diagnostic.size() <= MaxDiagnosticBytes &&
                Diagnostic.size() <= MaxTotalDiagnosticBytes - Total,
            "inspection diagnostics exceed existing worker byte caps");
    Total += Diagnostic.size();
  }
}

void checkInspectionDiagnostics(ArrayRef<std::string> Diagnostics,
                                StringRef Symbol = KernelName) {
  checkDiagnosticBounds(Diagnostics);
  // inspectLinkedOutputForPublication already validated the real ELF symbol
  // closure and compiler launch contract before returning these diagnostics.
  // These are reporting-contract assertions, not an alternative symbol parser
  // or authority derived from diagnostic strings.
  size_t Targets = 0, Exports = 0, Unresolved = 0, Metadata = 0, Kernels = 0;
  for (const auto &Diagnostic : Diagnostics) {
    StringRef Text(Diagnostic);
    Targets += Text.starts_with(
        "post_link.check=target status=ok arch=gfx942 code_object_version=6 "
        "e_flags=");
    Exports += Text == "post_link.check=exports status=ok "
                       "symbols=[" +
                           Symbol.str() + "," + Symbol.str() + ".kd]";
    Unresolved += Text == "post_link.check=unresolved status=ok symbols=[]";
    Metadata += Text == "post_link.check=metadata status=ok kernels=1 "
                        "target=amdgcn-amd-amdhsa--gfx942%3Axnack-";
    Kernels += Text.starts_with("post_link.kernel name=" + Symbol.str() +
                                " symbol=" + Symbol.str() + ".kd ") &&
               Text.ends_with(" wavefront_size=64 max_workgroup_size=64 "
                              "reqd_workgroup_size=[64,1,1]");
  }
  require(Diagnostics.size() == 5 && Targets == 1 && Exports == 1 &&
              Unresolved == 1 && Metadata == 1 && Kernels == 1,
          "bounded post-link diagnostic facts changed");
}

BuiltFixture buildRequest(Request RequestValue, StringRef Symbol = KernelName) {
  Response ResponseValue = execute(RequestValue);
  if (!ResponseValue.LinkedOutput) {
    size_t Remaining = MaxTotalDiagnosticBytes;
    for (const std::string &Diagnostic : ResponseValue.Diagnostics) {
      StringRef Part = StringRef(Diagnostic).take_front(Remaining);
      errs() << Part << '\n';
      Remaining -= Part.size();
      if (Remaining == 0)
        break;
    }
    fail("closed native LLVM fixture did not complete the worker pipeline");
  }
  const Output &OutputValue = *ResponseValue.LinkedOutput;
  require(ResponseValue.FailureStage == Stage::Complete &&
              ResponseValue.Protocol == ProtocolVersion::V2 &&
              ResponseValue.RequestId == RequestValue.RequestId &&
              ResponseValue.RequestIdentity == RequestValue.Identity &&
              ResponseValue.CompilerEnvelopeIdentity ==
                  RequestValue.CompilerEnvelopeIdentity &&
              ResponseValue.WorkerBuildIdentity == FE2O3_WORKER_BUILD_ID &&
              !ResponseValue.DeviceLibraryProvider &&
              !OutputValue.Bytes.empty() &&
              OutputValue.Bytes.size() <= PayloadByteLimit &&
              OutputValue.Digest == SHA256::hash(OutputValue.Bytes) &&
              ResponseValue.Derivation.has_value(),
          "worker success lacks exact bounded output/custody fields");
  const DerivationEvidence &Derivation = *ResponseValue.Derivation;
  require(Derivation.Hsaco.Digest == OutputValue.Digest &&
              Derivation.Hsaco.ByteLength == OutputValue.Bytes.size() &&
              Derivation.EvidenceIdentity ==
                  unwrap(calculateDerivationEvidenceIdentity(Derivation)) &&
              Derivation.NativeLinkInputs.size() == 1 &&
              Derivation.NativeLinkInputs[0].Source ==
                  NativeLinkInputSource::GeneratedObject &&
              Derivation.NativeLinkInputs[0].Content ==
                  Derivation.GeneratedObject,
          "normal LLVM/object/LLD derivation does not end at these bytes");
  auto InspectionDiagnostics = unwrap(
      inspectLinkedOutputForPublication(OutputValue.Bytes, RequestValue));
  checkInspectionDiagnostics(InspectionDiagnostics, Symbol);
  auto Evidence = analyzePayload(OutputValue.Bytes, Symbol);
  return {std::move(RequestValue), std::move(ResponseValue),
          std::move(Evidence), std::move(InspectionDiagnostics)};
}
