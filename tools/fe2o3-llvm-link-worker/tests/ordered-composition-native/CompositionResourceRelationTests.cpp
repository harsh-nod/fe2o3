#include <cstdint>
#include <iostream>
#include "CompositionResourceRelation.inc"
int main() {
  unsigned Count = 0;
  const auto Check = [&](bool Good) { ++Count; if (!Good) { std::cerr << "resource control " << Count << " failed\n"; return false; } return true; };
  const auto Allocation = [](uint64_t A, uint64_t G, uint64_t T, uint32_t R1, uint32_t R3) {
    return compositionGfx942Allocation(A, G, T, R1, R3);
  };
  if (!Check(Allocation(37,10,50,6,9)) || // actual O0: 37 + alignment3 + AGPR10
      !Check(Allocation(37,0,37,4,9)) ||
      !Check(Allocation(40,10,50,6,9)) ||
      !Check(Allocation(256,0,256,31,63)) ||
      !Check(Allocation(252,4,256,31,62)) ||
      !Check(!Allocation(37,10,47,6,9)) ||
      !Check(!Allocation(37,10,50,5,9)) ||
      !Check(!Allocation(37,10,50,6,8)) ||
      !Check(!Allocation(37,10,50,6,10)) ||
      !Check(!Allocation(37,11,50,6,9)) ||
      !Check(!Allocation(41,10,50,6,10)) ||
      !Check(!Allocation(37,0,40,4,9)) ||
      !Check(!Allocation(36,0,36,4,8)) ||
      !Check(!Allocation(257,0,257,32,0)) ||
      !Check(!Allocation(252,5,257,32,62)) ||
      !Check(!Allocation(37,257,297,37,9)) ||
      !Check(!Allocation(UINT64_MAX,10,50,6,9)) ||
      !Check(!Allocation(37,UINT64_MAX,50,6,9)) ||
      !Check(!Allocation(37,10,UINT64_MAX,6,9)) ||
      !Check(!Allocation(37,10,50,32,9)) ||
      !Check(compositionGfx942HiddenQueuePointer(232,8)) ||
      !Check(!compositionGfx942HiddenQueuePointer(280,8)) ||
      !Check(!compositionGfx942HiddenQueuePointer(231,8)) ||
      !Check(!compositionGfx942HiddenQueuePointer(232,4)))
    return 1;
  for(uint64_t D=0;D<2;++D)for(uint64_t R=0;R<2;++R) {
    const bool Expected=(D|R)!=0;
    const uint16_t Properties=Expected?CompositionDynamicStackMask:0;
    if(!Check(compositionGfx942DynamicStack(D,R,Expected,Properties)) ||
       !Check(!compositionGfx942DynamicStack(D,R,!Expected,Properties)) ||
       !Check(!compositionGfx942DynamicStack(D,R,Expected,
         Properties^CompositionDynamicStackMask)))return 1;
  }
  if(!Check(!compositionGfx942DynamicStack(2,0,true,CompositionDynamicStackMask)) ||
     !Check(!compositionGfx942DynamicStack(0,2,true,CompositionDynamicStackMask)) ||
     !Check(!compositionGfx942DynamicStack(UINT64_MAX,0,true,CompositionDynamicStackMask)) ||
     !Check(!compositionGfx942DynamicStack(0,UINT64_MAX,true,CompositionDynamicStackMask)))
    return 1;
  std::cout << Count << " gfx942 resource/hidden-slot controls passed\n";
  return Count == 40 ? 0 : 1;
}
