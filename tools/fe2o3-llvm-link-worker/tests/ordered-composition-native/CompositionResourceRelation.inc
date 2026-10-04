// Test-only gfx942 allocation relation; no machine-effect or execution admission.
// Sources: pinned AMDGPUAsmPrinter.cpp:958-1008 and
// AMDGPUBaseInfo.cpp:2597-2601. Metadata counts the combined unified allocation.
// Pinned AsmPrinter.cpp:1014-1016 joins resource symbols with OR, then
// lines594-605 emit descriptor bit11; HSAMetadataStreamer.cpp:506-508 emits
// the identical Boolean. This observes backend accounting, not source recursion.
constexpr uint16_t CompositionDynamicStackMask = uint16_t(1u << 11);
bool compositionGfx942DynamicStack(uint64_t HasDynSizedStack,
                                  uint64_t HasRecursion, bool Metadata,
                                  uint16_t Properties) {
  if (HasDynSizedStack > 1 || HasRecursion > 1)
    return false;
  const bool Expected = (HasDynSizedStack | HasRecursion) != 0;
  return Metadata == Expected &&
         bool(Properties & CompositionDynamicStackMask) == Expected;
}
constexpr uint64_t CompositionQueuePointerOffset = 32 + 200;
bool compositionGfx942HiddenQueuePointer(uint64_t Offset, uint64_t Size) {
  return Offset == CompositionQueuePointerOffset && Size == 8;
}
bool compositionGfx942Allocation(uint64_t Arch, uint64_t Acc, uint64_t Total,
                                uint32_t Rsrc1, uint32_t Rsrc3, uint64_t Minimum = 37) {
  // The two closed test profiles need v36 or v12 respectively. This is a
  // minimum observed architectural coverage, not exact footprint or no-AGPR.
  if ((Minimum != 37 && Minimum != 13) || Arch < Minimum || Arch > 256 ||
      Acc > 256 || Total < Minimum || Total > 256)
    return false;
  const uint64_t AlignedArch = ((Arch + 3) / 4) * 4;
  const uint64_t ExpectedTotal = Acc ? AlignedArch + Acc : Arch;
  const uint64_t Capacity = ((Rsrc1 & 63) + 1) * 8;
  const uint64_t AccumStart = ((Rsrc3 & 63) + 1) * 4;
  return Total == ExpectedTotal && Capacity >= Total && Capacity <= 256 &&
         AccumStart == AlignedArch;
}
