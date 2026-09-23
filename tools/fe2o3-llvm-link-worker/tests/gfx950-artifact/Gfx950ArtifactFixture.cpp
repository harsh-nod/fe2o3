// Diagnostic fixture only: actual unchanged worker emission, synthetic request
// identity fields, no source admission, proof, protected publication or dispatch.
#include "WorkerPipeline.h"
#include "llvm/ADT/StringExtras.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include <cerrno>
#include <cstdlib>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
using namespace llvm;
using namespace fe2o3::worker;
namespace {
[[noreturn]] void fail(StringRef Message) {
  errs() << Message << '\n';
  std::exit(1);
}
void require(bool Condition, StringRef Message) { if (!Condition) fail(Message); }
std::string moduleText() {
  LLVMInitializeAMDGPUTargetInfo();
  LLVMInitializeAMDGPUTarget();
  LLVMInitializeAMDGPUTargetMC();
  LLVMInitializeAMDGPUAsmPrinter();
  LLVMInitializeAMDGPUAsmParser();
  Triple TripleValue("amdgcn-amd-amdhsa");
  std::string Diagnostic;
  const Target *TargetValue = TargetRegistry::lookupTarget("amdgcn", TripleValue, Diagnostic);
  require(TargetValue != nullptr, Diagnostic);
  TargetOptions Options;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, "gfx950", "-xnack,-wavefrontsize32,+wavefrontsize64",
      Options, Reloc::PIC_, CodeModel::Small, CodeGenOptLevel::None));
  require(Machine != nullptr, "cannot create actual gfx950 machine");
  return "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"" +
    Machine->createDataLayout().getStringRepresentation() + R"LLVM("
declare i32 @llvm.amdgcn.workitem.id.x()
define amdgpu_kernel void @fe2o3_gfx950_observation_fixture(ptr addrspace(1) %out)
  nounwind "target-cpu"="gfx950"
  "target-features"="-xnack,-wavefrontsize32,+wavefrontsize64"
  "amdgpu-flat-work-group-size"="64,64" !reqd_work_group_size !0 {
entry:
  %lane = call i32 @llvm.amdgcn.workitem.id.x()
  %index = zext i32 %lane to i64
  %pointer = getelementptr i32, ptr addrspace(1) %out, i64 %index
  store i32 0, ptr addrspace(1) %pointer, align 4
  ret void
}
!0 = !{i32 64, i32 1, i32 1}
)LLVM";
}
void retain(StringRef Path, ArrayRef<uint8_t> Bytes) {
  require(Path.starts_with("/") && Path.size() <= 4096 &&
          !Bytes.empty() && Bytes.size() <= 65536, "bounded absolute output required");
  int FD = open(Path.str().c_str(), O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  require(FD >= 0, "fresh regular artifact output required");
  // One finite attempt; a short write fails rather than retrying indefinitely.
  ssize_t Written = write(FD, Bytes.data(), Bytes.size());
  struct stat State {};
  bool Good = Written == static_cast<ssize_t>(Bytes.size()) &&
      fstat(FD, &State) == 0 && S_ISREG(State.st_mode) &&
      State.st_size == static_cast<off_t>(Bytes.size());
  int Closed = close(FD);
  require(Good && Closed == 0, "retaining artifact failed; partial output is not qualified");
}
}
int main(int Count, char **Arguments) {
  require(Count == 2, "usage: gfx950-artifact-fixture ABS_FRESH_HSACO");
  const std::string Text = moduleText();
  require(Text.size() <= 4096, "fixed input exceeded cap");
  Request RequestValue;
  RequestValue.RequestId.fill(0x41);
  RequestValue.Identity.fill(0x42);
  RequestValue.Protocol = ProtocolVersion::V2;
  RequestValue.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  RequestValue.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
  RequestValue.WorkerExecutableDigest.fill(0x51);
  RequestValue.WorkerExecutableBytes = 4096;
  RequestValue.CompilerEnvelopeIdentity.fill(0x62);
  RequestValue.Target = "gfx950:xnack-";
  RequestValue.CodeObjectVersion = 6;
  RequestValue.LinkOptions = {OptimizationLevel::O0, false, true};
  RequestValue.CompilerModule.Kind = InputKind::LlvmTextIr;
  RequestValue.CompilerModule.Bytes.assign(Text.begin(), Text.end());
  RequestValue.CompilerModule.Digest = SHA256::hash(RequestValue.CompilerModule.Bytes);
  RequestValue.Inputs = {RequestValue.CompilerModule};
  RequestValue.RequiredSymbols = {
    "fe2o3_gfx950_observation_fixture", "fe2o3_gfx950_observation_fixture.kd"};
  RequestValue.ExpectedDefinedSymbols = RequestValue.RequiredSymbols;
  RequestValue.ExportSymbols = {"fe2o3_gfx950_observation_fixture"};
  RequestValue.FinalSymbols = RequestValue.RequiredSymbols;
  RequestValue.MaxOutputBytes = 65536;
  const Response Reply = execute(RequestValue);
  if (!Reply.LinkedOutput) {
    size_t Remaining = MaxTotalDiagnosticBytes;
    for (const std::string &Diagnostic : Reply.Diagnostics) {
      auto Part = StringRef(Diagnostic).take_front(Remaining);
      errs() << Part << '\n';
      Remaining -= Part.size();
      if (!Remaining) break;
    }
    fail("actual worker emission refused");
  }
  const Output &Artifact = *Reply.LinkedOutput;
  require(Reply.FailureStage == Stage::Complete &&
      Reply.Protocol == ProtocolVersion::V2 &&
      Reply.RequestId == RequestValue.RequestId &&
      Reply.RequestIdentity == RequestValue.Identity &&
      Reply.CompilerEnvelopeIdentity == RequestValue.CompilerEnvelopeIdentity &&
      Reply.WorkerBuildIdentity == FE2O3_WORKER_BUILD_ID &&
      !Reply.DeviceLibraryProvider && Reply.Derivation &&
      !Artifact.Bytes.empty() && Artifact.Bytes.size() <= 65536 &&
      Artifact.Digest == SHA256::hash(Artifact.Bytes), "worker result relation mismatch");
  const DerivationEvidence &D = *Reply.Derivation;
  auto Identity = calculateDerivationEvidenceIdentity(D);
  require(static_cast<bool>(Identity), "worker derivation identity failed");
  require(D.Hsaco.Digest == Artifact.Digest &&
      D.Hsaco.ByteLength == Artifact.Bytes.size() && D.EvidenceIdentity == *Identity &&
      D.NativeLinkInputs.size() == 1 &&
      D.NativeLinkInputs[0].Source == NativeLinkInputSource::GeneratedObject &&
      D.NativeLinkInputs[0].Content == D.GeneratedObject, "native derivation mismatch");
  auto Inspection = inspectLinkedOutputForPublication(Artifact.Bytes, RequestValue);
  if (!Inspection) fail(toString(Inspection.takeError()));
  require(Inspection->size() <= MaxDiagnostics, "inspection count cap");
  size_t DiagnosticBytes = 0;
  json::Array Diagnostics;
  for (const auto &Item : *Inspection) {
    require(Item.size() <= MaxDiagnosticBytes &&
      Item.size() <= MaxTotalDiagnosticBytes - DiagnosticBytes, "inspection byte cap");
    DiagnosticBytes += Item.size();
    Diagnostics.push_back(Item);
  }
  retain(Arguments[1], Artifact.Bytes);
  json::Object Report{
    {"schema", "diagnostic-gfx950-artifact-fixture-v1"},
    {"target", "gfx950:xnack-"}, {"code_object_version", 6},
    {"kernel_name", "fe2o3_gfx950_observation_fixture"},
    {"input_bytes", static_cast<int64_t>(Text.size())},
    {"input_sha256", toHex(RequestValue.CompilerModule.Digest, true)},
    {"artifact_bytes", static_cast<int64_t>(Artifact.Bytes.size())},
    {"artifact_sha256", toHex(Artifact.Digest, true)},
    {"derivation_sha256", toHex(D.EvidenceIdentity, true)},
    {"synthetic_request_identity_fields", true},
    {"source_authenticated", false}, {"protected_publication", false},
    {"gpu_execution", false}, {"inspection", std::move(Diagnostics)}};
  std::string Encoded;
  raw_string_ostream Stream(Encoded);
  Stream << json::Value(std::move(Report)) << '\n';
  require(Encoded.size() <= 65536, "report cap");
  outs() << Encoded;
}
