// TEST ONLY: capture unchanged actual helper-source LLVM through the ordinary
// pinned worker. No synthetic program is invoked and no GPU is opened.
#define main included_ordered_program_main_not_invoked
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include <limits.h>
#include <tuple>

namespace {
#include "HelperSourceRecord.inc"
#include "HelperSourceLlvm.inc"
#include "HelperSourceWorker.inc"
} // namespace

int main(int Argc, char **Argv) {
  require(Argc == 6,
          "usage: helper-source-native-observer ABS_LADDER_JSON SHA256 "
          "default256|edited512|repeat|two O0|O3 ABS_FRESH_OUTPUT_DIR");
  StringRef LevelName(Argv[4]);
  require(LevelName == "O0" || LevelName == "O3", "closed optimization level");
  auto In = helperRecord(Argv[1], Argv[2], Argv[3]);
  helperLlvmNameControls();
  auto Llvm = helperLlvmObservation(In);
  require(helperLlvmNamesRetained(Llvm, In),
          "LLVM symbol names survive module destruction");
  const int Directory = helperDirectory(Argv[5]);
  json::Array PostLink;
  auto Built = helperBuild(In, LevelName == "O0" ? OptimizationLevel::O0
                                               : OptimizationLevel::O3,
                           Directory, PostLink);
  const auto &OutputValue = *Built.LinkedOutput;
  auto Evidence = helperAnalyze(In, OutputValue.Bytes);
  helperRecheck(In);
  require(helperLlvmNamesRetained(Llvm, In),
          "LLVM symbol names survive native worker execution");
  json::Array Pins;
  for (const auto &P : In.Pins)
    Pins.emplace_back(json::Object{
        {"path", P.Path}, {"sha256", P.Sha}, {"bytes", P.Bytes}});
  json::Object Report{
      {"schema", "fe2o3-helper-source-native-observation-v30"},
      {"label", In.Label}, {"optimization", LevelName},
      {"target", "gfx942:xnack-"}, {"code_object_version", 6},
      {"kernel_entry", In.Entry}, {"kernel_descriptor", In.Descriptor},
      {"retained_inputs", std::move(Pins)},
      {"llvm", std::move(Llvm.Fields)}, {"native", helperNativeObservation(Evidence)},
      {"post_link_checks", std::move(PostLink)},
      {"output_path", std::string(Argv[5]) + "/output.hsaco"},
      {"output_sha256", hex(OutputValue.Digest)},
      {"output_bytes", OutputValue.Bytes.size()},
      {"derivation_identity", hex(Built.Derivation->EvidenceIdentity)},
      {"worker_build_id", FE2O3_WORKER_BUILD_ID},
      {"llvm_build_id", FE2O3_LLVM_BUILD_ID},
      {"ordinary_worker_llvm_object_lld_completed", true},
      {"actual_llvm_bytes_unchanged", true},
      {"llvm_symbol_storage_verified_after_worker", true},
      {"retained_input_hashes_rechecked", true},
      {"typed_handoff_redecoded_here", false},
      {"record_hash_authenticates_source_custody", false},
      {"synthetic_request_identity_fields", true},
      {"physical_helper_abi_qualified", false},
      {"native_semantics_qualified", false},
      {"hardware_observed", false},
      {"protected_finalizer_admitted", false},
      {"grants_artifact_or_launch_authority", false},
      {"milestone_completion", false}};
  std::string Text;
  raw_string_ostream Stream(Text);
  Stream << formatv("{0:2}", json::Value(std::move(Report))) << '\n';
  Stream.flush();
  require(Text.size() <= 1024 * 1024, "helper observation report cap");
  helperWriteAt(Directory, "observation.json",
                ArrayRef<uint8_t>(reinterpret_cast<const uint8_t *>(Text.data()),
                                  Text.size()));
  require(fsync(Directory) == 0 && close(Directory) == 0,
          "helper output directory close");
  outs() << Text;
  return 0;
}
