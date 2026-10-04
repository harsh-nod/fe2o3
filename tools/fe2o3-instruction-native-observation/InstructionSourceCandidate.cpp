// TASK-PRIVATE qualification observer, never a production selector or worker.
#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Metadata.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include <algorithm>
#include <array>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <fcntl.h>
#include <memory>
#include <optional>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>
#include <vector>
#if !defined(FE2O3_LLVM_BUILD_ID) || !defined(FE2O3_WORKER_BUILD_ID)
#error "retained unchanged worker build claims are required"
#endif
using namespace fe2o3::worker;
using namespace llvm;
namespace {
constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral KernelName = "choose_bits";
constexpr StringLiteral Constraints = "=&{v5},{v0},{v1},{v2},~{v4}";
constexpr StringLiteral DefaultAssembly =
    "v_xor_b32_e32 v4, $1, $2\n\t"
    "v_and_b32_e32 v4, v4, $3\n\t"
    "v_xor_b32_e32 $0, $2, v4";
constexpr StringLiteral EditedAssembly =
    "v_xor_b32_e32 v4, $1, $2\n\t"
    "v_and_b32_e32 v4, v4, $3\n\t"
    "v_or_b32_e32 $0, $2, v4";
constexpr size_t FixtureByteLimit = 64 * 1024;
constexpr size_t PayloadByteLimit = 1024 * 1024;
constexpr size_t InstructionLimit = 512;
constexpr size_t ReportByteLimit = 64 * 1024;
enum class InstructionProfile { Default, Edited };
StringRef profileName(InstructionProfile Profile) {
  return Profile == InstructionProfile::Default ? "default" : "edited";
}
StringRef assembly(InstructionProfile Profile) {
  return Profile == InstructionProfile::Default ? DefaultAssembly : EditedAssembly;
}
[[noreturn]] void fail(StringRef Message) {
  errs() << "private-instruction-edit observation failed: "
         << Message.take_front(4096) << '\n';
  std::exit(1);
}
void require(bool Condition, StringRef Message) {
  if (!Condition) fail(Message);
}
template <typename T> T unwrap(Expected<T> Value) {
  if (!Value) fail(toString(Value.takeError()));
  return std::move(*Value);
}
std::string hex(ArrayRef<uint8_t> Bytes) {
  constexpr char Digits[] = "0123456789abcdef";
  std::string Text;
  Text.reserve(Bytes.size() * 2);
  for (uint8_t Byte : Bytes) {
    Text += Digits[Byte >> 4]; Text += Digits[Byte & 15];
  }
  return Text;
}
void emit(json::Object Value) {
  const std::string Text = formatv("{0:2}\n", json::Value(std::move(Value))).str();
  require(Text.size() <= ReportByteLimit, "bounded final observation report");
  outs() << Text;
}
// Both retained includes remain byte-for-byte unchanged and separately pinned.
// Synthetic request fields are not executable, source or finalizer custody.
#include "OrderedProgramWorkerSupport.inc"
#include "DefaultSourceMachine.inc"
#include "InstructionSourceInput.inc"
#include "InstructionSourcePayloads.inc"
#include "InstructionSourceControls.inc"
#include "InstructionSourceMachine.inc"
} // namespace

int main(int Argc, char **Argv) {
  alarm(90);
  require((Argc == 2 && StringRef(Argv[1]) == "--shape-controls") || Argc == 6,
      "usage: instruction-source-candidate --shape-controls | default|edited ABS_LLVM SHA256 BYTES NEW_ABS_PAYLOAD_DIR");
  json::Object Controls = instructionShapeControls();
  if (Argc == 2) {
    emit(json::Object{{"report_kind", "private-instruction-edit-shape-controls-v1"},
        {"authority", "none"}, {"controls", std::move(Controls)},
        {"native_compilation", false}, {"hardware_execution", false}});
    alarm(0);
    return 0;
  }
  const StringRef Selector(Argv[1]);
  require(Selector == "default" || Selector == "edited", "closed opcode profile");
  const InstructionProfile Profile = Selector == "default"
      ? InstructionProfile::Default : InstructionProfile::Edited;
  const RetainedInstructionLlvm Source(Argv[2], Argv[3], Argv[4]);
  checkInstructionInput(Source.value(), Profile);
  RetainedInstructionPayloads Payloads(Argv[5]);
  json::Array Cases;
  for (OptimizationLevel Level : {OptimizationLevel::O0, OptimizationLevel::O3}) {
    Source.unchanged();
    auto Built = buildRequest(makeInputRequest(Source.value(), Level), KernelName);
    require(Built.RequestValue.CompilerModule.Bytes == Source.value().Bytes &&
                Built.RequestValue.CompilerModule.Digest == Source.value().Digest,
            "worker compiler input differs from retained exact LLVM");
    auto Observation = describeInstruction(Built, Level, Profile);
    auto Mutations = instructionMachineControls(Built, Profile);
    auto Retained = Payloads.publish(Level, *Built.ResponseValue.LinkedOutput);
    Cases.emplace_back(json::Object{{"machine_observation", std::move(Observation)},
        {"mutation_controls", std::move(Mutations)}, {"retained_payload", std::move(Retained)}});
    Source.unchanged();
    Payloads.unchanged();
  }
  Source.unchanged();
  Payloads.unchanged();
  emit(json::Object{
      {"report_kind", "private-instruction-edit-native-observation-v1"},
      {"authority", "unauthenticated-test-transport"},
      {"profile", profileName(Profile)}, {"kernel_symbol", KernelName},
      {"register_plan", json::Array{4, 5, 0, 1, 2}},
      {"descriptors", json::Array{133, 307,
          Profile == InstructionProfile::Default ? 413 : 412}},
      {"result_use", "sole-direct-nonvolatile-nonatomic-global-store"},
      {"prefix_source_admitted", false},
      {"llvm_sha256", hex(Source.value().Digest)},
      {"llvm_bytes", Source.value().Bytes.size()},
      {"expected_input_identity_matched", true},
      {"retained_file_identity_and_bytes_rechecked", true},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"target", "gfx942:xnack-"}, {"wave_width", 64},
      {"workgroup_size", 64}, {"code_object_version", 6},
      {"shape_controls", std::move(Controls)}, {"cases", std::move(Cases)},
      {"source_ancestry", "not-established-by-llvm-file"},
      {"synthetic_worker_request_identity_fields", true},
      {"production_exact_program_admission", false},
      {"protected_finalizer_admission", false}, {"artifact_authority", false},
      {"source_authentication", false}, {"compiler_closure_attestation", false},
      {"runtime_closure_attestation", "unavailable"},
      {"hardware_execution", false}, {"native_whole_kernel_correctness", false},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"whole_kernel_order_or_byte_stability_claim", false}});
  alarm(0);
  return 0;
}
