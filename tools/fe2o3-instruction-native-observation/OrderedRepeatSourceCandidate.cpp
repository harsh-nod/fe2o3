// Task-private repeat observation; never a production selector or launcher.
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
#error "unchanged independently selected worker/LLVM build claims required"
#endif
using namespace fe2o3::worker;
using namespace llvm;
namespace {
constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral KernelName = "ordered_repeat_u32";
constexpr StringLiteral Constraints = "=&{v33},{v34},{v35},{v36},~{v32}";
constexpr size_t FixtureByteLimit = 64 * 1024;
constexpr size_t PayloadByteLimit = 1024 * 1024;
constexpr size_t InstructionLimit = 512;
constexpr size_t ReportByteLimit = 64 * 1024;
constexpr uint32_t RequiredBindingExtent = 37;
// The unchanged typed input helper is parameterized by this local profile type,
// assembly(), KernelName and Constraints. No old XOR profile/matcher is included.
using InstructionProfile = unsigned;
bool validRepeatCount(unsigned Count) { return Count == 1 || Count == 2 || Count == 15; }
std::optional<unsigned> countSelector(StringRef Text) {
  if (Text == "1") return 1;
  if (Text == "2") return 2;
  if (Text == "15") return 15;
  return std::nullopt;
}
std::string assembly(InstructionProfile Count) {
  std::string Text = "v_mov_b32_e32 $0, $1";
  for (unsigned Index = 0; Index < Count; ++Index)
    Text += "\n\tv_add_u32_e32 $0, $0, $2";
  return Text;
}
[[noreturn]] void fail(StringRef Message) {
  errs() << "task-ordered-repeat-native observation failed: "
         << Message.take_front(4096) << '\n';
  std::exit(1);
}
void require(bool Condition, StringRef Message) { if (!Condition) fail(Message); }
template <typename T> T unwrap(Expected<T> Value) {
  if (!Value) fail(toString(Value.takeError()));
  return std::move(*Value);
}
std::string hex(ArrayRef<uint8_t> Bytes) {
  constexpr char Digits[] = "0123456789abcdef";
  std::string Text; Text.reserve(Bytes.size() * 2);
  for (uint8_t Byte : Bytes) { Text += Digits[Byte >> 4]; Text += Digits[Byte & 15]; }
  return Text;
}
void emit(json::Object Value) {
  const std::string Text = formatv("{0:2}\n", json::Value(std::move(Value))).str();
  require(Text.size() <= ReportByteLimit, "bounded final report");
  outs() << Text;
}
// Byte-identical shared helpers: original LLVM fd/stat/hash retention,
// exact typed ABI/result-use guard, create-new payloads and existing pipeline.
// The worker support uses disclosed synthetic request identity placeholders.
#include "OrderedProgramWorkerSupport.inc"
#include "InstructionSourceInput.inc"
#include "InstructionSourcePayloads.inc"
#include "OrderedRepeatSourceMachine.inc"
#include "OrderedRepeatSourceControls.inc"
} // namespace

int main(int Argc, char **Argv) {
  alarm(90);
  require((Argc == 2 && StringRef(Argv[1]) == "--shape-controls") || Argc == 6,
      "usage: ordered-repeat-source-candidate --shape-controls | 1|2|15 ABS_LLVM SHA256 BYTES NEW_ABS_PAYLOAD_DIR");
  auto Controls = repeatShapeControls();
  if (Argc == 2) {
    emit(json::Object{{"report_kind", "task-ordered-repeat-native-shape-controls-v1"},
        {"authority", "none"}, {"controls", std::move(Controls)},
        {"native_compilation", false}, {"hardware_execution", false}});
    alarm(0); return 0;
  }
  const auto Count = countSelector(Argv[1]);
  require(Count.has_value(), "repeat count must be exactly 1, 2 or 15");
  const RetainedInstructionLlvm Source(Argv[2], Argv[3], Argv[4]);
  checkInstructionInput(Source.value(), *Count);
  RetainedInstructionPayloads Payloads(Argv[5]);
  json::Array Cases;
  for (OptimizationLevel Level : {OptimizationLevel::O0, OptimizationLevel::O3}) {
    Source.unchanged();
    auto Built = buildRequest(makeInputRequest(Source.value(), Level), KernelName);
    require(Built.RequestValue.CompilerModule.Bytes == Source.value().Bytes &&
        Built.RequestValue.CompilerModule.Digest == Source.value().Digest,
        "worker input differs from selected exact LLVM");
    auto Observation = describeRepeat(Built, Level, *Count);
    auto Mutations = repeatMachineControls(Built, *Count);
    auto Retained = Payloads.publish(Level, *Built.ResponseValue.LinkedOutput);
    Cases.emplace_back(json::Object{{"machine_observation", std::move(Observation)},
        {"mutation_controls", std::move(Mutations)}, {"retained_payload", std::move(Retained)}});
    Source.unchanged(); Payloads.unchanged();
  }
  Source.unchanged(); Payloads.unchanged();
  emit(json::Object{
      {"report_kind", "task-ordered-repeat-native-observation-v1"},
      {"authority", "unauthenticated-test-transport"},
      {"repetitions", *Count}, {"program_count", *Count + 1},
      {"kernel_symbol", KernelName}, {"register_plan", json::Array{32, 33, 34, 35, 36}},
      {"constraints", Constraints}, {"required_binding_extent", RequiredBindingExtent},
      {"result_use", "sole-direct-nonvolatile-nonatomic-global-store"},
      {"prefix_source_admitted", false},
      {"llvm_sha256", hex(Source.value().Digest)}, {"llvm_bytes", Source.value().Bytes.size()},
      {"expected_input_identity_matched", true},
      {"retained_file_identity_and_bytes_rechecked", true},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID}, {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"target", "gfx942:xnack-"}, {"wave_width", 64}, {"workgroup_size", 64},
      {"code_object_version", 6}, {"shape_controls", std::move(Controls)}, {"cases", std::move(Cases)},
      {"source_ancestry", "not-established-by-llvm-file"},
      {"synthetic_worker_request_identity_fields", true},
      {"production_exact_program_admission", false}, {"protected_finalizer_admission", false},
      {"artifact_authority", false}, {"source_authentication", false},
      {"compiler_closure_attestation", false}, {"runtime_closure_attestation", "unavailable"},
      {"hardware_execution", false}, {"native_whole_kernel_correctness", false},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"whole_kernel_order_or_byte_stability_claim", false}});
  alarm(0); return 0;
}
