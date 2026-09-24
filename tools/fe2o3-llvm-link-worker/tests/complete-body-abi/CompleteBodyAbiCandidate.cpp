// Test-only actual LLVM/native mechanism fixture; never GPU execution or a source owner.
#define main fe2o3_unused_ordered_program_test_main
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/IR/IntrinsicsAMDGPU.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/Support/AMDHSAKernelDescriptor.h"
namespace {
#include "CompleteBodyPlans.inc"
#include "CompleteBodyLlvm.inc"
#include "CompleteBodyNative.inc"
#include "CompleteBodyMetadata.inc"
#include "CompleteBodySelector.inc"

void completeWritePayload(StringRef Path,ArrayRef<uint8_t> Bytes) {
  require(Path.starts_with("/") && Path.size()<=4096 && !Bytes.empty() && Bytes.size()<=64*1024 &&
    llvm::none_of(Path,[](char C){return static_cast<unsigned char>(C)<32 || C==127;}),
    "bounded exclusive payload path/bytes required");
  const int FD=open(Path.str().c_str(),O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);
  require(FD>=0,"payload output must be a fresh exclusive leaf");
  size_t At=0;
  while(At<Bytes.size()) {
    const ssize_t N=write(FD,Bytes.data()+At,Bytes.size()-At);
    if(N<0 && errno==EINTR) continue;
    require(N>0,"payload output write failed"); At+=static_cast<size_t>(N);
  }
  struct stat S{};
  require(fstat(FD,&S)==0 && S_ISREG(S.st_mode) && S.st_nlink==1 &&
      static_cast<uint64_t>(S.st_size)==Bytes.size(),"published payload extent");
  require(fsync(FD)==0 && close(FD)==0,"payload output flush/close");
  const auto After=readSourceLlvm(Path); // unchanged bounded byte reader; no parse
  require(After.Bytes.size()==Bytes.size() &&
      std::equal(After.Bytes.begin(),After.Bytes.end(),Bytes.begin()),"retained payload readback");
}
json::Object completeReport(const CompletePlan &P,const Input &InputValue,
    OptimizationLevel Level,StringRef Symbol,BuiltFixture &Built,StringRef PayloadPath) {
  auto &Payload=Built.ResponseValue.LinkedOutput->Bytes;
  require(Payload.size()<=64*1024,"retained HSACO cap");
  // Retain exact emitted bytes even if a later relation refuses. A file is not
  // qualification: only the final successful report records all checks.
  completeWritePayload(PayloadPath,Payload);
  CompleteLayout Layout;
  const auto Relation=completeRelation(P,Built.Evidence,Payload,Symbol,&Layout);
  if(Relation) fail(*Relation);
  const auto Metadata=completeMetadata(Payload,Symbol,Level);
  require(Metadata.has_value(),"actual six-slot/hidden metadata profile refused");
  const auto Descriptor=completeDescriptor(P,Built.Evidence,Payload,Symbol,*Metadata);
  require(Descriptor.has_value(),"actual descriptor/resource/payload relation refused");
  CompleteSelectorFlow SelectorFlow;
  if(auto Failure=completeSelectorFlow(Built.Evidence,*Descriptor,Layout.Start,&SelectorFlow)) fail(*Failure);
  auto SelectorNegatives=completeSelectorNegatives(Built,Symbol,*Descriptor,Layout.Start,SelectorFlow);
  auto NativeNegatives=completeNativeNegatives(P,Built,Symbol,Layout);
  auto MetadataNegatives=completeMetadataNegatives(Payload,Symbol,Level);
  auto DescriptorNegatives=completeDescriptorNegatives(P,Built,Symbol,*Descriptor,Level);
  json::Array Trace,Blocks,Authored,Arithmetic,Inspections,SelectorCopies;
  for(size_t I:SelectorFlow.Copies)
    SelectorCopies.emplace_back(Built.Evidence.Instructions[I].InstructionOffset);
  for(const auto &I:Built.Evidence.Instructions) {
    auto Row=instructionJson(I);
    Row["block"]=I.BlockOrdinal; Row["branch_kind"]=static_cast<unsigned>(I.BranchKind);
    Row["branch_target"]=I.BranchTarget; Row["memory_access"]=static_cast<unsigned>(I.MemoryAccess);
    Row["memory_width"]=I.MemoryWidth; Row["explicit_definitions"]=I.ExplicitDefinitionCount;
    json::Array Operands;
    for(const auto &O:I.Operands) Operands.emplace_back(json::Object{
      {"kind",static_cast<unsigned>(O.Kind)},{"register",O.Register},
      {"value_u64_decimal",std::to_string(O.Value)},{"tied_to",O.TiedTo}});
    Row["operands"]=std::move(Operands); Trace.emplace_back(std::move(Row));
  }
  for(const auto &B:Built.Evidence.Blocks) {
    json::Array Edges; for(auto E:B.Successors) Edges.emplace_back(E);
    Blocks.emplace_back(json::Object{{"ordinal",B.Ordinal},{"first_offset",B.FirstInstructionOffset},
      {"instructions",B.InstructionCount},{"successors",std::move(Edges)}});
  }
  size_t Ordinal=0;
  for(size_t I=0;I<P.Blocks.size();++I) {
    const auto &B=P.Blocks[I];
    json::Array Edges;
    if(B.End==CompleteEnd::Zero) {Edges.emplace_back(B.Zero);Edges.emplace_back(B.Nonzero);}
    else if(B.End==CompleteEnd::Jump) Edges.emplace_back(B.Zero);
    Authored.emplace_back(json::Object{{"ordinal",I},{"source_label",B.Label},
      {"native_first_offset",Built.Evidence.Instructions[Layout.BlockStarts[I]].InstructionOffset},
      {"steps",B.Steps.size()},{"successors",std::move(Edges)},{"compiler_tail",B.End==CompleteEnd::Tail}});
    for(const auto &S:B.Steps) {
      Arithmetic.emplace_back(json::Object{{"ordinal",Ordinal},
        {"native_offset",Built.Evidence.Instructions[Layout.Arithmetic[Ordinal]].InstructionOffset},
        {"opcode",machineOpcode(S.Op)},{"destination",S.Destination},{"source0",S.Source0},
        {"source1",S.Op==Opcode::Mov?json::Value(nullptr):json::Value(S.Source1)}});
      ++Ordinal;
    }
  }
  for(const auto &I:Built.InspectionDiagnostics) Inspections.emplace_back(I);
  return json::Object{
    {"schema","private-complete-body-abi-mechanism-v1"},{"authority","observation_only"},
    {"profile",P.Name},{"optimization",Level==OptimizationLevel::O0?"O0":"O3"},
    {"llvm_sha256",hex(InputValue.Digest)},{"llvm_bytes",InputValue.Bytes.size()},
    {"hsaco_sha256",hex(Built.ResponseValue.LinkedOutput->Digest)},{"hsaco_bytes",Payload.size()},
    {"retained_hsaco_path",PayloadPath},{"entry_offset",Built.Evidence.Entries[0].CodeOffset},
    {"entry_bytes",Built.Evidence.Entries[0].CodeSize},
    {"compiler_prologue_instructions",Layout.Start},{"compiler_tail_instructions",9},
    {"compiler_tail_offset",Built.Evidence.Instructions[Layout.Tail].InstructionOffset},
    {"authored_blocks",std::move(Authored)},{"authored_instructions",std::move(Arithmetic)},
    {"complete_entry_trace",std::move(Trace)},{"decoded_cfg",std::move(Blocks)},
    {"target","gfx942:xnack-"},{"wave_width",64},{"workgroup_size",64},{"code_object_version",6},
    {"explicit_argument_bytes",32},{"hidden_argument_extent",256},{"kernarg_bytes",288},
    {"metadata_arguments",completeArgumentsJson(*Metadata)},
    {"hidden_argument_metadata_entries",Metadata->Arguments.size()-6},
    {"metadata_resources",json::Object{{"sgprs",Metadata->Sgprs},{"vgprs",Metadata->Vgprs},
      {"agprs",Metadata->Agprs},{"sgpr_spills",Metadata->SgprSpills},{"vgpr_spills",Metadata->VgprSpills},
      {"lds_bytes",0},{"private_bytes",0},{"dynamic_stack",false}}},
    {"descriptor",json::Object{{"file_offset",Descriptor->Offset},{"bytes",64},
      {"sha256",hex(Descriptor->Digest)},{"rsrc1",Descriptor->Rsrc1},{"rsrc3",Descriptor->Rsrc3},
      {"vgpr_capacity",Descriptor->VgprCapacity},{"sgpr_capacity",Descriptor->SgprCapacity},
      {"architected_vgpr_boundary",Descriptor->Boundary}}},
    {"native_mutation_refusals",std::move(NativeNegatives)},
    {"metadata_mutation_refusals",std::move(MetadataNegatives)},
    {"descriptor_mutation_refusals",std::move(DescriptorNegatives)},
    {"selector_native_provenance",json::Object{{"kernarg_pointer_sgpr",SelectorFlow.KernargSgpr},
      {"load_offset",Built.Evidence.Instructions[SelectorFlow.Load].InstructionOffset},
      {"kernarg_byte_offset",28},{"destination_sgpr",22},{"load_ready",true},
      {"copy_offsets",std::move(SelectorCopies)}}},
    {"selector_mutation_refusals",std::move(SelectorNegatives)},
    {"post_link_metadata_checks",std::move(Inspections)},
    {"synthetic_worker_identity_fields",true},{"source_authentication",false},
    {"canonical_owner_admission",false},{"native_functional_execution",false},
    {"hardware_execution",false},{"protected_finalizer_admission",false}};
}
}
int main(int Argc,char **Argv) {
  alarm(90);
  require(Argc==5,"usage: complete-body-abi-candidate PROFILE O0|O3 ABS_LLVM ABS_FRESH_HSACO");
  const CompletePlan P=completePlan(Argv[1]);
  const StringRef LevelArg(Argv[2]);
  require(LevelArg=="O0" || LevelArg=="O3","optimization must be O0 or O3");
  const auto Level=LevelArg=="O0"?OptimizationLevel::O0:OptimizationLevel::O3;
  const auto InputValue=readCompleteLlvm(Argv[3]);
  const auto Symbol=completeSymbol(InputValue,P);
  require(Symbol.has_value(),"exact typed complete-body LLVM profile refused");
  auto LlvmNegatives=completeLlvmNegatives(InputValue,P);
  auto Built=buildRequest(makeInputRequest(InputValue,Level,*Symbol),*Symbol);
  auto Report=completeReport(P,InputValue,Level,*Symbol,Built,Argv[4]);
  const auto After=readCompleteLlvm(Argv[3]);
  require(After.Bytes==InputValue.Bytes && After.Digest==InputValue.Digest,"LLVM changed across native compilation");
  Report["llvm_mutation_refusals"]=std::move(LlvmNegatives);
  emitReport(std::move(Report));
  alarm(0);
  return 0;
}
