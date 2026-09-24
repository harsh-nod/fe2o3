// Pure literal/resource controls, not actual-source or native qualification.
#define PROMOTED_COMPOSITION_NO_OBSERVER_MAIN
#include "PromotedNativeObservation.cpp"
int main() {
  (void)&promotedRecord;(void)&compositionRun;(void)&helperAnalyze;(void)&compositionRecord;
  require(compositionPatternCount(CompositionProfile::LegacyV32)==3 &&
    compositionPatternCount(CompositionProfile::PromotedV8)==2,"closed pattern counts");
  require(compositionAssembly(0)=="v_xor_b32_e32 v32, $1, $2\n\tv_and_b32_e32 $0, v32, $3" &&
    compositionAssembly(1)=="v_mov_b32_e32 $0, $1" &&
    compositionAssembly(2)=="v_mov_b32_e32 $0, $2","legacy exact assembly");
  require(compositionConstraints(CompositionProfile::LegacyV32)==Constraints,"legacy exact constraints");
  require(compositionAssembly(0,CompositionProfile::PromotedV8)==
    "v_xor_b32_e32 v8, $1, $2\n\tv_and_b32_e32 $0, v8, $3" &&
    compositionAssembly(1,CompositionProfile::PromotedV8)=="v_mov_b32_e32 $0, $3",
    "promoted pair and input2 literal assembly");
  require(compositionConstraints(CompositionProfile::PromotedV8)==
    "=&{v9},{v10},{v11},{v12},~{v8}","actual emitter roles");
  require(expectedWord(compositionSteps(0)[0])==0x2a404722 &&
    expectedWord(compositionSteps(1)[0])==0x7e420322 &&
    expectedWord(compositionSteps(2)[0])==0x7e420323,"legacy independent words");
  require(expectedWord(compositionSteps(0,CompositionProfile::PromotedV8)[0])==0x2a10170a &&
    expectedWord(compositionSteps(0,CompositionProfile::PromotedV8)[1])==0x26121908 &&
    expectedWord(compositionSteps(1,CompositionProfile::PromotedV8)[0])==0x7e12030c,
    "promoted independently transcribed machine words");
  require(compositionGfx942Allocation(37,0,37,4,9) &&
    !compositionGfx942Allocation(36,0,36,4,8),"legacy minimum unchanged");
  require(compositionGfx942Allocation(13,0,13,1,3,13) &&
    compositionGfx942Allocation(13,3,19,2,3,13),"minimum13 observes capacity not exact footprint");
  require(!compositionGfx942Allocation(12,0,12,1,2,13) &&
    !compositionGfx942Allocation(13,0,13,0,3,13) &&
    !compositionGfx942Allocation(13,0,13,1,3,12),"short or unknown minimum refuses");
  require(compositionGfx942HiddenQueuePointer(232,8) &&
    !compositionGfx942HiddenQueuePointer(280,8),"unchanged hidden queue relation");
  require(compositionGfx942DynamicStack(1,0,true,CompositionDynamicStackMask) &&
    !compositionGfx942DynamicStack(2,0,true,CompositionDynamicStackMask),"unchanged stack relation");
  outs()<<"12 pure promoted/legacy literal and resource groups passed; no source/native run.\n";
}
