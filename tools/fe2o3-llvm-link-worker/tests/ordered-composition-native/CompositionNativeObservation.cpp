// Test-only ordinary LLVM/LLD observation; no execution or source authority.
#define main retained_ordered_program_main_not_invoked
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/Support/AMDHSAKernelDescriptor.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Object/ELFObjectFile.h"
#include <limits.h>
#include <tuple>
#include <chrono>

namespace {
#include "BoundedInputs.inc"
#include "CompositionRecord.inc"
#include "helper-source-abi/HelperSourceWorker.inc"
#include "CompositionLlvm.inc"
#include "CompositionMetadata.inc"
#include "CompositionNative.inc"
}

int main(int argc,char **argv) {
  // The shared helper fixture also defines its older analyzer entry. Retain a
  // non-invoking reference; this observer uses compositionAnalyze instead.
  (void)&helperAnalyze;
  const auto Started=std::chrono::steady_clock::now();
  const auto Timely=[&](){require(std::chrono::steady_clock::now()-Started<std::chrono::seconds(110),
      "native observer total post-observation deadline");};
  require(argc==7,"usage: observer ABS_NORMAL_REPORT REPORT_SHA ABS_REPOSITORY FEATURE O0|O3 ABS_FRESH_OUTPUT");
  const StringRef LevelName(argv[5]);require(LevelName=="O0"||LevelName=="O3","optimization profile");
  auto R=compositionRecord(argv[1],argv[2],argv[3],argv[4]);Timely();
  auto Llvm=compositionLlvm(R);Timely();
  const int Directory=helperDirectory(argv[6]);json::Array Inspection;
  const auto Response=helperBuild(R.Input,LevelName=="O0"?OptimizationLevel::O0:OptimizationLevel::O3,
    Directory,Inspection);Timely();
  const auto &Bytes=Response.LinkedOutput->Bytes;
  auto Resources=compositionMetadata(Bytes,R);
  require(Resources.has_value(),"ordinary helper metadata/descriptor/argument resource relation");
  size_t MetadataNegatives=0;
  const auto Reject=[&](std::vector<uint8_t> Changed) {
    require(!compositionMetadata(Changed,R),"changed native descriptor accepted");++MetadataNegatives;Timely();
  };
  for(size_t Offset:{size_t(0),size_t(4),size_t(8)}) {
    auto Changed=Bytes;Changed[Resources->DescriptorOffset+Offset]^=1;Reject(std::move(Changed));
  }
  for(unsigned Kind=0;Kind<3;++Kind) {
    auto Changed=Bytes;
    using namespace llvm::amdhsa;
    const size_t At=Resources->DescriptorOffset;
    if(Kind<2) {
      auto Word=support::endian::read32le(Changed.data()+At+COMPUTE_PGM_RSRC1_OFFSET);
      if(Kind==0)Word&=~uint32_t(63);
      else Word=(Word&~uint32_t(15<<6))|uint32_t(15<<6);
      support::endian::write32le(Changed.data()+At+COMPUTE_PGM_RSRC1_OFFSET,Word);
    } else {
      auto Word=support::endian::read16le(Changed.data()+At+KERNEL_CODE_PROPERTIES_OFFSET);
      support::endian::write16le(Changed.data()+At+KERNEL_CODE_PROPERTIES_OFFSET,
        Word|KERNEL_CODE_PROPERTY_ENABLE_WAVEFRONT_SIZE32);
    }
    Reject(std::move(Changed));
  }
  auto Wrong=R;Wrong.ArgumentNames[0]+="_wrong";
  require(!compositionMetadata(Bytes,Wrong),"mutated LLVM/metadata argument join");++MetadataNegatives;
  Wrong=R;Wrong.Input.Descriptor+="_wrong";
  require(!compositionMetadata(Bytes,Wrong),"mutated exact descriptor symbol");++MetadataNegatives;
  for(unsigned Kind=0;Kind<2;++Kind) {
    auto Changed=Bytes;
    const size_t At=Resources->DescriptorOffset;
    using namespace llvm::amdhsa;
    if(Kind==0) {
      auto Word=support::endian::read32le(Changed.data()+At+COMPUTE_PGM_RSRC3_OFFSET);
      support::endian::write32le(Changed.data()+At+COMPUTE_PGM_RSRC3_OFFSET,Word^1);
    } else {
      auto Word=support::endian::read16le(Changed.data()+At+KERNEL_CODE_PROPERTIES_OFFSET);
      support::endian::write16le(Changed.data()+At+KERNEL_CODE_PROPERTIES_OFFSET,
        Word^KERNEL_CODE_PROPERTY_ENABLE_SGPR_QUEUE_PTR);
    }
    Reject(std::move(Changed));
  }
  static_assert(CompositionDynamicStackMask==llvm::amdhsa::KERNEL_CODE_PROPERTY_USES_DYNAMIC_STACK);
  using NativeSym=llvm::object::ELF64LE::Sym;
  for(unsigned Kind=0;Kind<8;++Kind) {
    auto Changed=Bytes;
    if(Kind<2) {
      // Reject each non-Boolean absolute resource symbol independently.
      support::endian::write64le(Changed.data()+Resources->StackSymbolOffsets[Kind]+
        offsetof(NativeSym,st_value),2);
    } else if(Kind==2) {
      // Valid Boolean payloads, opposite to retained metadata/descriptor.
      for(const auto At:Resources->StackSymbolOffsets)
        support::endian::write64le(Changed.data()+At+offsetof(NativeSym,st_value),
          Resources->UsesDynamicStack?0:1);
    } else if(Kind==3) {
      // Duplicate first symbol cannot substitute for missing second symbol.
      const auto Name=support::endian::read32le(Changed.data()+
        Resources->StackSymbolOffsets[0]+offsetof(NativeSym,st_name));
      support::endian::write32le(Changed.data()+Resources->StackSymbolOffsets[1]+
        offsetof(NativeSym,st_name),Name);
    } else if(Kind==4) {
      support::endian::write32le(Changed.data()+Resources->StackSymbolOffsets[0]+
        offsetof(NativeSym,st_name),0);
    } else if(Kind==5) {
      support::endian::write16le(Changed.data()+Resources->StackSymbolOffsets[0]+
        offsetof(NativeSym,st_shndx),llvm::ELF::SHN_UNDEF);
    } else if(Kind==6) {
      const auto At=Resources->DescriptorOffset+llvm::amdhsa::KERNEL_CODE_PROPERTIES_OFFSET;
      const auto Word=support::endian::read16le(Changed.data()+At);
      support::endian::write16le(Changed.data()+At,Word^CompositionDynamicStackMask);
    } else {
      Changed[Resources->DynamicMetadataOffset]^=1; // exact c2/c3 Boolean only
    }
    Reject(std::move(Changed));
  }
  require(MetadataNegatives==18,"metadata/resource mutation control census");
  bool Preserved=false;auto Machine=compositionMachine(R,Bytes,*Resources,Preserved);Timely();
  helperRecheck(R.Input);Timely();
  json::Array Pins;for(const auto &P:R.Input.Pins)Pins.emplace_back(json::Object{
    {"path",P.Path},{"bytes",P.Bytes},{"sha256",P.Sha}});
  json::Object Report{{"schema","fe2o3-composition-native-observation-v1"},
    {"feature",R.Input.Label},{"optimization",LevelName.str()},
    {"entry",R.Input.Entry},{"ordinary_worker_completed",true},{"unchanged_normal_llvm",true},
    {"llvm_sha256",hex(R.Input.Llvm.Digest)},{"hsaco_sha256",hex(Response.LinkedOutput->Digest)},
    {"hsaco_bytes",Bytes.size()},{"source_and_output_pins",std::move(Pins)},
    {"llvm_observation",std::move(Llvm)},{"resources",compositionResourceJson(*Resources)},
    {"metadata_mutation_refusals",MetadataNegatives},{"post_link_inspection",std::move(Inspection)},
    {"machine_observation",std::move(Machine)},{"authored_interval_observation_complete",Preserved},
    {"synthetic_worker_request_identity",true},{"source_custody_from_files",false},
    {"runtime_conditions_discharged",false},{"hardware_observed",false},{"protected_authority",false},
    {"functional_equivalence_proved",false},{"milestone_completion",false}};
  std::string Encoded;raw_string_ostream Stream(Encoded);Stream<<formatv("{0:2}",json::Value(std::move(Report)))<<"\n";Stream.flush();
  require(Encoded.size()<=2*1024*1024,"bounded native report");Timely();
  helperWriteAt(Directory,"observation.json",ArrayRef<uint8_t>(
    reinterpret_cast<const uint8_t*>(Encoded.data()),Encoded.size()));
  require(fsync(Directory)==0 && close(Directory)==0,"native output directory close");Timely();
  outs()<<Encoded;outs().flush();Timely();
  // Exit0 means the bounded observation completed, NOT all native preservation
  // checks passed. The root matrix must require all14 per-case flags before any
  // preservation qualification. Analyzer refusal is retained as incomplete.
  return 0;
}
