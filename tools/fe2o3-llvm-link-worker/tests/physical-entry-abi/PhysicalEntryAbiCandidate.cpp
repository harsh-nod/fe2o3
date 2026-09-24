// Inert fixed-profile transport qualification only. No source or launch authority.
#define main fe2o3_unused_ordered_program_test_main
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Support/AMDHSAKernelDescriptor.h"
namespace {
constexpr StringLiteral PhysicalName = "physical_entry_select_fixture";
constexpr StringLiteral UpstreamRevision = "f58b06dce1f9c15707c5f808fd002e18c2accf7e";
#include "PhysicalEntryInput.inc"
#include "PhysicalEntryMetadata.inc"
#include "PhysicalEntryRelation.inc"
#include "PhysicalEntryDescriptor.inc"
#include "PhysicalEntryControls.inc"

void physicalWrite(StringRef Path, ArrayRef<uint8_t> Bytes) {
  require(Path.starts_with("/") && Path.size() <= 4096 && !Bytes.empty() && Bytes.size() <= 64 * 1024
      && llvm::none_of(Path, [](char C) { return static_cast<unsigned char>(C) < 32 || C == 127; }),
      "physical output path/byte cap");
  int FD = open(Path.str().c_str(), O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  require(FD >= 0, "physical output requires fresh exclusive regular file");
  size_t At = 0;
  while (At < Bytes.size()) {
    ssize_t Count = write(FD, Bytes.data() + At, Bytes.size() - At);
    if (Count < 0 && errno == EINTR) continue;
    require(Count > 0, "physical output write"); At += static_cast<size_t>(Count);
  }
  struct stat S{};
  require(fstat(FD, &S) == 0 && S_ISREG(S.st_mode) && S.st_nlink == 1
      && static_cast<uint64_t>(S.st_size) == Bytes.size(), "physical output extent");
  require(fsync(FD) == 0 && close(FD) == 0, "physical output flush/close");
  auto After = readSourceLlvm(Path); // Existing bounded byte reader; no LLVM parsing.
  require(After.Bytes.size() == Bytes.size() &&
      std::equal(After.Bytes.begin(), After.Bytes.end(), Bytes.begin()), "physical output readback");
}
}
int main(int Argc, char **Argv) {
  alarm(90);
  require(Argc == 4, "usage: physical-entry-abi-candidate copy|select O0|O3 ABS_FRESH_DIRECTORY");
  const StringRef Mode(Argv[1]), LevelName(Argv[2]), Directory(Argv[3]);
  require(Mode == "copy" || Mode == "select", "closed physical mode");
  require(LevelName == "O0" || LevelName == "O3", "closed optimization");
  require(Directory.starts_with("/") && Directory.size() <= 4000 &&
      llvm::none_of(Directory, [](char C) { return static_cast<unsigned char>(C) < 32 || C == 127; }),
      "fresh absolute output directory");
  require(mkdir(Directory.str().c_str(), 0700) == 0, "physical directory must be new");
  const bool Select = Mode == "select";
  const auto Level = LevelName == "O0" ? OptimizationLevel::O0 : OptimizationLevel::O3;
  const auto InputValue = physicalInput(Select);
  physicalWrite(Directory.str() + "/input.ll", InputValue.Bytes);
  auto InputNegatives = physicalInputNegatives(Select, Level);
  auto Built = buildRequest(makeInputRequest(InputValue, Level, PhysicalName), PhysicalName);
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  require(Payload.size() <= 64 * 1024, "physical HSACO cap");
  physicalWrite(Directory.str() + "/output.hsaco", Payload);
  if (auto Failure = physicalRelation(Select, Built.Evidence, Payload)) fail(*Failure);
  auto Metadata = physicalMetadata(Payload, PhysicalName);
  require(Metadata.has_value(), "physical exact six explicit/thirteen hidden argument metadata");
  auto Descriptor = physicalDescriptor(Built.Evidence, Payload, *Metadata);
  require(Descriptor.has_value(), "physical entry-state descriptor/metadata relation");
  auto Mutations = physicalMutations(Select, Built, *Descriptor);
  json::Array Trace, Blocks, Inspection;
  for (const auto &I : Built.Evidence.Instructions) {
    auto Row = instructionJson(I);
    Row["branch_kind"] = static_cast<unsigned>(I.BranchKind); Row["branch_target"] = I.BranchTarget;
    Row["memory_access"] = static_cast<unsigned>(I.MemoryAccess); Row["memory_width"] = I.MemoryWidth;
    Row["explicit_definitions"] = I.ExplicitDefinitionCount;
    json::Array Operands;
    for (const auto &O : I.Operands) Operands.emplace_back(json::Object{
        {"kind", static_cast<unsigned>(O.Kind)}, {"register", O.Register},
        {"value_u64_decimal", std::to_string(O.Value)}, {"tied_to", O.TiedTo}});
    Row["operands"] = std::move(Operands); Trace.emplace_back(std::move(Row));
  }
  for (const auto &B : Built.Evidence.Blocks) {
    json::Array Edges; for (auto E : B.Successors) Edges.emplace_back(E);
    Blocks.emplace_back(json::Object{{"ordinal", B.Ordinal}, {"first_offset", B.FirstInstructionOffset},
        {"instructions", B.InstructionCount}, {"successors", std::move(Edges)}});
  }
  for (const auto &D : Built.InspectionDiagnostics) Inspection.emplace_back(D);
  emitReport(json::Object{
      {"schema", "private-physical-entry-abi-observation-v1"}, {"mode", Mode}, {"optimization", LevelName},
      {"llvm_function_shell", "ordinary"}, {"code_object_version", 6},
      {"compiler_owned_hardware_entry_descriptor", true}, {"author_owned_instruction_body", true},
      {"upstream_revision", UpstreamRevision}, {"target", "gfx942:xnack-"}, {"wave_width", 64},
      {"workgroup", json::Array{64, 1, 1}}, {"maximum_workgroups_premise", json::Array{2, 1, 1}},
      {"llvm_sha256", hex(InputValue.Digest)}, {"llvm_bytes", InputValue.Bytes.size()},
      {"hsaco_sha256", hex(SHA256::hash(Payload))}, {"hsaco_bytes", Payload.size()},
      {"entry_bytes", Built.Evidence.Entries[0].CodeSize}, {"authored_instructions", Trace.size()},
      {"compiler_prologue_instructions", 0}, {"compiler_tail_instructions", 0},
      {"entry_kernarg_sgpr_pair", "s[0:1]"}, {"entry_workgroup_x", "s2"}, {"entry_workitem_x", "v0"},
      {"descriptor", json::Object{{"file_offset", Descriptor->Offset}, {"bytes", 64},
          {"sha256", hex(Descriptor->Digest)}, {"rsrc1", Descriptor->Rsrc1}, {"rsrc2", Descriptor->Rsrc2},
          {"rsrc3", Descriptor->Rsrc3}, {"code_properties", Descriptor->Properties},
          {"kernarg_preload", 0}, {"vgpr_capacity", Descriptor->Vgprs}, {"sgpr_capacity", Descriptor->Sgprs}}},
      {"metadata_arguments", physicalArgumentsJson(*Metadata)},
      {"trace", std::move(Trace)}, {"decoded_cfg", std::move(Blocks)},
      {"input_refusals", std::move(InputNegatives)}, {"native_mutation_refusals", std::move(Mutations)},
      {"post_link_checks", std::move(Inspection)}, {"outputs", Directory},
      {"synthetic_worker_identity_fields", true}, {"source_authentication", false},
      {"canonical_owner_admission", false}, {"native_functional_execution", false},
      {"hardware_execution", false}, {"protected_finalizer_admission", false},
      {"general_hazard_model_qualified", false}, {"runtime_pointer_validity_proved", false},
      {"owner_contract_accepted", false}, {"milestone_completion", false}});
  alarm(0); return 0;
}
