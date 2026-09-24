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
#include "llvm/ADT/STLFunctionalExtras.h"

namespace {
#include "BoundedInputs.inc"
#include "CompositionRecord.inc"
#include "helper-source-abi/HelperSourceWorker.inc"
#include "CompositionLlvm.inc"
#include "CompositionMetadata.inc"
#include "CompositionNative.inc"
#include "CompositionRun.inc"
}

int main(int argc,char **argv) {
  (void)&helperAnalyze;
  const auto Started=std::chrono::steady_clock::now();
  require(argc==7,"usage: observer ABS_NORMAL_REPORT REPORT_SHA ABS_REPOSITORY FEATURE O0|O3 ABS_FRESH_OUTPUT");
  const StringRef LevelName(argv[5]);require(LevelName=="O0"||LevelName=="O3","optimization profile");
  auto R=compositionRecord(argv[1],argv[2],argv[3],argv[4]);
  require(std::chrono::steady_clock::now()-Started<std::chrono::seconds(110),"native observer input deadline");
  return compositionRun(std::move(R),LevelName,argv[6],Started,
    "fe2o3-composition-native-observation-v1",[]{});
}
