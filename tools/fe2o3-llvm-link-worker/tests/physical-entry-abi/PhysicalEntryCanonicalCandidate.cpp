// Opt-in normal-worker native observation of unchanged V20-emitted LLVM.
// Sidecars are diagnostic expectations only. No bytes-to-source/canonical custody.
#define FE2O3_PHYSICAL_ENTRY_MAIN fe2o3_unused_physical_entry_fixture_main
#include "PhysicalEntryAbiCandidate.cpp"
#undef FE2O3_PHYSICAL_ENTRY_MAIN
namespace {
#include "PhysicalEntryCanonicalInput.inc"
#include "PhysicalEntryCanonicalRelation.inc"
#include "PhysicalEntryCanonicalControls.inc"
void canonicalPhysicalReport(json::Object Report) {
  std::string Text;raw_string_ostream Stream(Text);
  Stream<<formatv("{0:2}",json::Value(std::move(Report)));Stream.flush();
  require(Text.size()<=512*1024,"canonical physical report byte bound");
  outs()<<Text<<'\n';
}
}
int main(int Argc,char **Argv) {
  alarm(90);
  require(Argc==5,"usage: physical-entry-canonical-candidate ABS_LLVM ABS_EXPECTATION O0|O3 ABS_FRESH_DIRECTORY");
  const StringRef LlvmPath(Argv[1]),ExpectationPath(Argv[2]),LevelName(Argv[3]),Directory(Argv[4]);
  require(LevelName=="O0"||LevelName=="O3","canonical physical optimization level");
  require(Directory.starts_with("/")&&Directory.size()<=4000&&
      llvm::none_of(Directory,[](char C){return static_cast<unsigned char>(C)<32||C==127;}),
      "canonical physical fresh absolute output directory");
  const Input Llvm=readSourceLlvm(LlvmPath),Sidecar=readSourceLlvm(ExpectationPath);
  require(Llvm.Bytes.size()<=32*1024&&Sidecar.Bytes.size()<=16*1024,"canonical physical input caps");
  auto Parsed=parseCanonicalPhysicalInput(Sidecar.Bytes);require(Parsed.has_value(),"canonical physical expectation grammar");
  require(canonicalInputJoin(*Parsed,Llvm),"canonical expectation/unchanged LLVM content join");
  auto Expected=canonicalPhysicalRows(*Parsed);require(Expected.has_value(),"closed canonical primitive/native row profile");
  const auto Level=LevelName=="O0"?OptimizationLevel::O0:OptimizationLevel::O3;
  auto SidecarNegatives=canonicalPhysicalSidecarNegatives(Sidecar.Bytes,Llvm);
  require(mkdir(Directory.str().c_str(),0700)==0,"canonical physical output directory must be new");
  physicalWrite(Directory.str()+"/input.ll",Llvm.Bytes);
  physicalWrite(Directory.str()+"/input.expect",Sidecar.Bytes);
  auto InputNegatives=canonicalPhysicalInputNegatives(Llvm,Parsed->Entry,Level);
  auto Built=buildRequest(makeInputRequest(Llvm,Level,Parsed->Entry),Parsed->Entry);
  const auto &Payload=Built.ResponseValue.LinkedOutput->Bytes;
  require(Payload.size()<=64*1024,"canonical physical HSACO byte bound");
  if(auto Failure=canonicalPhysicalRelation(*Parsed,*Expected,Built.Evidence,Payload))fail(*Failure);
  auto Metadata=physicalMetadata(Payload,Parsed->Entry);require(Metadata.has_value(),"canonical exact six/thirteen ABI metadata");
  auto Descriptor=physicalDescriptor(Built.Evidence,Payload,*Metadata,Parsed->Entry,Expected->Sgprs,Expected->Vgprs);
  require(Descriptor.has_value(),"canonical entry-state descriptor and actual authored resource minima");
  physicalWrite(Directory.str()+"/output.hsaco",Payload);
  auto Mutations=canonicalPhysicalMutations(*Parsed,*Expected,Built,*Descriptor);
  const auto LlvmAfter=readSourceLlvm(LlvmPath),SidecarAfter=readSourceLlvm(ExpectationPath);
  require(LlvmAfter.Bytes==Llvm.Bytes&&LlvmAfter.Digest==Llvm.Digest&&
      SidecarAfter.Bytes==Sidecar.Bytes&&SidecarAfter.Digest==Sidecar.Digest,
      "canonical native inputs changed during observation");
  json::Array Trace,Blocks,Inspection;
  for(const auto &I:Built.Evidence.Instructions) {
    auto Row=instructionJson(I);Row["branch_kind"]=static_cast<unsigned>(I.BranchKind);
    Row["branch_target"]=I.BranchTarget;Row["memory_access"]=static_cast<unsigned>(I.MemoryAccess);
    Row["memory_width"]=I.MemoryWidth;Row["explicit_definitions"]=I.ExplicitDefinitionCount;
    json::Array Operands;for(const auto &O:I.Operands)Operands.emplace_back(json::Object{
      {"kind",static_cast<unsigned>(O.Kind)},{"register",O.Register},
      {"value_u64_decimal",std::to_string(O.Value)},{"tied_to",O.TiedTo}});
    Row["operands"]=std::move(Operands);Trace.emplace_back(std::move(Row));
  }
  for(const auto &B:Built.Evidence.Blocks) {
    json::Array Edges;for(auto E:B.Successors)Edges.emplace_back(E);
    Blocks.emplace_back(json::Object{{"ordinal",B.Ordinal},{"first_offset",B.FirstInstructionOffset},
      {"instructions",B.InstructionCount},{"successors",std::move(Edges)}});
  }
  for(const auto &D:Built.InspectionDiagnostics)Inspection.emplace_back(D);
  canonicalPhysicalReport(json::Object{
    {"schema","private-canonical-physical-entry-native-observation-v20-r1"},
    {"entry",Parsed->Entry},{"optimization",LevelName},{"upstream_revision",UpstreamRevision},
    {"target","gfx942:xnack-"},{"wave_width",64},{"code_object_version",6},
    {"llvm_function_shell","ordinary"},{"workgroup",json::Array{64,1,1}},
    {"maximum_workgroups_premise",json::Array{2,1,1}},
    {"canonical_claimed_sha256",Parsed->CanonicalDigest},{"canonical_claimed_bytes",Parsed->CanonicalBytes},
    {"expectation_sha256",hex(Sidecar.Digest)},{"expectation_bytes",Sidecar.Bytes.size()},
    {"llvm_sha256",hex(Llvm.Digest)},{"llvm_bytes",Llvm.Bytes.size()},
    {"hsaco_sha256",hex(SHA256::hash(Payload))},{"hsaco_bytes",Payload.size()},
    {"entry_bytes",Built.Evidence.Entries[0].CodeSize},{"authored_instructions",Trace.size()},
    {"compiler_prologue_instructions",0},{"compiler_tail_instructions",0},
    {"expected_sgpr_minimum",Expected->Sgprs},{"expected_vgpr_minimum",Expected->Vgprs},
    {"descriptor",json::Object{{"file_offset",Descriptor->Offset},{"bytes",64},{"sha256",hex(Descriptor->Digest)},
      {"rsrc1",Descriptor->Rsrc1},{"rsrc2",Descriptor->Rsrc2},{"rsrc3",Descriptor->Rsrc3},
      {"code_properties",Descriptor->Properties},{"kernarg_preload",0},
      {"vgpr_capacity",Descriptor->Vgprs},{"sgpr_capacity",Descriptor->Sgprs}}},
    {"metadata_arguments",physicalArgumentsJson(*Metadata)},{"trace",std::move(Trace)},
    {"decoded_cfg",std::move(Blocks)},{"post_link_checks",std::move(Inspection)},
    {"input_refusals",std::move(InputNegatives)},{"expectation_refusals",std::move(SidecarNegatives)},
    {"native_mutation_refusals",std::move(Mutations)},{"outputs",Directory},
    {"expectations_are_inert",true},{"external_same_source_owner_join_required",true},
    {"synthetic_worker_identity_fields",true},{"source_authentication",false},
    {"canonical_owner_admission",false},{"native_functional_execution",false},{"hardware_execution",false},
    {"protected_finalizer_admission",false},{"general_hazard_model_qualified",false},
    {"runtime_pointer_validity_proved",false},{"owner_contract_accepted",false},{"milestone_completion",false}});
  alarm(0);return 0;
}
