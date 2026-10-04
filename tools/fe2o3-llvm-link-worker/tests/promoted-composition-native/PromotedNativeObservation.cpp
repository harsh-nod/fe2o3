// Distinct promoted-source profile. No arbitrary LLVM import or source authority.
#define main retained_ordered_program_main_not_invoked
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/Support/AMDHSAKernelDescriptor.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/ADT/STLFunctionalExtras.h"
#include <limits.h>
#include <tuple>
#include <chrono>
namespace {
#include "ordered-composition-native/BoundedInputs.inc"
#include "ordered-composition-native/CompositionRecord.inc"
#include "helper-source-abi/HelperSourceWorker.inc"
#include "ordered-composition-native/CompositionLlvm.inc"
#include "ordered-composition-native/CompositionMetadata.inc"
#include "ordered-composition-native/CompositionNative.inc"
#include "ordered-composition-native/CompositionRun.inc"
#include "PromotedInputs.inc"
#include "PromotedRecord.inc"
}
#ifndef PROMOTED_COMPOSITION_NO_OBSERVER_MAIN
int main(int argc,char **argv) {
  (void)&helperAnalyze;(void)&compositionRecord;
  const auto Started=std::chrono::steady_clock::now();
  require(argc==7,"usage: promoted observer ABS_STAGE_A_REPORT SHA ABS_REPOSITORY copy|preserve|edit O0|O3 ABS_FRESH_OUTPUT");
  const StringRef Level(argv[5]);require(Level=="O0"||Level=="O3","closed optimization");
  auto P=promotedRecord(argv[1],argv[2],argv[3],argv[4]);
  require(std::chrono::steady_clock::now()-Started<std::chrono::seconds(110),"promoted input deadline");
  // Copies only the bounded inert inspection record; source stamp custody remains
  // in P and is rechecked after the actual ordinary worker, before report output.
  return compositionRun(P.Record,Level,argv[6],Started,
    "fe2o3-promoted-composition-native-observation-v1",[&]{promotedRecheck(P);});
}
#endif
