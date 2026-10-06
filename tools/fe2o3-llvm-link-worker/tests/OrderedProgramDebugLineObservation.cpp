// Explicit CPU-only test target. Reuse the existing O0/O3 in-process LLVM/LLD
// route and its full exact program/descriptor checks. No Worker process, GPU,
// production admission, source authentication or automatic CTest registration.
#define main fe2o3_ordered_program_original_main
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/IR/DebugInfoMetadata.h"

namespace {
void publishLinkedDebugPayload(StringRef Directory, StringRef Name,
                               ArrayRef<uint8_t> Bytes) {
  require(!Bytes.empty() && Bytes.size() <= PayloadByteLimit,
          "bounded linked DWARF payload");
  const std::string Path = Directory.str() + "/" + Name.str();
  const int Fd = ::open(Path.c_str(), O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  require(Fd >= 0, "new linked DWARF payload output required");
  size_t Offset = 0, Calls = 0;
  while (Offset < Bytes.size() && ++Calls <= 1024) {
    const ssize_t Count = ::write(Fd, Bytes.data() + Offset, Bytes.size() - Offset);
    if (Count < 0 && errno == EINTR)
      continue;
    if (Count <= 0) {
      ::close(Fd);
      fail("linked DWARF payload write");
    }
    Offset += static_cast<size_t>(Count);
  }
  const int Closed = ::close(Fd);
  require(Offset == Bytes.size() && Closed == 0, "linked DWARF payload terminal");
}

int verifyHelperDebugFixture(StringRef Path) {
  const Input Source = readSourceLlvm(Path);
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  const std::string Text(Source.Bytes.begin(), Source.Bytes.end());
  auto Parsed = parseAssemblyString(Text, Diagnostic, Context);
  require(Parsed != nullptr, "helper debug fixture parse");
  const Function *Root = Parsed->getFunction("region_kernel");
  const Function *Helper = Parsed->getFunction("ordinary_line_helper");
  require(Root && Helper && !Root->isDeclaration() && !Helper->isDeclaration()
              && Root->getCallingConv() == CallingConv::AMDGPU_KERNEL
              && Root->getSubprogram() && !Helper->getSubprogram(),
          "exact annotated root and unannotated helper required");
  size_t Defined = 0;
  for (const Function &F : *Parsed)
    Defined += !F.isDeclaration();
  require(Defined == 2 && Helper->arg_size() == 1
              && Helper->getReturnType()->isIntegerTy(32)
              && Helper->getArg(0)->getType()->isIntegerTy(32)
              && Helper->size() == 1 && Helper->front().size() == 1,
          "fixed two-function identity-helper fixture");
  const auto *Return = dyn_cast<ReturnInst>(&Helper->front().front());
  require(Return && Return->getReturnValue() == Helper->getArg(0),
          "ordinary helper is actual identity return");
  std::array<const CallBase *, 3> Calls{};
  size_t CallCount = 0, Instructions = 0;
  for (const BasicBlock &Block : *Root)
    for (const Instruction &I : Block) {
      require(++Instructions <= 8192, "helper fixture instruction bound");
      if (const auto *Call = dyn_cast<CallBase>(&I)) {
        require(CallCount < Calls.size(), "exactly three fixture calls");
        Calls[CallCount++] = Call;
      }
    }
  require(CallCount == 3 && Calls[0]->getCalledFunction() == Helper
              && Calls[2]->getCalledFunction() == Helper
              && !Calls[0]->getDebugLoc() && !Calls[2]->getDebugLoc()
              && Calls[1]->getDebugLoc()
              && Calls[1]->getDebugLoc()->getScope() == Root->getSubprogram()
              && Calls[0]->getParent() == Calls[1]->getParent()
              && Calls[1]->getParent() == Calls[2]->getParent(),
          "helper calls must bracket the only located asm in one block");
  InlineAssemblyCallCensus Census;
  require(observeInlineAssemblyCall(*Calls[1], assembly(Profile::Three), true, Census)
              && Census.Programs == 1 && Census.ScalarMoves == 0
              && Calls[1]->getArgOperand(0) == Calls[0]
              && Calls[2]->arg_size() == 1
              && Calls[2]->getArgOperand(0) == Calls[1],
          "helper result/ordered call/result-helper dataflow");
  bool BrokenDebugInfo = false;
  std::string Verification;
  raw_string_ostream Errors(Verification);
  require(!verifyModule(*Parsed, &Errors, &BrokenDebugInfo) && !BrokenDebugInfo,
          "helper fixture IR and debug info must both verify");
  const Input After = readSourceLlvm(Path);
  require(After.Bytes == Source.Bytes && After.Digest == Source.Digest,
          "helper fixture changed during verification");
  emitReport(json::Object{
      {"schema", "private-ordered-debug-helper-verification-v1"},
      {"llvm_sha256", hex(Source.Digest)}, {"llvm_bytes", Source.Bytes.size()},
      {"root", "region_kernel"}, {"helper", "ordinary_line_helper"},
      {"ordinary_calls", 2}, {"located_ordered_calls", 1},
      {"ir_verified", true}, {"debug_info_verified", true},
      {"native_emitted", false}, {"hardware_observed", false},
      {"grants_artifact_or_launch_authority", false}});
  return 0;
}
}

int main(int Argc, char **Argv) {
  alarm(90);
  require(Argc == 3, "expected ABS_LLVM ABS_FRESH_OUTPUT_DIRECTORY or --verify-helper-only ABS_LLVM");
  if (StringRef(Argv[1]) == "--verify-helper-only") {
    const int Status = verifyHelperDebugFixture(Argv[2]);
    alarm(0);
    return Status;
  }
  const StringRef Directory(Argv[2]);
  require(Directory.starts_with("/") && Directory.size() <= 4096,
          "bounded absolute output directory");
  struct stat Status {};
  require(::lstat(Argv[2], &Status) == 0 && S_ISDIR(Status.st_mode),
          "real output directory required");
  const Input Source = readSourceLlvm(Argv[1]);
  const std::string Symbol = sourceKernelSymbol(Source, Profile::Three, true);
  const StringRef Text(reinterpret_cast<const char *>(Source.Bytes.data()),
                       Source.Bytes.size());
  require(Text.contains("!DILocation(") && Text.contains("!llvm.dbg.cu"),
          "opt-in emitted line metadata required");
  json::Array Cases;
  for (OptimizationLevel Level : {OptimizationLevel::O0, OptimizationLevel::O3}) {
    const StringRef Name = Level == OptimizationLevel::O0 ? "O0.hsaco" : "O3.hsaco";
    auto RequestValue = makeInputRequest(Source, Level, Symbol);
    // Existing request uses StripDebug=false. Do not change linker/optimizer
    // policy merely to make a debug-section observation pass.
    auto Built = buildRequest(std::move(RequestValue), Symbol);
    auto Case = describePositive(Profile::Three, Built, Level, true, Symbol);
    Case["descriptor_resources"] = descriptorResources(Built, Symbol);
    publishLinkedDebugPayload(Directory, Name, Built.ResponseValue.LinkedOutput->Bytes);
    Case["payload_file"] = Name;
    Cases.emplace_back(std::move(Case));
  }
  const Input After = readSourceLlvm(Argv[1]);
  require(After.Bytes == Source.Bytes && After.Digest == Source.Digest,
          "exact LLVM input changed during O0/O3 compilation");
  emitReport(json::Object{
      {"schema", "fe2o3-ordered-region-line-native-observation-v17"},
      {"llvm_sha256", hex(Source.Digest)}, {"llvm_bytes", Source.Bytes.size()},
      {"kernel_symbol", Symbol},
      {"cases", std::move(Cases)}, {"line_table_verified", false},
      {"line_table_verification", "separate pinned llvm-dwarfdump and exact source-location join required"},
      {"synthetic_worker_request_identity_fields", true},
      {"source_authentication", false}, {"protected_admission", false},
      {"hardware_executed", false}});
  alarm(0);
  return 0;
}
